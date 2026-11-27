//! Win32 low-level keyboard hook. Key matching and shortcut capture live in the parent module.

use super::{
    CaptureState, ChordMatcher, KeyClass, KeyInput, NativeHotkeyHandler, ShortcutCaptureHandler,
};
use crate::native_keys;
use std::collections::BTreeSet;
use std::ptr;
use std::sync::atomic::{AtomicBool, AtomicPtr, AtomicU32, Ordering as AtomicOrdering};
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::Duration;
use windows_sys::Win32::Foundation::{GetLastError, LPARAM, LRESULT, WPARAM};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::System::Threading::GetCurrentThreadId;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, VK_CONTROL, VK_MENU};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, DispatchMessageW, GetMessageW, PeekMessageW, PostThreadMessageW,
    SetWindowsHookExW, TranslateMessage, UnhookWindowsHookEx, HC_ACTION, HHOOK, KBDLLHOOKSTRUCT,
    LLKHF_INJECTED, MSG, PM_NOREMOVE, WH_KEYBOARD_LL, WM_QUIT,
};

const STARTUP_TIMEOUT: Duration = Duration::from_secs(3);
const ACCEPT_SYNTHETIC_EVENTS_ENV: &str = "ORATYPE_ACCEPT_SYNTHETIC_HOTKEY_EVENTS";

const WM_KEYDOWN: usize = 0x0100;
const WM_KEYUP: usize = 0x0101;
const WM_SYSKEYDOWN: usize = 0x0104;
const WM_SYSKEYUP: usize = 0x0105;
const VK_RMENU: u16 = 0xA5;

static HOOK_CONTEXT: AtomicPtr<CallbackContext> = AtomicPtr::new(ptr::null_mut());

pub(super) struct PlatformNativeMonitor {
    thread_id: u32,
    thread: Option<thread::JoinHandle<()>>,
}

struct WindowsStartupState {
    thread_id: AtomicU32,
    cancelled: AtomicBool,
}

impl WindowsStartupState {
    fn new() -> Self {
        Self {
            thread_id: AtomicU32::new(0),
            cancelled: AtomicBool::new(false),
        }
    }

    fn cancel(&self) {
        self.cancelled.store(true, AtomicOrdering::SeqCst);
        let thread_id = self.thread_id.load(AtomicOrdering::SeqCst);
        if thread_id != 0 {
            let ok = unsafe { PostThreadMessageW(thread_id, WM_QUIT, 0, 0) };
            if ok == 0 {
                tracing::warn!(
                    "Failed to post WM_QUIT to cancelled Windows native hotkey hook thread: {}",
                    unsafe { GetLastError() }
                );
            }
        }
    }

    fn is_cancelled(&self) -> bool {
        self.cancelled.load(AtomicOrdering::SeqCst)
    }
}

enum HookMode {
    Hotkeys {
        matcher: ChordMatcher,
        handler: NativeHotkeyHandler,
    },
    Capture {
        capture: CaptureState,
        handler: ShortcutCaptureHandler,
    },
}

struct HookState {
    mode: HookMode,
    // `KBDLLHOOKSTRUCT` omits an autorepeat flag, so track currently depressed VK codes.
    held_vks: BTreeSet<u16>,
}

struct CallbackContext {
    state: Mutex<HookState>,
    hook: Mutex<Option<HHOOK>>,
}

unsafe impl Send for CallbackContext {}
unsafe impl Sync for CallbackContext {}

impl PlatformNativeMonitor {
    pub fn start(matcher: ChordMatcher, handler: NativeHotkeyHandler) -> Result<Self, String> {
        Self::spawn_hook(HookMode::Hotkeys { matcher, handler })
    }

    // TODO(windows): wire Settings `NativeHotkeyRecorder` UI on Windows so users can record native keys (`End`, `RightAlt`, side modifiers) in the frontend too.
    pub fn start_capture(handler: ShortcutCaptureHandler) -> Result<Self, String> {
        Self::spawn_hook(HookMode::Capture {
            capture: CaptureState::default(),
            handler,
        })
    }

    fn spawn_hook(mode: HookMode) -> Result<Self, String> {
        let (status_tx, status_rx) = mpsc::channel();
        let startup = Arc::new(WindowsStartupState::new());
        let thread_startup = Arc::clone(&startup);
        let thread = thread::Builder::new()
            .name("munia-native-hotkey-win".to_string())
            .spawn(move || run_keyboard_hook_loop(mode, thread_startup, status_tx))
            .map_err(|error| {
                format!("Failed to spawn Windows native hotkey monitor thread: {error}")
            })?;

        match status_rx.recv_timeout(STARTUP_TIMEOUT) {
            Ok(Ok(thread_id)) => Ok(Self {
                thread_id,
                thread: Some(thread),
            }),
            Ok(Err(error)) => {
                startup.cancel();
                let _ = thread.join();
                Err(error)
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                startup.cancel();
                let _ = thread.join();
                Err("Timed out starting Windows native hotkey hook after 3 seconds".to_string())
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                startup.cancel();
                let _ = thread.join();
                Err("Windows native hotkey hook thread exited before startup completed".to_string())
            }
        }
    }
}

