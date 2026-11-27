//! A **`CGEventTap`** is a hook that CoreGraphics (the macOS window server client library)
//! calls for every input event before the focused app sees it. We create one at the
//! "session" level with `CGEventTapCreate`, add it to a `CFRunLoop` on a background thread,
//! and our callback runs for each key event. The callback may return the event unchanged
//! (the app receives it) or return null, which **swallows** the event (the app never sees
//! it). That is how bare End can start dictation without also moving the cursor.
//!
//! Creating a tap that can see and swallow keys needs the **Accessibility** permission. If
//! the permission is missing, `CGEventTapCreate` returns null, and the tap must be created
//! again after the user grants it (see `CLAUDE.md`, "Lessons learned").
//!
//! macOS reports normal keys with key-down / key-up events and modifier keys with
//! `flagsChanged` events; see `native_keys.rs` for keycodes and the device-dependent flag bits
//! that tell Right Shift from Left Shift.

use super::{
    CaptureState, ChordMatcher, KeyClass, KeyInput, NativeHotkeyEvent, ShortcutCaptureEvent,
};
use crate::native_keys;

#[cfg(target_os = "macos")]
pub(super) use event_tap::PlatformNativeMonitor;

/// The three kinds of macOS keyboard events the tap listens to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MacEventKind {
    KeyDown,
    KeyUp,
    FlagsChanged,
}

/// The raw fields the tap callback reads from a macOS keyboard event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MacKeyEvent {
    pub kind: MacEventKind,
    /// `kCGKeyboardEventKeycode`.
    pub keycode: u16,
    /// `CGEventGetFlags`: which modifiers are held after this event.
    pub flags: u64,
    /// `kCGKeyboardEventAutorepeat` (key-down only).
    pub autorepeat: bool,
}

/// Decode a macOS event into a key edge. Returns `None` for keys we do not track
/// (CapsLock, and flagsChanged events for unknown keys).
pub fn mac_key_input(event: MacKeyEvent) -> Option<KeyInput> {
    match event.kind {
        MacEventKind::FlagsChanged => {
            // A flagsChanged event says "the modifier with this keycode changed". Whether it
            // went down or up is read from its own flag bit in the new flags.
            let key = native_keys::key_by_code(event.keycode)?;
            if !key.is_modifier {
                return None;
            }
            let flag = key.device_flag?;
            Some(KeyInput {
                code: event.keycode,
                pressed: (event.flags & flag) != 0,
                autorepeat: false,
                class: KeyClass::Modifier,
            })
        }
        MacEventKind::KeyDown | MacEventKind::KeyUp => {
            let class = match native_keys::key_by_code(event.keycode) {
                Some(key) if key.is_modifier => return None,
                Some(key) if key.standalone => KeyClass::Standalone,
                _ => KeyClass::Typing,
            };
            Some(KeyInput {
                code: event.keycode,
                pressed: event.kind == MacEventKind::KeyDown,
                autorepeat: event.kind == MacEventKind::KeyDown && event.autorepeat,
                class,
            })
        }
    }
}

/// Side-specific modifiers whose held state disagrees with `flags`. Returns
/// `(keycode, now_pressed)` pairs. Fn is left out because its flag bit is also set on
/// arrow and function-key events that have nothing to do with the Fn key.
fn modifier_mismatches(held: &[u16], flags: u64) -> Vec<(u16, bool)> {
    native_keys::NATIVE_KEYS
        .iter()
        .filter(|key| key.is_modifier && key.name != "Fn")
        .filter_map(|key| {
            let flag = key.device_flag?;
            let down = (flags & flag) != 0;
            (down != held.contains(&key.code)).then_some((key.code, down))
        })
        .collect()
}

/// What the tap is doing: matching configured shortcuts, or capturing a new one.
pub enum TapLogic {
    Hotkeys(ChordMatcher),
    Capture(CaptureState),
}

/// Result of one tap callback: whether to swallow the event and what to send.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct TapOutput {
    pub swallow: bool,
    pub hotkey_events: Vec<NativeHotkeyEvent>,
    pub capture_events: Vec<ShortcutCaptureEvent>,
    /// Plan `typing-speed-and-nudge`: the event typed one keystroke of text that reaches the
    /// focused app (never set while capturing a shortcut).
    pub typed: bool,
}

