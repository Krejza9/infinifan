//! Background process: controls fans and lighting, owns the tray icon and the pipes to the window.
//! It loads no graphics, so it needs only a few MB and almost no CPU.
//! The window is a separate process (`--gui`) that exits completely when closed.

use std::os::windows::process::CommandExt;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use tray_icon::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tray_icon::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use windows::Win32::Foundation::{LPARAM, WPARAM};
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::WindowsAndMessaging::{DispatchMessageW, GetMessageW, MSG, PostThreadMessageW, TranslateMessage, WM_APP};

use crate::config::AppConfig;
use crate::ipc::{self, ToGui};
use crate::worker::{Command, Worker};

const WAKE: u32 = WM_APP + 1;
const QUIT_NONE: u8 = 0;
const QUIT_EXIT: u8 = 1;
const QUIT_ELEVATE: u8 = 2;
const DETACHED_PROCESS: u32 = 0x0000_0008;

fn wake(tid: u32) {
    unsafe {
        let _ = PostThreadMessageW(tid, WAKE, WPARAM(0), LPARAM(0));
    }
}

/// Opens the window; if it is already connected, just brings it to the front.
fn open_gui(client: &ipc::Client) {
    crate::system::allow_foreground();
    if let Some(tx) = client.lock().unwrap().as_ref() {
        let _ = tx.send(ToGui::Show);
        return;
    }
    if let Ok(exe) = std::env::current_exe()
        && let Err(e) = std::process::Command::new(exe).arg("--gui").creation_flags(DETACHED_PROCESS).spawn()
    {
        log::error!("Starting the window: {e}");
    }
}

/// `demo` = without a controller (tests only).
pub fn run(open_window: bool, demo: bool) {
    let cfg = AppConfig::load();
    crate::lang::apply(cfg.language.as_deref());
    crate::system::migrate_autostart();
    let shared = Arc::new(Mutex::new(cfg.clone()));
    let mut worker = Worker::spawn(cfg, demo, || {});
    let main_tid = unsafe { GetCurrentThreadId() };
    let quit = Arc::new(AtomicU8::new(QUIT_NONE));
    let open_requested = Arc::new(AtomicBool::new(false));
    let lang_changed = Arc::new(AtomicBool::new(false));
    let client: ipc::Client = Arc::new(Mutex::new(None));
    let dirty: Arc<Mutex<Option<Instant>>> = Arc::new(Mutex::new(None));

    // Commands from the window.
    {
        let (wtx, shared, quit, dirty, lang_changed) = (worker.tx.clone(), shared.clone(), quit.clone(), dirty.clone(), lang_changed.clone());
        let shared2 = shared.clone();
        ipc::serve(
            client.clone(),
            move |cmd| match cmd {
                Command::Config(c) => {
                    let mut old = shared.lock().unwrap();
                    if old.language != c.language {
                        crate::lang::apply(c.language.as_deref());
                        lang_changed.store(true, Ordering::SeqCst);
                        wake(main_tid);
                    }
                    *old = (*c).clone();
                    drop(old);
                    *dirty.lock().unwrap() = Some(Instant::now());
                    let _ = wtx.send(Command::Config(c));
                }
                Command::Quit => {
                    quit.store(QUIT_EXIT, Ordering::SeqCst);
                    wake(main_tid);
                }
                Command::RestartElevated => {
                    quit.store(QUIT_ELEVATE, Ordering::SeqCst);
                    wake(main_tid);
                }
                Command::Shutdown => {}
                other => {
                    let _ = wtx.send(other);
                }
            },
            move |tx| {
                let _ = tx.send(ToGui::Config(Box::new(shared2.lock().unwrap().clone())));
            },
        );
    }

    // Status to the window (only when connected) and debounced config saving.
    {
        let (status, client, shared, dirty) = (worker.status.clone(), client.clone(), shared.clone(), dirty.clone());
        std::thread::Builder::new()
            .name("status".into())
            .spawn(move || {
                loop {
                    std::thread::sleep(Duration::from_millis(500));
                    if let Some(tx) = client.lock().unwrap().as_ref() {
                        let _ = tx.send(ToGui::Status(Box::new(status.lock().unwrap().clone())));
                    }
                    let mut d = dirty.lock().unwrap();
                    if d.is_some_and(|t| t.elapsed() >= Duration::from_millis(800)) {
                        shared.lock().unwrap().save();
                        *d = None;
                    }
                }
            })
            .expect("spawn status thread");
    }

    // Tray icon.
    let menu = Menu::new();
    let open = MenuItem::new(tr!("tray.open"), true, None);
    let exit = MenuItem::new(tr!("tray.quit"), true, None);
    let _ = menu.append_items(&[&open, &PredefinedMenuItem::separator(), &exit]);
    let (open_id, exit_id) = (open.id().clone(), exit.id().clone());
    let tray = tray_icon::Icon::from_rgba(crate::icon::icon_rgba(32), 32, 32).ok().and_then(|icon| {
        TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_tooltip("InfiniFan")
            .with_icon(icon)
            .build()
            .map_err(|e| log::error!("Tray icon: {e}"))
            .ok()
    });
    {
        let o = open_requested.clone();
        TrayIconEvent::set_event_handler(Some(move |ev: TrayIconEvent| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. }
            | TrayIconEvent::DoubleClick { button: MouseButton::Left, .. } = ev
            {
                o.store(true, Ordering::SeqCst);
                wake(main_tid);
            }
        }));
        let (o, q) = (open_requested.clone(), quit.clone());
        MenuEvent::set_event_handler(Some(move |ev: MenuEvent| {
            if ev.id == open_id {
                o.store(true, Ordering::SeqCst);
            } else if ev.id == exit_id {
                q.store(QUIT_EXIT, Ordering::SeqCst);
            }
            wake(main_tid);
        }));
    }
    if open_window {
        open_requested.store(true, Ordering::SeqCst);
    }
    log::info!("Background process running");

    // Windows message loop (needed by the tray icon); sleeps otherwise.
    let mut msg = MSG::default();
    loop {
        if open_requested.swap(false, Ordering::SeqCst) {
            open_gui(&client);
        }
        if lang_changed.swap(false, Ordering::SeqCst) {
            open.set_text(tr!("tray.open"));
            exit.set_text(tr!("tray.quit"));
        }
        if quit.load(Ordering::SeqCst) != QUIT_NONE {
            break;
        }
        let r = unsafe { GetMessageW(&mut msg, None, 0, 0) };
        if !r.as_bool() {
            break;
        }
        unsafe {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }

    // Shutdown: close the window, save, stop control (nothing is sent; the controller keeps its last state).
    if let Some(tx) = client.lock().unwrap().as_ref() {
        let _ = tx.send(ToGui::Close);
    }
    shared.lock().unwrap().save();
    worker.shutdown();
    drop(tray);
    if quit.load(Ordering::SeqCst) == QUIT_ELEVATE {
        crate::system::relaunch_elevated("--background --open-gui --elevated-restart");
    }
    log::info!("Background process stopped");
}
