//! Control thread: the only code that talks to the device (like the L-Connect service).
//! Every second: temperatures → duty of the 4 groups → read RPM.

use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender, channel};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use hidapi::HidApi;

use crate::config::{AppConfig, ExternalControl};
use crate::device::{GROUP_COUNT, SlInfinity};
use crate::effects;
use crate::sensors::{CpuSensorStatus, Sensors, Temperatures};

const TICK: Duration = Duration::from_millis(1000);
const RECONNECT_EVERY: Duration = Duration::from_secs(3);
/// A gap between ticks longer than this means the PC resumed from sleep.
const RESUME_GAP: Duration = Duration::from_secs(6);
const RESUME_DELAY: Duration = Duration::from_secs(2);

#[derive(serde::Serialize, serde::Deserialize)]
pub enum Command {
    Config(Box<AppConfig>),
    Ping(usize),
    /// Reopen the sensors (after installing PawnIO).
    ReloadSensors,
    /// Restart the background process as administrator (background process only).
    RestartElevated,
    /// Stop control (background process only).
    Quit,
    Shutdown,
}

#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct Status {
    pub connected: bool,
    pub firmware: Option<String>,
    pub rpm: [u16; GROUP_COUNT],
    pub duty: [Option<u8>; GROUP_COUNT],
    pub temps: Temperatures,
    pub cpu_sensor: Option<CpuSensorStatus>,
    pub gpu_name: Option<String>,
    pub error: Option<String>,
    pub signalrgb: Option<String>,
}

pub struct Worker {
    pub tx: Sender<Command>,
    pub status: Arc<Mutex<Status>>,
    handle: Option<JoinHandle<()>>,
}

impl Worker {
    /// `demo` = no controller, made-up values (UI preview).
    pub fn spawn(cfg: AppConfig, demo: bool, on_update: impl Fn() + Send + 'static) -> Self {
        let (tx, rx) = channel();
        let status = Arc::new(Mutex::new(Status::default()));
        let st = status.clone();
        let handle = thread::Builder::new()
            .name("infinifan-worker".into())
            .spawn(move || {
                let mut l = Loop::new(cfg, rx, st, Box::new(on_update));
                l.demo = demo;
                l.run()
            })
            .expect("spawn control thread");
        Self { tx, status, handle: Some(handle) }
    }

}

/// The window's interface to control: a local thread (demo) or the background process over a pipe.
pub trait Backend {
    fn send(&self, cmd: Command);
    fn status(&self) -> Status;
}

impl Backend for Worker {
    fn send(&self, cmd: Command) {
        let _ = self.tx.send(cmd);
    }
    fn status(&self) -> Status {
        self.status.lock().unwrap().clone()
    }
}

impl Worker {
    /// Stops the control thread and waits for it.
    pub fn shutdown(&mut self) {
        let _ = self.tx.send(Command::Shutdown);
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        self.shutdown();
    }
}

struct Loop {
    cfg: AppConfig,
    rx: Receiver<Command>,
    status: Arc<Mutex<Status>>,
    on_update: Box<dyn Fn() + Send>,
    api: Option<HidApi>,
    dev: Option<SlInfinity>,
    sensors: Sensors,
    last_connect_try: Option<Instant>,
    signalrgb_done: Option<bool>,
    last_signalrgb_try: Option<Instant>,
    demo: bool,
}

impl Loop {
    fn new(cfg: AppConfig, rx: Receiver<Command>, status: Arc<Mutex<Status>>, on_update: Box<dyn Fn() + Send>) -> Self {
        let sensors = Sensors::new();
        {
            let mut s = status.lock().unwrap();
            s.cpu_sensor = Some(sensors.cpu_status.clone());
            s.gpu_name = sensors.gpu_name.clone();
        }
        Self {
            cfg,
            rx,
            status,
            on_update,
            api: None,
            dev: None,
            sensors,
            last_connect_try: None,
            signalrgb_done: None,
            last_signalrgb_try: None,
            demo: false,
        }
    }

    fn set_status(&self, f: impl FnOnce(&mut Status)) {
        f(&mut self.status.lock().unwrap());
        (self.on_update)();
    }