impl TapLogic {
    fn held_codes(&self) -> Vec<u16> {
        match self {
            Self::Hotkeys(matcher) => matcher.held_codes(),
            Self::Capture(capture) => capture.held_codes(),
        }
    }

    fn feed(&mut self, input: KeyInput, output: &mut TapOutput) -> bool {
        match self {
            Self::Hotkeys(matcher) => matcher.handle(input, &mut output.hotkey_events),
            Self::Capture(capture) => {
                let (event, swallow) = capture.handle(input);
                output.capture_events.extend(event);
                swallow
            }
        }
    }

    /// Process one macOS keyboard event.
    ///
    /// Before the event itself, held modifiers are brought in line with the event's flags:
    /// a key event adds a modifier that is down but was never seen going down (for example
    /// Right Shift held before the tap started, then End pressed), and a flagsChanged event
    /// drops a modifier whose release was missed. Without this, one lost event would leave a
    /// chord stuck.
    pub fn process_mac_event(&mut self, event: MacKeyEvent) -> TapOutput {
        self.process_mac_event_during(event, false)
    }

    /// [`Self::process_mac_event`] while OraType may be sending its own ⌘C or ⌘V
    /// (`own_keystroke`, see [`super::OwnKeystroke`]). Then new key presses are not matched and reach
    /// the app untouched; releases still are, so no held key is left stuck.
    pub fn process_mac_event_during(
        &mut self,
        event: MacKeyEvent,
        own_keystroke: bool,
    ) -> TapOutput {
        let mut output = TapOutput::default();
        let held = self.held_codes();
        for (code, down) in modifier_mismatches(&held, event.flags) {
            let allowed = match event.kind {
                MacEventKind::FlagsChanged => !down && code != event.keycode,
                MacEventKind::KeyDown | MacEventKind::KeyUp => down,
            };
            if allowed && !(own_keystroke && down) {
                let input = KeyInput {
                    code,
                    pressed: down,
                    autorepeat: false,
                    class: KeyClass::Modifier,
                };
                let _ = self.feed(input, &mut output);
            }
        }

        if let Some(input) = mac_key_input(event) {
            if !(own_keystroke && input.pressed && !input.autorepeat) {
                output.swallow = self.feed(input, &mut output);
            }
        } else if matches!(self, Self::Capture(_)) && event.kind != MacEventKind::FlagsChanged {
            output.swallow = true;
        }
        output.typed = matches!(self, Self::Hotkeys(_))
            && crate::speed_stats::counts_as_typing(&event, output.swallow);
        output
    }

    /// The OS disabled the tap for a moment, so key events may have been lost.
    pub fn reset(&mut self) -> TapOutput {
        let mut output = TapOutput::default();
        match self {
            Self::Hotkeys(matcher) => matcher.reset(&mut output.hotkey_events),
            Self::Capture(capture) => capture.reset(),
        }
        output
    }
}

#[cfg(target_os = "macos")]
mod event_tap {
    use super::super::{
        own_keystroke_in_flight, CaptureState, ChordMatcher, KeystrokeObserver,
        NativeHotkeyHandler, ShortcutCaptureHandler,
    };
    use super::{MacEventKind, MacKeyEvent, TapLogic, TapOutput};
    use std::ffi::c_void;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{mpsc, Arc, Mutex};
    use std::thread;
    use std::time::Duration;

    const STARTUP_TIMEOUT: Duration = Duration::from_secs(3);

    type CgEventMask = u64;
    type CgEventType = u32;
    type CgEventTapLocation = u32;
    type CgEventTapPlacement = u32;
    type CgEventTapOptions = u32;
    type CgEventField = u32;
    type CgEventFlags = u64;
    type CfStringRef = *const c_void;
    type CfAllocatorRef = *const c_void;

    #[repr(C)]
    struct OpaqueCgEvent(c_void);
    type CgEventRef = *mut OpaqueCgEvent;

    #[repr(C)]
    struct OpaqueCfMachPort(c_void);
    type CfMachPortRef = *mut OpaqueCfMachPort;

