//! Windows foreground-window tracking and focus restoration around recording/paste.
//!
//! Showing the capsule overlay or racing with background UI transitions can move Win32
//! `GetForegroundWindow()` away from the text field the user was focused on when dictation
//! started. This module records the non-overlay foreground window before `Preparing` updates
//! the UI and restores it before text output if the foreground is missing or is one of our
//! overlays. An ordinary foreground window remains the paste target.

use std::sync::Mutex;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForegroundWindowInfo {
    pub hwnd: isize,
    pub pid: u32,
    pub thread_id: u32,
}

static LAST_NON_OVERLAY_FOREGROUND: Mutex<Option<ForegroundWindowInfo>> = Mutex::new(None);
#[cfg(target_os = "windows")]
static ACTIVE_RECORDING_TARGET: Mutex<Option<ForegroundWindowInfo>> = Mutex::new(None);

/// Pure decision helper for unit testing: returns `true` when we have a valid recorded
/// target window and the foreground is missing or belongs to one of our overlays.
#[cfg(any(target_os = "windows", test))]
fn should_attempt_focus_restore(
    recorded: Option<&ForegroundWindowInfo>,
    current: Option<&ForegroundWindowInfo>,
    current_is_overlay: bool,
) -> bool {
    let Some(target) = recorded else {
        return false;
    };
    target.hwnd != 0 && current.is_none_or(|fg| current_is_overlay && fg.hwnd != target.hwnd)
}

/// A window as the log names it: its handle and process id. The program's name is left out on
/// purpose: the log is a file users send with a bug report, and it must not record which apps
/// they dictate into (the macOS log does not name the app either).
pub fn format_foreground_summary(info: Option<&ForegroundWindowInfo>) -> String {
    match info {
        Some(fg) => format!("hwnd=0x{:x}, pid={}", fg.hwnd as usize, fg.pid),
        None => "hwnd=0x0, pid=0".to_string(),
    }
}

pub fn last_non_overlay_hwnd() -> Option<isize> {
    LAST_NON_OVERLAY_FOREGROUND
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .as_ref()
        .map(|info| info.hwnd)
}

#[cfg(target_os = "windows")]
pub fn remember_non_overlay_hwnd(hwnd: isize) {
    if hwnd == 0 || crate::overlay_window::is_own_overlay_hwnd(hwnd) {
        return;
    }
    if let Some(info) = unsafe { win::inspect_hwnd(hwnd as _) } {
        *LAST_NON_OVERLAY_FOREGROUND
            .lock()
            .unwrap_or_else(|e| e.into_inner()) = Some(info);
    }
}

#[cfg(not(target_os = "windows"))]
pub fn remember_non_overlay_hwnd(_hwnd: isize) {}

/// Captures the target window at recording start BEFORE showing `Preparing` capsule UI.
#[cfg(target_os = "windows")]
pub fn capture_recording_start_foreground() -> Option<ForegroundWindowInfo> {
    let fg = unsafe { win::find_non_overlay_foreground() }.or_else(|| {
        LAST_NON_OVERLAY_FOREGROUND
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
            .filter(|info| unsafe { win::matches_window(info) })
    });
    if let Some(info) = &fg {
        *LAST_NON_OVERLAY_FOREGROUND
            .lock()
            .unwrap_or_else(|e| e.into_inner()) = Some(info.clone());
    }
    *ACTIVE_RECORDING_TARGET
        .lock()
        .unwrap_or_else(|e| e.into_inner()) = fg.clone();
    tracing::info!(
        "Windows foreground at recording start: {}",
        format_foreground_summary(fg.as_ref())
    );
    fg
}

#[cfg(not(target_os = "windows"))]
pub fn capture_recording_start_foreground() -> Option<ForegroundWindowInfo> {
    None
}

/// Logs the current foreground window right before output/paste, and restores focus back
/// to the recording-start target when a Munia overlay gained focus or foreground is missing.
#[cfg(target_os = "windows")]
pub fn ensure_recording_target_foreground_before_paste() -> Option<ForegroundWindowInfo> {
    let recorded = ACTIVE_RECORDING_TARGET
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone();
    let current = unsafe { win::current_foreground_info() };
    tracing::info!(
        "Windows foreground before paste: current=({}) recorded=({})",
        format_foreground_summary(current.as_ref()),
        format_foreground_summary(recorded.as_ref())
    );

    let current_is_overlay = current
        .as_ref()
        .is_some_and(|fg| crate::overlay_window::is_own_overlay_hwnd(fg.hwnd));
    if !should_attempt_focus_restore(recorded.as_ref(), current.as_ref(), current_is_overlay) {
        return current;
    }

    let Some(target) = recorded else {
        return current;
    };
    if !unsafe { win::matches_window(&target) } {
        tracing::info!(
            "Windows focus restore skipped: recorded target hwnd=0x{:x} no longer belongs to the recording target",
            target.hwnd as usize
        );
        return current;
    }

    let restored = unsafe { win::restore_foreground_hwnd(&target) };
    std::thread::sleep(std::time::Duration::from_millis(30));
    let final_fg = unsafe { win::current_foreground_info() };
    tracing::info!(
        "Windows focus restore result: target=({}) restored={} final=({})",
        format_foreground_summary(Some(&target)),
        restored,
        format_foreground_summary(final_fg.as_ref())
    );
    final_fg
}