    fn run(mut self) {
        let mut next_tick = Instant::now();
        let mut last_tick = Instant::now();
        loop {
            let timeout = next_tick.saturating_duration_since(Instant::now());
            match self.rx.recv_timeout(timeout) {
                Ok(Command::Shutdown) | Err(RecvTimeoutError::Disconnected) => break,
                Ok(Command::Config(new)) => {
                    // When sliders are dragged quickly, only apply the latest config.
                    let mut latest = new;
                    let mut ping = None;
                    while let Ok(cmd) = self.rx.try_recv() {
                        match cmd {
                            Command::Config(c) => latest = c,
                            Command::Ping(g) => ping = Some(g),
                            Command::ReloadSensors => self.reload_sensors(),
                            Command::Shutdown => return,
                            Command::RestartElevated | Command::Quit => {}
                        }
                    }
                    self.apply_config(*latest);
                    if let Some(g) = ping {
                        self.ping(g);
                    }
                }
                Ok(Command::Ping(g)) => self.ping(g),
                Ok(Command::ReloadSensors) => self.reload_sensors(),
                Ok(Command::RestartElevated | Command::Quit) => {}
                Err(RecvTimeoutError::Timeout) => {
                    let now = Instant::now();
                    if now.duration_since(last_tick) > RESUME_GAP && self.dev.is_some() {
                        log::info!("Resumed from sleep – reinitializing the controller");
                        thread::sleep(RESUME_DELAY);
                        self.with_dev(|d, cfg| init_device(d, cfg, false));
                    }
                    last_tick = Instant::now();
                    self.tick();
                    next_tick = last_tick + TICK;
                }
            }
        }
        // Like L-Connect 3, nothing is sent on exit – the controller keeps its last state.
        log::info!("Control thread exiting");
    }

    /// Runs an action on the device; on error closes it (the next tick reconnects).
    fn with_dev(&mut self, f: impl FnOnce(&SlInfinity, &AppConfig) -> anyhow::Result<()>) {
        let Some(dev) = &self.dev else { return };
        if let Err(e) = f(dev, &self.cfg) {
            log::error!("Communication error: {e:#}");
            self.dev = None;
            let msg = format!("{e:#}");
            self.set_status(|s| {
                s.connected = false;
                s.error = Some(msg);
            });
        }
    }

    fn ensure_connected(&mut self) {
        if self.dev.is_some() || self.cfg.external.release_device {
            return;
        }
        if self.last_connect_try.is_some_and(|t| t.elapsed() < RECONNECT_EVERY) {
            return;
        }
        self.last_connect_try = Some(Instant::now());
        let api = match self.api.as_mut() {
            Some(api) => {
                let _ = api.refresh_devices();
                api
            }
            None => match HidApi::new() {
                Ok(api) => self.api.insert(api),
                Err(e) => {
                    let msg = e.to_string();
                    self.set_status(|s| s.error = Some(msg));
                    return;
                }
            },
        };
        match SlInfinity::open(api) {
            Ok(dev) => {
                let fw = dev.firmware_version().ok().map(|b| format_firmware(&b));
                log::info!("Controller connected, firmware {fw:?}");
                self.dev = Some(dev);
                self.set_status(|s| {
                    s.connected = true;
                    s.firmware = fw;
                    s.error = None;
                });
                self.with_dev(|d, cfg| init_device(d, cfg, true));
            }
            Err(e) => {
                let msg = format!("{e:#}");
                self.set_status(|s| {
                    s.connected = false;
                    s.error = Some(msg);
                });
            }
        }
    }