impl Drop for PlatformNativeMonitor {
    fn drop(&mut self) {
        let ok = unsafe { PostThreadMessageW(self.thread_id, WM_QUIT, 0, 0) };
        if ok == 0 {
            tracing::warn!(
                "Failed to post WM_QUIT to Windows native hotkey hook thread: {}",
                unsafe { GetLastError() }
            );
        }
        if let Some(thread) = self.thread.take() {
            if thread.thread().id() != thread::current().id() && thread.join().is_err() {
                tracing::warn!("Windows native hotkey hook thread panicked during shutdown");
            }
        }
    }
}

fn run_keyboard_hook_loop(
    mode: HookMode,
    startup: Arc<WindowsStartupState>,
    status_tx: mpsc::Sender<Result<u32, String>>,
) {
    let thread_id = unsafe { GetCurrentThreadId() };

    unsafe {
        let mut message: MSG = std::mem::zeroed();
        let _ = PeekMessageW(&mut message, ptr::null_mut(), 0, 0, PM_NOREMOVE);
        startup.thread_id.store(thread_id, AtomicOrdering::SeqCst);

        if startup.is_cancelled() {
            let _ = status_tx.send(Err(
                "Windows native hotkey hook startup was cancelled before install".to_string(),
            ));
            return;
        }

        let context = Box::into_raw(Box::new(CallbackContext {
            state: Mutex::new(HookState {
                mode,
                held_vks: BTreeSet::new(),
            }),
            hook: Mutex::new(None),
        }));
        HOOK_CONTEXT.store(context, AtomicOrdering::SeqCst);

        // Global low-level hooks should be installed with the current module handle.
        // TODO(windows): Windows UIPI blocks `WH_KEYBOARD_LL` events when an elevated (Admin) window has focus unless the binary is Authenticode-signed with `uiAccess="true"`.
        let module = GetModuleHandleW(ptr::null());
        if module.is_null() {
            let error = GetLastError();
            let _ = HOOK_CONTEXT.compare_exchange(
                context,
                ptr::null_mut(),
                AtomicOrdering::SeqCst,
                AtomicOrdering::SeqCst,
            );
            drop(Box::from_raw(context));
            let _ = status_tx.send(Err(format!(
                "Failed to resolve Windows module handle for native hotkey hook: Win32 error {error}"
            )));
            return;
        }

        let hook = SetWindowsHookExW(WH_KEYBOARD_LL, Some(low_level_keyboard_proc), module, 0);
        if hook.is_null() {
            let _ = HOOK_CONTEXT.compare_exchange(
                context,
                ptr::null_mut(),
                AtomicOrdering::SeqCst,
                AtomicOrdering::SeqCst,
            );
            drop(Box::from_raw(context));
            let _ = status_tx.send(Err(format!(
                "Failed to install Windows native hotkey keyboard hook: Win32 error {}",
                GetLastError()
            )));
            return;
        }
        *(*context).hook.lock().unwrap_or_else(|e| e.into_inner()) = Some(hook);

        if startup.is_cancelled() {
            cleanup_context(context);
            let _ = status_tx.send(Err(
                "Windows native hotkey hook startup was cancelled after install".to_string(),
            ));
            return;
        }

        if status_tx.send(Ok(thread_id)).is_err() || startup.is_cancelled() {
            cleanup_context(context);
            return;
        }

        loop {
            let result = GetMessageW(&mut message, ptr::null_mut(), 0, 0);
            if result <= 0 {
                break;
            }
            let _ = TranslateMessage(&message);
            let _ = DispatchMessageW(&message);
        }

        cleanup_context(context);
    }
}

unsafe fn cleanup_context(context: *mut CallbackContext) {
    if let Some(hook) = (*context)
        .hook
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .take()
    {
        if UnhookWindowsHookEx(hook) == 0 {
            tracing::warn!(
                "Failed to unhook Windows native hotkey keyboard hook: {}",
                GetLastError()
            );
        }
    }
    let _ = HOOK_CONTEXT.compare_exchange(
        context,
        ptr::null_mut(),
        AtomicOrdering::SeqCst,
        AtomicOrdering::SeqCst,
    );
    drop(Box::from_raw(context));
}

