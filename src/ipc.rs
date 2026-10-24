//! Communication between the window and the background process over two local named pipes
//! (one-way, so a blocking read never prevents a write):
//!   `infinifan-cmd`: window → background, commands (`worker::Command`),
//!   `infinifan-evt`: background → window, settings and status (`ToGui`).
//! Messages are line-delimited JSON. The pipes only accept local clients of the logged-on user.

use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use std::os::windows::io::FromRawHandle;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Sender, channel};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use windows::Win32::Foundation::{ERROR_PIPE_CONNECTED, HLOCAL, LocalFree};
use windows::Win32::Security::Authorization::{ConvertStringSecurityDescriptorToSecurityDescriptorW, SDDL_REVISION_1};
use windows::Win32::Security::{PSECURITY_DESCRIPTOR, SECURITY_ATTRIBUTES};
use windows::Win32::Storage::FileSystem::{FILE_FLAGS_AND_ATTRIBUTES, PIPE_ACCESS_INBOUND, PIPE_ACCESS_OUTBOUND};
use windows::Win32::System::Pipes::{
    ConnectNamedPipe, CreateNamedPipeW, PIPE_READMODE_BYTE, PIPE_REJECT_REMOTE_CLIENTS, PIPE_TYPE_BYTE, PIPE_WAIT,
};
use windows::core::{HSTRING, w};

use crate::config::AppConfig;
use crate::worker::{Backend, Command, Status};

fn cmd_pipe() -> String {
    crate::system::object_name(r"\\.\pipe\infinifan-cmd")
}

fn evt_pipe() -> String {
    crate::system::object_name(r"\\.\pipe\infinifan-evt")
}

/// Messages from the background process to the window.
#[derive(Serialize, Deserialize)]
pub enum ToGui {
    Config(Box<AppConfig>),
    Status(Box<Status>),
    /// Bring the window to the foreground (tray icon clicked while it is already open).
    Show,
    /// The background process is exiting – close the window.
    Close,
}

fn send_line<T: Serialize>(f: &mut File, msg: &T) -> std::io::Result<()> {
    let mut line = serde_json::to_vec(msg).map_err(std::io::Error::other)?;
    line.push(b'\n');
    f.write_all(&line)?;
    f.flush()
}

// ---------------------------------------------------------------- server (background process)

/// Creates a pipe instance; access: SYSTEM and administrators full, interactive users read/write.
fn create_pipe(name: &str, inbound: bool) -> std::io::Result<File> {
    unsafe {
        let mut sd = PSECURITY_DESCRIPTOR::default();
        ConvertStringSecurityDescriptorToSecurityDescriptorW(
            w!("D:(A;;GA;;;SY)(A;;GA;;;BA)(A;;GRGW;;;IU)"),
            SDDL_REVISION_1,
            &mut sd,
            None,
        )
        .map_err(std::io::Error::other)?;
        let sa = SECURITY_ATTRIBUTES {
            nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: sd.0,
            bInheritHandle: false.into(),
        };
        let mode: FILE_FLAGS_AND_ATTRIBUTES = if inbound { PIPE_ACCESS_INBOUND } else { PIPE_ACCESS_OUTBOUND };
        let h = CreateNamedPipeW(
            &HSTRING::from(name),
            mode,
            PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT | PIPE_REJECT_REMOTE_CLIENTS,
            1,
            64 * 1024,
            64 * 1024,
            0,
            Some(&sa),
        );
        let _ = LocalFree(Some(HLOCAL(sd.0)));
        if h.is_invalid() {
            return Err(std::io::Error::last_os_error());
        }
        Ok(File::from_raw_handle(h.0))
    }
}

fn wait_for_client(f: &File) -> std::io::Result<()> {
    use std::os::windows::io::AsRawHandle;
    let h = windows::Win32::Foundation::HANDLE(f.as_raw_handle());
    match unsafe { ConnectNamedPipe(h, None) } {
        Ok(()) => Ok(()),
        Err(e) if e.code() == ERROR_PIPE_CONNECTED.to_hresult() => Ok(()),
        Err(e) => Err(std::io::Error::other(e)),
    }
}

/// The currently connected window (for sending messages from other threads).
pub type Client = Arc<Mutex<Option<Sender<ToGui>>>>;