    fn tick(&mut self) {
        if self.demo {
            let t = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| (d.as_secs() % 600) as f32)
                .unwrap_or(0.0);
            let temps = Temperatures { cpu: Some(52.0 + 4.0 * (t * 0.3).sin()), gpu: Some(41.0) };
            let duty: [Option<u8>; GROUP_COUNT] = std::array::from_fn(|g| temps.for_source(self.cfg.fans[g].source).0.map(|x| self.cfg.fans[g].duty(x)));
            let rpm = std::array::from_fn(|g| if self.cfg.quantity[g] == 0 { 0 } else { duty[g].map(|d| d as u16 * 16).unwrap_or(0) });
            self.set_status(|s| {
                s.connected = true;
                s.firmware = Some("1.4 (demo)".into());
                s.temps = temps;
                s.duty = duty;
                s.rpm = rpm;
                s.error = None;
            });
            return;
        }
        self.ensure_connected();
        self.signalrgb_handshake();
        let temps = self.sensors.read();
        let mut duty = [None; GROUP_COUNT];
        if !self.cfg.external.mb_rpm_sync {
            for (g, fan) in self.cfg.fans.iter().enumerate() {
                duty[g] = temps.for_source(fan.source).0.map(|t| fan.duty(t));
            }
        }
        let mut rpm = None;
        self.with_dev(|d, _| {
            for (g, dt) in duty.iter().enumerate() {
                if let Some(dt) = dt {
                    d.set_fan_duty(g, *dt)?;
                }
            }
            rpm = Some(d.fan_speeds()?);
            Ok(())
        });
        let connected = self.dev.is_some();
        self.set_status(|s| {
            s.temps = temps;
            s.duty = duty;
            if let Some(r) = rpm {
                s.rpm = r;
            }
            if !connected {
                s.rpm = [0; GROUP_COUNT];
            }
        });
    }

    fn apply_config(&mut self, new: AppConfig) {
        if self.demo {
            self.cfg = new;
            return;
        }
        let old = std::mem::replace(&mut self.cfg, new);
        let (o, n) = (old.external.clone(), self.cfg.external.clone());

        if n.release_device && !o.release_device {
            log::info!("Releasing the device for other software");
            self.dev = None;
            self.set_status(|s| {
                s.connected = false;
                s.error = Some(tr!("light.blocked_released"));
            });
            return;
        }
        if !n.release_device && o.release_device {
            self.last_connect_try = None;
            self.ensure_connected();
            return;
        }
        if n.rgb_by_other_software != o.rgb_by_other_software {
            self.signalrgb_done = None;
            self.last_signalrgb_try = None;
            self.signalrgb_handshake();
        }

        let quantity_changed = old.quantity != self.cfg.quantity;
        let lighting_changed = old.lighting != self.cfg.lighting;
        let ext_changed = o != n;
        self.with_dev(|d, cfg| {
            if n.mb_rpm_sync != o.mb_rpm_sync {
                for g in 0..GROUP_COUNT {
                    d.set_fan_mb_sync(g, n.mb_rpm_sync)?;
                }
            }
            if n.mb_argb_sync != o.mb_argb_sync && !n.rgb_by_other_software {
                d.set_lighting_mb_sync(n.mb_argb_sync)?;
            }
            if quantity_changed && lighting_allowed(&n) {
                for (g, q) in cfg.quantity.iter().enumerate() {
                    d.set_quantity(g, *q)?;
                }
            }
            let lighting_reenabled = ext_changed && lighting_allowed(&n) && !lighting_allowed(&o);
            if lighting_allowed(&n) && (lighting_changed || quantity_changed || lighting_reenabled) {
                effects::apply(d, &cfg.lighting)?;
            }
            Ok(())
        });
    }

    fn reload_sensors(&mut self) {
        self.sensors = Sensors::new();
        let (cpu, gpu) = (self.sensors.cpu_status.clone(), self.sensors.gpu_name.clone());
        self.set_status(|s| {
            s.cpu_sensor = Some(cpu);
            s.gpu_name = gpu;
        });
    }

    fn ping(&mut self, group: usize) {
        if self.demo {
            return;
        }
        if !lighting_allowed(&self.cfg.external) || group >= GROUP_COUNT {
            return;
        }
        // Like L-Connect 3: identify effect for 2.5 s, then restore the lighting.
        self.with_dev(|d, _| d.ping(group, 0, 2));
        thread::sleep(Duration::from_millis(2500));
        self.with_dev(|d, cfg| effects::apply(d, &cfg.lighting));
    }

    /// Like L-Connect 3: when SignalRGB is enabled, grant it lighting control through its local API.
    fn signalrgb_handshake(&mut self) {
        let want = self.cfg.external.rgb_by_other_software;
        if self.signalrgb_done == Some(want) {
            return;
        }
        if self.last_signalrgb_try.is_some_and(|t| t.elapsed() < Duration::from_secs(10)) {
            return;
        }
        self.last_signalrgb_try = Some(Instant::now());
        let body = if want { r#"{"fans":"denied","lighting":"allowed"}"# } else { r#"{"fans":"denied","lighting":"denied"}"# };
        let result = signalrgb_patch(body);
        let msg = match &result {
            Ok(()) => Some(if want { tr!("srgb.granted") } else { tr!("srgb.revoked") }),
            Err(_) if want => Some(tr!("srgb.not_running")),
            Err(_) => None,
        };
        // If SignalRGB is not running and the switch is off, there is nothing to revoke.
        if result.is_ok() || !want {
            self.signalrgb_done = Some(want);
        }
        self.set_status(|s| s.signalrgb = msg);
    }
}

