//! Windows integration: single instance, autostart via Task Scheduler, run as administrator.

use std::os::windows::process::CommandExt;
use std::process::Command;

use std::sync::Mutex;

use windows::Win32::Foundation::{CloseHandle, ERROR_ALREADY_EXISTS, GetLastError, HANDLE};
use windows::Win32::System::Threading::{CreateMutexW, OpenMutexW, SYNCHRONIZATION_SYNCHRONIZE};
use windows::Win32::UI::Shell::ShellExecuteW;
use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
use windows::core::{HSTRING, w};

const TASK_NAME: &str = "InfiniFan";
/// Autostart task of the program's former name (L-Connect RS).
const OLD_TASK_NAME: &str = "LConnect-RS";

/// Kernel object name; the INFINIFAN_TEST_SUFFIX variable (tests only) keeps it distinct from a running instance.
pub fn object_name(base: &str) -> String {
    format!("{base}{}", std::env::var("INFINIFAN_TEST_SUFFIX").unwrap_or_default())
}
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// Named single-instance mutex, held for the whole lifetime of the process.
struct SingleInstance(HANDLE);

// The mutex handle is a kernel object; it is only used under the lock below.
unsafe impl Send for SingleInstance {}

static INSTANCE: Mutex<Option<SingleInstance>> = Mutex::new(None);

/// Takes the single-instance lock; `false` = another instance is already running.
pub fn acquire_single_instance() -> bool {
    unsafe {
        let Ok(h) = CreateMutexW(None, false, &HSTRING::from(object_name("Local\\InfiniFan-single-instance"))) else { return false };
        if GetLastError() == ERROR_ALREADY_EXISTS {
            let _ = CloseHandle(h);
            return false;
        }
        *INSTANCE.lock().unwrap() = Some(SingleInstance(h));
        true
    }
}

/// Is the background process running (holding the single-instance lock)?
pub fn background_running() -> bool {
    unsafe {
        match OpenMutexW(SYNCHRONIZATION_SYNCHRONIZE, false, &HSTRING::from(object_name("Local\\InfiniFan-single-instance"))) {
            Ok(h) => {
                let _ = CloseHandle(h);
                true
            }
            Err(_) => false,
        }
    }
}

/// Window lock – only one window may be open. Held until the process exits.
pub fn acquire_gui_instance() -> bool {
    unsafe {
        let Ok(h) = CreateMutexW(None, false, &HSTRING::from(object_name("Local\\InfiniFan-gui"))) else { return false };
        if GetLastError() == ERROR_ALREADY_EXISTS {
            let _ = CloseHandle(h);
            return false;
        }
        std::mem::forget(SingleInstance(h));
        true
    }
}

/// Starts the background process as a separate process (with arguments, e.g. `--open-gui`).
pub fn spawn_background(extra: &[&str]) -> bool {
    const DETACHED_PROCESS: u32 = 0x0000_0008;
    let Ok(exe) = std::env::current_exe() else { return false };
    Command::new(exe).arg("--background").args(extra).creation_flags(DETACHED_PROCESS).spawn().is_ok()
}

/// Releases the lock (before starting a new instance, e.g. as administrator).
pub fn release_single_instance() {
    if let Some(i) = INSTANCE.lock().unwrap().take() {
        unsafe {
            let _ = CloseHandle(i.0);
        }
    }
}

fn schtasks(args: &[&str]) -> std::io::Result<std::process::Output> {
    Command::new("schtasks").args(args).creation_flags(CREATE_NO_WINDOW).output()
}

pub fn autostart_enabled() -> bool {
    schtasks(&["/Query", "/TN", TASK_NAME]).map(|o| o.status.success()).unwrap_or(false)
}

/// A task with highest privileges starts the program at logon without a UAC prompt
/// (required for reading the CPU temperature via PawnIO). It can only be created as administrator.
pub fn set_autostart(enable: bool) -> Result<(), String> {
    let out = if enable {
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        let tr = format!("\"{}\" --background", exe.display());
        schtasks(&["/Create", "/F", "/TN", TASK_NAME, "/SC", "ONLOGON", "/RL", "HIGHEST", "/TR", &tr])
    } else {
        schtasks(&["/Delete", "/F", "/TN", TASK_NAME])
    }
    .map_err(|e| e.to_string())?;
    if out.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).trim().to_string())
    }
}

/// Replaces the autostart task of the former name with one for this exe (needs administrator;
/// otherwise the old task stays until the user toggles autostart in Settings).
pub fn migrate_autostart() {
    let old = schtasks(&["/Query", "/TN", OLD_TASK_NAME]).map(|o| o.status.success()).unwrap_or(false);
    if !old || !crate::sensors::is_elevated() {
        return;
    }
    match set_autostart(true) {
        Ok(()) => {
            let _ = schtasks(&["/Delete", "/F", "/TN", OLD_TASK_NAME]);
            log::info!("Autostart task migrated from {OLD_TASK_NAME} to {TASK_NAME}");
        }
        Err(e) => log::warn!("Autostart migration failed: {e}"),
    }
}

/// Error dialog for problems that stop the program before any window exists.
pub fn error_box(title: &str, text: &str) {
    use windows::Win32::UI::WindowsAndMessaging::{MB_ICONERROR, MB_OK, MessageBoxW};
    unsafe {
        MessageBoxW(None, &HSTRING::from(text), &HSTRING::from(title), MB_OK | MB_ICONERROR);
    }
}

/// Restarts the program as administrator (UAC) with the given arguments. Returns true if the launch succeeded.
pub fn relaunch_elevated(args: &str) -> bool {
    let Ok(exe) = std::env::current_exe() else { return false };
    let exe = HSTRING::from(exe.as_os_str());
    // The new instance shouldn't have to wait for this one to exit.
    release_single_instance();
    let ok = unsafe { ShellExecuteW(None, w!("runas"), &exe, &HSTRING::from(args), None, SW_SHOWNORMAL).0 as usize > 32 };
    if !ok {
        acquire_single_instance();
    }
    ok
}


/// Dark window title bar right at creation (the window is still hidden). Otherwise egui sets it only after the first
/// frame and Windows 10 doesn't repaint an inactive title bar – a window opened from the tray had a white bar until clicked.
pub fn dark_title_bar(cc: &eframe::CreationContext<'_>) {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use windows::Win32::Foundation::{HWND, TRUE};
    use windows::Win32::Graphics::Dwm::{DWMWA_USE_IMMERSIVE_DARK_MODE, DwmSetWindowAttribute};
    let Ok(RawWindowHandle::Win32(h)) = cc.window_handle().map(|w| w.as_raw()) else { return };
    let on = TRUE;
    unsafe {
        let _ = DwmSetWindowAttribute(HWND(h.hwnd.get() as _), DWMWA_USE_IMMERSIVE_DARK_MODE, (&on as *const windows::core::BOOL).cast(), size_of_val(&on) as u32);
    }
}

/// Allows a window (of another process) to take the foreground – called by the background process after a tray click.
pub fn allow_foreground() {
    use windows::Win32::UI::WindowsAndMessaging::{ASFW_ANY, AllowSetForegroundWindow};
    unsafe {
        let _ = AllowSetForegroundWindow(ASFW_ANY);
    }
}