unsafe extern "system" fn low_level_keyboard_proc(
    code: i32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if code == HC_ACTION as i32 && lparam != 0 {
        let keyboard = *(lparam as *const KBDLLHOOKSTRUCT);
        let injected = (keyboard.flags & LLKHF_INJECTED) != 0;
        // TODO(windows): European layouts emit a synthetic `VK_LCONTROL` (scan code `0x21D` / `LLKHF_ALTDOWN`) immediately before `VK_RMENU` (`AltGr`); verify filtering on real Windows ISO keyboards.
        if (!injected || accept_synthetic_events())
            && dispatch_keyboard_event(keyboard.vkCode, wparam)
        {
            return 1;
        }
    }

    CallNextHookEx(ptr::null_mut(), code, wparam, lparam)
}

fn key_class_for_windows_vk(key: &native_keys::WindowsKey) -> KeyClass {
    if key.code == VK_RMENU || key.standalone {
        KeyClass::Standalone
    } else if key.is_modifier {
        KeyClass::Modifier
    } else {
        KeyClass::Typing
    }
}

fn dispatch_keyboard_event(vk_code: u32, message: WPARAM) -> bool {
    let pressed = match message {
        WM_KEYDOWN | WM_SYSKEYDOWN => true,
        WM_KEYUP | WM_SYSKEYUP => false,
        _ => return false,
    };
    let Ok(code) = u16::try_from(vk_code) else {
        return false;
    };
    let key = native_keys::windows_key_by_code(code);
    let own_in_flight = super::own_keystroke_in_flight() && !accept_synthetic_events();

    let context = unsafe { callback_context() };
    let Some(context) = context else {
        return false;
    };

    let mut hotkey_events = Vec::new();
    let mut capture_events = Vec::new();
    let mut hotkey_handler = None;
    let mut capture_handler = None;
    let mut keystroke_observer = None;
    let swallow;

    {
        let mut state = context.state.lock().unwrap_or_else(|e| e.into_inner());
        let autorepeat = if pressed {
            !state.held_vks.insert(code)
        } else {
            state.held_vks.remove(&code);
            false
        };

        match &mut state.mode {
            HookMode::Hotkeys { matcher, handler } => {
                let Some(key) = key else {
                    return false;
                };
                if own_in_flight && pressed && !autorepeat {
                    swallow = false;
                } else {
                    let input = KeyInput {
                        code,
                        pressed,
                        autorepeat,
                        class: key_class_for_windows_vk(key),
                    };
                    swallow = matcher.handle(input, &mut hotkey_events);
                }
                if !hotkey_events.is_empty() {
                    hotkey_handler = Some(Arc::clone(handler));
                }
                let ctrl_or_alt_held = unsafe {
                    GetAsyncKeyState(VK_CONTROL as i32) < 0 || GetAsyncKeyState(VK_MENU as i32) < 0
                };
                if !swallow
                    && !own_in_flight
                    && pressed
                    && !autorepeat
                    && !ctrl_or_alt_held
                    && !key.is_modifier
                    && !key.standalone
                {
                    keystroke_observer = matcher.keystroke_observer();
                }
            }
            HookMode::Capture { capture, handler } => {
                if let Some(key) = key {
                    let input = KeyInput {
                        code,
                        pressed,
                        autorepeat,
                        class: if key.is_modifier {
                            KeyClass::Modifier
                        } else {
                            key_class_for_windows_vk(key)
                        },
                    };
                    let (event, should_swallow) = capture.handle_windows(input);
                    swallow = should_swallow;
                    if let Some(event) = event {
                        capture_events.push(event);
                        capture_handler = Some(Arc::clone(handler));
                    }
                } else {
                    swallow = true;
                }
            }
        }
    }

    if let Some(handler) = hotkey_handler {
        for event in hotkey_events {
            (handler.as_ref())(event);
        }
    }
    if let Some(handler) = capture_handler {
        for event in capture_events {
            (handler.as_ref())(event);
        }
    }
    if let Some(observer) = keystroke_observer {
        (observer.as_ref())();
    }
    swallow
}

unsafe fn callback_context<'a>() -> Option<&'a CallbackContext> {
    let ptr = HOOK_CONTEXT.load(AtomicOrdering::SeqCst);
    if ptr.is_null() {
        None
    } else {
        Some(&*ptr)
    }
}

fn accept_synthetic_events() -> bool {
    std::env::var(ACCEPT_SYNTHETIC_EVENTS_ENV).ok().as_deref() == Some("1")
}