    #[repr(C)]
    struct OpaqueCfRunLoop(c_void);
    type CfRunLoopRef = *mut OpaqueCfRunLoop;

    #[repr(C)]
    struct OpaqueCfRunLoopSource(c_void);
    type CfRunLoopSourceRef = *mut OpaqueCfRunLoopSource;

    // kCGSessionEventTap: see events for the whole login session.
    const SESSION_EVENT_TAP: CgEventTapLocation = 1;
    // kCGHeadInsertEventTap: run before other taps.
    const HEAD_INSERT: CgEventTapPlacement = 0;
    // kCGEventTapOptionDefault: an active tap that may change or drop events.
    const TAP_OPTION_DEFAULT: CgEventTapOptions = 0;

    const KEY_DOWN: CgEventType = 10;
    const KEY_UP: CgEventType = 11;
    const FLAGS_CHANGED: CgEventType = 12;
    // macOS disables a tap whose callback is too slow, or when secure input starts, and
    // tells the callback with these pseudo event types. We re-enable it.
    const TAP_DISABLED_BY_TIMEOUT: CgEventType = 0xFFFF_FFFE;
    const TAP_DISABLED_BY_USER_INPUT: CgEventType = 0xFFFF_FFFF;

    const KEYBOARD_EVENT_AUTOREPEAT: CgEventField = 8;
    const KEYBOARD_EVENT_KEYCODE: CgEventField = 9;
    // kCGEventSourceUnixProcessID: the process that posted the event (0 for real hardware).
    const EVENT_SOURCE_UNIX_PROCESS_ID: CgEventField = 41;

    type CgEventTapCallBack = extern "C" fn(
        proxy: *mut c_void,
        event_type: CgEventType,
        event: CgEventRef,
        user_info: *mut c_void,
    ) -> CgEventRef;

    #[link(name = "CoreGraphics", kind = "framework")]
    extern "C" {
        fn CGEventTapCreate(
            tap: CgEventTapLocation,
            place: CgEventTapPlacement,
            options: CgEventTapOptions,
            events_of_interest: CgEventMask,
            callback: CgEventTapCallBack,
            user_info: *mut c_void,
        ) -> CfMachPortRef;
        fn CGEventTapEnable(tap: CfMachPortRef, enable: bool);
        fn CGEventGetIntegerValueField(event: CgEventRef, field: CgEventField) -> i64;
        fn CGEventGetFlags(event: CgEventRef) -> CgEventFlags;
    }

    #[link(name = "CoreFoundation", kind = "framework")]
    extern "C" {
        fn CFMachPortCreateRunLoopSource(
            allocator: CfAllocatorRef,
            port: CfMachPortRef,
            order: isize,
        ) -> CfRunLoopSourceRef;
        fn CFRunLoopGetCurrent() -> CfRunLoopRef;
        fn CFRunLoopAddSource(rl: CfRunLoopRef, source: CfRunLoopSourceRef, mode: CfStringRef);
        fn CFRunLoopRun();
        fn CFRunLoopStop(rl: CfRunLoopRef);
        fn CFRelease(cf: *const c_void);
        static kCFRunLoopCommonModes: CfStringRef;
    }

    struct MacShutdownHandles {
        tap: Mutex<Option<CfMachPortRef>>,
        runloop: Mutex<Option<CfRunLoopRef>>,
        cancelled: AtomicBool,
    }

    unsafe impl Send for MacShutdownHandles {}
    unsafe impl Sync for MacShutdownHandles {}

    impl MacShutdownHandles {
        fn new() -> Self {
            Self {
                tap: Mutex::new(None),
                runloop: Mutex::new(None),
                cancelled: AtomicBool::new(false),
            }
        }