#[cfg(not(target_os = "windows"))]
pub fn ensure_recording_target_foreground_before_paste() -> Option<ForegroundWindowInfo> {
    None
}

#[cfg(target_os = "windows")]
mod win {
    use super::ForegroundWindowInfo;
    use std::ptr;
    use windows_sys::Win32::Foundation::HWND;
    use windows_sys::Win32::System::Threading::{AttachThreadInput, GetCurrentThreadId};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        BringWindowToTop, GetForegroundWindow, GetWindow, GetWindowThreadProcessId, IsWindow,
        IsWindowVisible, SetForegroundWindow, GW_HWNDNEXT,
    };

    pub(super) unsafe fn matches_window(info: &ForegroundWindowInfo) -> bool {
        inspect_hwnd(info.hwnd as HWND).as_ref() == Some(info)
    }

    pub(super) unsafe fn current_foreground_info() -> Option<ForegroundWindowInfo> {
        let hwnd = GetForegroundWindow();
        inspect_hwnd(hwnd)
    }

    pub(super) unsafe fn find_non_overlay_foreground() -> Option<ForegroundWindowInfo> {
        let fg = GetForegroundWindow();
        if !fg.is_null() && !crate::overlay_window::is_own_overlay_hwnd(fg as isize) {
            return inspect_hwnd(fg);
        }
        // If one of our own overlays somehow holds foreground, walk behind it in Z-order.
        let mut candidate = fg;
        for _ in 0..64 {
            candidate = if candidate.is_null() {
                break;
            } else {
                GetWindow(candidate, GW_HWNDNEXT)
            };
            if candidate.is_null() {
                break;
            }
            if IsWindowVisible(candidate) == 0 {
                continue;
            }
            if crate::overlay_window::is_own_overlay_hwnd(candidate as isize) {
                continue;
            }
            if let Some(info) = inspect_hwnd(candidate) {
                return Some(info);
            }
        }
        None
    }

    pub(super) unsafe fn inspect_hwnd(hwnd: HWND) -> Option<ForegroundWindowInfo> {
        if hwnd.is_null() || IsWindow(hwnd) == 0 {
            return None;
        }
        let mut pid = 0u32;
        let thread_id = GetWindowThreadProcessId(hwnd, &mut pid);
        if thread_id == 0 {
            return None;
        }
        Some(ForegroundWindowInfo {
            hwnd: hwnd as isize,
            pid,
            thread_id,
        })
    }

    pub(super) unsafe fn restore_foreground_hwnd(target: &ForegroundWindowInfo) -> bool {
        let target_hwnd = target.hwnd as HWND;
        if !matches_window(target) {
            return false;
        }
        if SetForegroundWindow(target_hwnd) != 0 && GetForegroundWindow() == target_hwnd {
            return true;
        }

        // Windows blocks `SetForegroundWindow` from background threads unless input queues
        // are temporarily attached to the current foreground / target threads.
        let cur_tid = GetCurrentThreadId();
        let fg_hwnd = GetForegroundWindow();
        let fg_tid = if !fg_hwnd.is_null() {
            GetWindowThreadProcessId(fg_hwnd, ptr::null_mut())
        } else {
            0
        };
        let target_tid = target.thread_id;

        let attached_cur = if target_tid != 0 && cur_tid != target_tid {
            AttachThreadInput(cur_tid, target_tid, 1) != 0
        } else {
            false
        };
        let attached_fg = if fg_tid != 0 && fg_tid != target_tid && fg_tid != cur_tid {
            AttachThreadInput(fg_tid, target_tid, 1) != 0
        } else {
            false
        };

        let ok_fg = SetForegroundWindow(target_hwnd) != 0;
        let _ = BringWindowToTop(target_hwnd);

        if attached_fg {
            let _ = AttachThreadInput(fg_tid, target_tid, 0);
        }
        if attached_cur {
            let _ = AttachThreadInput(cur_tid, target_tid, 0);
        }

        ok_fg || GetForegroundWindow() == target_hwnd
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn info(hwnd: isize) -> ForegroundWindowInfo {
        ForegroundWindowInfo {
            hwnd,
            pid: 42,
            thread_id: 100,
        }
    }

    #[test]
    fn the_log_summary_contains_only_handle_and_process_id() {
        assert_eq!(
            format_foreground_summary(Some(&info(0x1234))),
            "hwnd=0x1234, pid=42"
        );
        assert_eq!(format_foreground_summary(None), "hwnd=0x0, pid=0");
    }

    #[test]
    fn focus_restore_respects_the_current_app() {
        let recorded = info(0x1234);
        assert!(!should_attempt_focus_restore(
            Some(&recorded),
            Some(&recorded),
            false
        ));
        assert!(!should_attempt_focus_restore(
            Some(&recorded),
            Some(&info(0x9999)),
            false
        ));
    }

    #[test]
    fn focus_restore_triggers_when_an_overlay_or_no_window_has_focus() {
        let recorded = info(0x1234);
        assert!(should_attempt_focus_restore(
            Some(&recorded),
            Some(&info(0x9999)),
            true
        ));
        assert!(should_attempt_focus_restore(Some(&recorded), None, false));
    }

    #[test]
    fn focus_restore_requires_a_recorded_window() {
        let current = info(0x9999);
        assert!(!should_attempt_focus_restore(None, Some(&current), true));
        assert!(!should_attempt_focus_restore(
            Some(&info(0)),
            Some(&current),
            true
        ));
    }
}