fn lighting_allowed(e: &ExternalControl) -> bool {
    !e.mb_argb_sync && !e.rgb_by_other_software && !e.release_device
}

/// Initialization like L-Connect 3 at start and after resume; the sync state
/// with the motherboard is always sent (L-Connect 3 only sent "on", so turning it off did not survive a restart).
fn init_device(d: &SlInfinity, cfg: &AppConfig, first: bool) -> anyhow::Result<()> {
    let e = &cfg.external;
    if !e.rgb_by_other_software {
        for (g, q) in cfg.quantity.iter().enumerate() {
            d.set_quantity(g, *q)?;
        }
    }
    d.set_merge_order([0, 1, 2, 3])?;
    if !e.rgb_by_other_software {
        d.set_lighting_mb_sync(e.mb_argb_sync)?;
    }
    if first || e.mb_rpm_sync {
        for g in 0..GROUP_COUNT {
            d.set_fan_mb_sync(g, e.mb_rpm_sync)?;
        }
    }
    if lighting_allowed(e) {
        effects::apply(d, &cfg.lighting)?;
    }
    Ok(())
}

/// Decodes the firmware version.
fn format_firmware(b: &[u8]) -> String {
    if b.len() < 5 || b[..4] != [0xE0, 0x50, 0x80, 0xC4] {
        return format!("{b:02X?}");
    }
    let fine = b[4] as u32;
    let (hi, mut lo) = (fine >> 4, fine & 0xF);
    if fine <= 3 || (15..=31).contains(&fine) || (fine >= 32 && lo > 9) {
        return "0.0".into();
    }
    if fine == 13 || fine == 14 {
        lo += 1;
    }
    let v = hi * 10 + lo;
    format!("{}.{}", v / 10, v % 10)
}

fn signalrgb_patch(body: &str) -> std::io::Result<()> {
    // Partner ID of the L-Connect integration in SignalRGB (the same one L-Connect 3 sends).
    const PATH: &str = "/api/v1/partners/01e8be4fb463f2b928deb48a39273d7a";
    let addr = "127.0.0.1:16038".parse().unwrap();
    let mut s = TcpStream::connect_timeout(&addr, Duration::from_millis(300))?;
    s.set_read_timeout(Some(Duration::from_secs(2)))?;
    write!(
        s,
        "PATCH {PATH} HTTP/1.1\r\nHost: localhost:16038\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )?;
    let mut resp = [0u8; 64];
    let n = s.read(&mut resp)?;
    let line = String::from_utf8_lossy(&resp[..n]);
    if line.starts_with("HTTP/1.1 2") || line.starts_with("HTTP/1.0 2") {
        Ok(())
    } else {
        Err(std::io::Error::other(line.lines().next().unwrap_or_default().to_string()))
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn firmware() {
        assert_eq!(super::format_firmware(&[0xE0, 0x50, 0x80, 0xC4, 0x0D]), "1.4");
        assert_eq!(super::format_firmware(&[0xE0, 0x50, 0x80, 0xC4, 0x21]), "2.1");
    }
}