        fn shutdown(&self) {
            self.cancelled.store(true, Ordering::SeqCst);
            if let Some(tap) = self.tap.lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
                unsafe { CGEventTapEnable(*tap, false) };
            }
            if let Some(runloop) = self
                .runloop
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .as_ref()
            {
                unsafe { CFRunLoopStop(*runloop) };
            }
        }
    }

    pub struct PlatformNativeMonitor {
        handles: Arc<MacShutdownHandles>,
    }

    /// Where the tap sends what it finds.
    enum TapHandler {
        Hotkeys(NativeHotkeyHandler),
        Capture(ShortcutCaptureHandler),
    }

    impl TapHandler {
        fn dispatch(&self, output: TapOutput) {
            match self {
                Self::Hotkeys(handler) => {
                    for event in output.hotkey_events {
                        (handler.as_ref())(event);
                    }
                }
                Self::Capture(handler) => {
                    for event in output.capture_events {
                        (handler.as_ref())(event);
                    }
                }
            }
        }
    }

    impl PlatformNativeMonitor {
        pub fn start(matcher: ChordMatcher, handler: NativeHotkeyHandler) -> Result<Self, String> {
            let keystrokes = matcher.keystroke_observer();
            Self::start_tap(
                TapLogic::Hotkeys(matcher),
                TapHandler::Hotkeys(handler),
                keystrokes,
            )
        }

        pub fn start_capture(handler: ShortcutCaptureHandler) -> Result<Self, String> {
            Self::start_tap(
                TapLogic::Capture(CaptureState::default()),
                TapHandler::Capture(handler),
                None,
            )
        }

        fn start_tap(
            logic: TapLogic,
            handler: TapHandler,
            keystrokes: Option<KeystrokeObserver>,
        ) -> Result<Self, String> {
            let handles = Arc::new(MacShutdownHandles::new());
            let thread_handles = Arc::clone(&handles);
            let (status_tx, status_rx) = mpsc::channel();
            thread::Builder::new()
                .name("oratype-native-hotkey-mac".to_string())
                .spawn(move || {
                    run_event_tap_loop(logic, handler, keystrokes, thread_handles, status_tx)
                })
                .map_err(|error| {
                    format!("Failed to spawn macOS native hotkey monitor thread: {error}")
                })?;

            match status_rx.recv_timeout(STARTUP_TIMEOUT) {
                Ok(Ok(())) => Ok(Self { handles }),
                Ok(Err(error)) => {
                    handles.shutdown();
                    Err(error)
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    handles.shutdown();
                    Err(
                        "Timed out starting macOS native hotkey EventTap after 3 seconds"
                            .to_string(),
                    )
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    handles.shutdown();
                    Err(
                        "macOS native hotkey EventTap thread exited before startup completed"
                            .to_string(),
                    )
                }
            }
        }
    }

    impl Drop for PlatformNativeMonitor {
        fn drop(&mut self) {
            self.handles.shutdown();
        }
    }

    struct CallbackContext {
        logic: Mutex<TapLogic>,
        handler: TapHandler,
        keystrokes: Option<KeystrokeObserver>,
        handles: Arc<MacShutdownHandles>,
    }

    fn run_event_tap_loop(
        logic: TapLogic,
        handler: TapHandler,
        keystrokes: Option<KeystrokeObserver>,
        handles: Arc<MacShutdownHandles>,
        status_tx: mpsc::Sender<Result<(), String>>,
    ) {
        let context = Box::into_raw(Box::new(CallbackContext {
            logic: Mutex::new(logic),
            handler,
            keystrokes,
            handles: Arc::clone(&handles),
        }));
        let mask: CgEventMask = (1u64 << FLAGS_CHANGED) | (1u64 << KEY_DOWN) | (1u64 << KEY_UP);

        unsafe {
            let tap = CGEventTapCreate(
                SESSION_EVENT_TAP,
                HEAD_INSERT,
                TAP_OPTION_DEFAULT,
                mask,
                event_tap_callback,
                context as *mut c_void,
            );
            if tap.is_null() {
                drop(Box::from_raw(context));
                let _ = status_tx.send(Err(
                    "Failed to create macOS native hotkey EventTap; Accessibility permission may be denied"
                        .to_string(),
                ));
                return;
            }
            *handles.tap.lock().unwrap_or_else(|e| e.into_inner()) = Some(tap);

            let source = CFMachPortCreateRunLoopSource(std::ptr::null(), tap, 0);
            if source.is_null() {
                CGEventTapEnable(tap, false);
                *handles.tap.lock().unwrap_or_else(|e| e.into_inner()) = None;
                CFRelease(tap as *const c_void);
                drop(Box::from_raw(context));
                let _ = status_tx.send(Err(
                    "Failed to create macOS native hotkey EventTap run loop source".to_string(),
                ));
                return;
            }

            let runloop = CFRunLoopGetCurrent();
            *handles.runloop.lock().unwrap_or_else(|e| e.into_inner()) = Some(runloop);

            if handles.cancelled.load(Ordering::SeqCst) {
                CGEventTapEnable(tap, false);
                CFRelease(source as *const c_void);
                *handles.tap.lock().unwrap_or_else(|e| e.into_inner()) = None;
                *handles.runloop.lock().unwrap_or_else(|e| e.into_inner()) = None;
                CFRelease(tap as *const c_void);
                drop(Box::from_raw(context));
                let _ = status_tx.send(Err(
                    "macOS native hotkey EventTap startup was cancelled".to_string()
                ));
                return;
            }

            CFRunLoopAddSource(runloop, source, kCFRunLoopCommonModes);
            CFRelease(source as *const c_void);
            CGEventTapEnable(tap, true);

            if status_tx.send(Ok(())).is_err() || handles.cancelled.load(Ordering::SeqCst) {
                handles.shutdown();
            }

            if !handles.cancelled.load(Ordering::SeqCst) {
                CFRunLoopRun();
            }

            if let Some(tap) = handles.tap.lock().unwrap_or_else(|e| e.into_inner()).take() {
                CGEventTapEnable(tap, false);
                CFRelease(tap as *const c_void);
            }
            *handles.runloop.lock().unwrap_or_else(|e| e.into_inner()) = None;
            drop(Box::from_raw(context));
        }
    }

    /// Called by macOS for every key event. Kept thin: read the fields, let `TapLogic`
    /// decide, send the results, and return null to swallow the event.
    extern "C" fn event_tap_callback(
        _proxy: *mut c_void,
        event_type: CgEventType,
        event: CgEventRef,
        user_info: *mut c_void,
    ) -> CgEventRef {
        if user_info.is_null() {
            return event;
        }
        let context = unsafe { &*(user_info as *const CallbackContext) };

        let kind = match event_type {
            TAP_DISABLED_BY_TIMEOUT | TAP_DISABLED_BY_USER_INPUT => {
                if let Some(tap) = context
                    .handles
                    .tap
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .as_ref()
                {
                    unsafe { CGEventTapEnable(*tap, true) };
                }
                let output = context
                    .logic
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .reset();
                context.handler.dispatch(output);
                return event;
            }
            KEY_DOWN => MacEventKind::KeyDown,
            KEY_UP => MacEventKind::KeyUp,
            FLAGS_CHANGED => MacEventKind::FlagsChanged,
            _ => return event,
        };

        let keycode = unsafe { CGEventGetIntegerValueField(event, KEYBOARD_EVENT_KEYCODE) };
        let autorepeat = kind == MacEventKind::KeyDown
            && unsafe { CGEventGetIntegerValueField(event, KEYBOARD_EVENT_AUTOREPEAT) } != 0;
        let mac_event = MacKeyEvent {
            kind,
            keycode: u16::try_from(keycode).unwrap_or(u16::MAX),
            flags: unsafe { CGEventGetFlags(event) },
            autorepeat,
        };

        // Decide under the lock, send after releasing it.
        let output = context
            .logic
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .process_mac_event_during(mac_event, own_keystroke_in_flight());
        let swallow = output.swallow;
        // Plan `typing-speed-and-nudge`: count the keystroke (never which key), unless OraType
        // posted the event itself (Type-directly output).
        if output.typed {
            if let Some(observer) = context.keystrokes.as_ref() {
                let source_pid =
                    unsafe { CGEventGetIntegerValueField(event, EVENT_SOURCE_UNIX_PROCESS_ID) };
                if source_pid != i64::from(std::process::id()) {
                    (observer.as_ref())();
                }
            }
        }
        context.handler.dispatch(output);

        if swallow {
            std::ptr::null_mut()
        } else {
            event
        }
    }
}