/// Starts the background server. `on_cmd` receives window commands, `on_connect` is called after a connection
/// (typically sends the current settings).
pub fn serve(client: Client, on_cmd: impl Fn(Command) + Send + 'static, on_connect: impl Fn(&Sender<ToGui>) + Send + 'static) {
    std::thread::Builder::new()
        .name("ipc-server".into())
        .spawn(move || {
            loop {
                let (cmd, mut evt) = match (create_pipe(&cmd_pipe(), true), create_pipe(&evt_pipe(), false)) {
                    (Ok(c), Ok(e)) => (c, e),
                    (Err(e), _) | (_, Err(e)) => {
                        log::error!("Window pipe: {e}");
                        std::thread::sleep(Duration::from_secs(5));
                        continue;
                    }
                };
                if wait_for_client(&cmd).and_then(|_| wait_for_client(&evt)).is_err() {
                    continue;
                }
                log::info!("Window connected");
                let (tx, rx) = channel::<ToGui>();
                on_connect(&tx);
                *client.lock().unwrap() = Some(tx);
                let writer = std::thread::spawn(move || {
                    for msg in rx {
                        if send_line(&mut evt, &msg).is_err() {
                            break;
                        }
                    }
                });
                for line in BufReader::new(cmd).lines() {
                    let Ok(line) = line else { break };
                    match serde_json::from_str::<Command>(&line) {
                        Ok(c) => on_cmd(c),
                        Err(e) => log::warn!("Invalid command from window: {e}"),
                    }
                }
                // Window disconnected: drop the sender, the writer thread finishes.
                *client.lock().unwrap() = None;
                let _ = writer.join();
                log::info!("Window disconnected");
            }
        })
        .expect("pipe thread");
}

// ---------------------------------------------------------------- client (window)

pub struct Connection {
    cmd: File,
    evt: File,
}

/// Connects to the background process (waits up to `timeout`).
pub fn connect(timeout: Duration) -> Option<Connection> {
    let start = Instant::now();
    loop {
        let cmd = std::fs::OpenOptions::new().write(true).open(cmd_pipe());
        if let Ok(cmd) = cmd {
            // The evt instance is created together with cmd; retry briefly just in case.
            for _ in 0..50 {
                if let Ok(evt) = std::fs::OpenOptions::new().read(true).open(evt_pipe()) {
                    return Some(Connection { cmd, evt });
                }
                std::thread::sleep(Duration::from_millis(20));
            }
        }
        if start.elapsed() >= timeout {
            return None;
        }
        std::thread::sleep(Duration::from_millis(150));
    }
}

/// Window's link to the background process – implements the same interface as the local thread.
pub struct RemoteLink {
    cmd: Mutex<File>,
    status: Arc<Mutex<Status>>,
    /// Settings sent by the background process (the window adopts them).
    pub config: Arc<Mutex<Option<AppConfig>>>,
    pub show: Arc<AtomicBool>,
    pub close: Arc<AtomicBool>,
}

impl RemoteLink {
    pub fn new(conn: Connection, on_update: impl Fn() + Send + 'static) -> Self {
        let status = Arc::new(Mutex::new(Status::default()));
        let config = Arc::new(Mutex::new(None));
        let show = Arc::new(AtomicBool::new(false));
        let close = Arc::new(AtomicBool::new(false));
        let (st, cf, sh, cl) = (status.clone(), config.clone(), show.clone(), close.clone());
        let evt = conn.evt;
        std::thread::Builder::new()
            .name("ipc-client".into())
            .spawn(move || {
                for line in BufReader::new(evt).lines() {
                    let Ok(line) = line else { break };
                    match serde_json::from_str::<ToGui>(&line) {
                        Ok(ToGui::Status(s)) => *st.lock().unwrap() = *s,
                        Ok(ToGui::Config(c)) => *cf.lock().unwrap() = Some(*c),
                        Ok(ToGui::Show) => sh.store(true, Ordering::SeqCst),
                        Ok(ToGui::Close) => cl.store(true, Ordering::SeqCst),
                        Err(e) => log::warn!("Invalid message from background: {e}"),
                    }
                    on_update();
                }
                // The background process has exited.
                cl.store(true, Ordering::SeqCst);
                on_update();
            })
            .expect("client thread");
        Self { cmd: Mutex::new(conn.cmd), status, config, show, close }
    }
}

impl Backend for RemoteLink {
    fn send(&self, cmd: Command) {
        if matches!(cmd, Command::Shutdown) {
            return;
        }
        if let Err(e) = send_line(&mut self.cmd.lock().unwrap(), &cmd) {
            log::warn!("Sending to background process: {e}");
        }
    }

    fn status(&self) -> Status {
        self.status.lock().unwrap().clone()
    }

}
