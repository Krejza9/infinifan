use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use eframe::egui::{self, Color32, CornerRadius, Frame, Margin, RichText, Stroke, ViewportCommand};
use egui_plot::{HoverPosition, Line, LineStyle, Plot, PlotPoint, PlotPoints, Points, VLine};

use crate::config::AppConfig;
use crate::curve::{FanGroupConfig, FanSize, Preset, Source};
use crate::device::GROUP_COUNT;
use crate::effects::{self, EffectSetting, ModeDef, Scope};
use crate::pawnio_setup;
use crate::preview::FanPreview;
use crate::sensors::{CpuSensorStatus, is_elevated};
use crate::system;
use crate::theme::{self, ACCENT, ACCENT2, BG, BORDER, CARD, ERR, MUTED, OK, SIDEBAR, TEXT, WARN};
use crate::icons;
use crate::widgets::{self, banner, card, card_with, field_label, segmented, stat_tile, toggle_row, with_icon};
use crate::ipc;
use crate::lang;
use crate::worker::{Backend, Command, Status, Worker};

const SAVE_DELAY: Duration = Duration::from_millis(800);
const FAN_COLUMN: f32 = 190.0;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
    Fans,
    Lighting,
    External,
    Settings,
    About,
}

/// Flags shared with the connection to the background process.
struct RemoteFlags {
    config: Arc<std::sync::Mutex<Option<AppConfig>>>,
    show: Arc<AtomicBool>,
    close: Arc<AtomicBool>,
}

pub struct App {
    cfg: AppConfig,
    worker: Box<dyn Backend>,
    status: Status,
    tab: Tab,
    group: usize,
    light_all_groups: bool,
    /// Lighting draft – sent to the fans only after Save.
    light_draft: crate::effects::LightingConfig,
    dragging: Option<usize>,
    dirty_since: Option<Instant>,
    autostart: bool,
    message: Option<String>,
    elevated: bool,
    preview: FanPreview,
    /// Connection to the background process (None = demo with a local thread).
    remote: Option<RemoteFlags>,
    /// Request to restart control with administrator rights.
    quit_requested: Arc<AtomicBool>,
    quitting: bool,
    demo: bool,
    pawnio: PawnDialog,
    pawnio_checked: bool,
    pawnio_job: Option<std::sync::mpsc::Receiver<Result<(), String>>>,
}

/// Dialog instalace PawnIO.
#[derive(Clone, PartialEq)]
enum PawnDialog {
    Hidden,
    Ask,
    Running(bool),
    Done(bool, Result<(), String>),
}

impl App {
    /// `conn` = connection to the background process; `None` = demo without a controller (local thread).
    pub fn new(cc: &eframe::CreationContext<'_>, cfg: AppConfig, conn: Option<ipc::Connection>) -> Self {
        let demo = conn.is_none();
        theme::apply(&cc.egui_ctx);
        system::dark_title_bar(cc);
        // Otherwise a window opened from the tray stays inactive behind other windows.
        cc.egui_ctx.send_viewport_cmd(ViewportCommand::Focus);

        let ctx = cc.egui_ctx.clone();
        let (worker, remote): (Box<dyn Backend>, Option<RemoteFlags>) = match conn {
            Some(conn) => {
                let link = ipc::RemoteLink::new(conn, move || ctx.request_repaint());
                let flags = RemoteFlags { config: link.config.clone(), show: link.show.clone(), close: link.close.clone() };
                (Box::new(link), Some(flags))
            }
            None => (Box::new(Worker::spawn(cfg.clone(), true, move || ctx.request_repaint())), None),
        };

        let quit_requested = Arc::new(AtomicBool::new(false));
        let group = (0..GROUP_COUNT).find(|g| cfg.quantity[*g] > 0).unwrap_or(0);
        // In demo mode INFINIFAN_DEMO_TAB picks the start tab (screenshots).
        // In demo mode INFINIFAN_DEMO_DIRTY simulates an unsaved lighting change (Save/Cancel bar screenshot).
        let mut cfg = cfg;
        if cfg.saved_colors.is_empty() {
            // The "My colors" palette starts with the colors already in use.
            let l = &cfg.lighting;
            let mut first: Vec<[u8; 3]> = Vec::new();
            for s in l.groups.iter().flat_map(|g| [&g.whole, &g.inner, &g.outer]).chain(l.merged.iter()) {
                let n = effects::mode(&s.mode).map(|m| m.color_count()).unwrap_or(0);
                for c in s.colors.iter().take(n) {
                    if !first.contains(c) && first.len() < 8 {
                        first.push(*c);
                    }
                }
            }
            cfg.saved_colors = first;
        }
        let mut light_draft = cfg.lighting.clone();
        if demo && std::env::var_os("INFINIFAN_DEMO_DIRTY").is_some()
            && let Some(m) = effects::mode("Meteor_Outer")
        {
            for gl in light_draft.groups.iter_mut() {
                gl.outer.switch_mode(m);
            }
        }
        let tab = match std::env::var("INFINIFAN_DEMO_TAB").ok().filter(|_| demo).as_deref() {
            Some("lighting") => Tab::Lighting,
            Some("external") => Tab::External,
            Some("settings") => Tab::Settings,
            Some("about") => Tab::About,
            _ => Tab::Fans,
        };

        Self {
            light_draft,
            cfg,
            worker,
            status: Status::default(),
            tab,
            group,
            light_all_groups: true,
            dragging: None,
            dirty_since: None,
            autostart: system::autostart_enabled(),
            message: None,
            elevated: is_elevated(),
            preview: FanPreview::default(),
            remote,
            quit_requested,
            quitting: false,
            demo,
            pawnio: PawnDialog::Hidden,
            pawnio_checked: false,
            pawnio_job: None,
        }
    }

    /// Runs the PawnIO installer in the background (Windows shows the UAC prompt).
    fn start_pawnio(&mut self, install: bool) {
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let _ = tx.send(pawnio_setup::run(install));
        });
        self.pawnio_job = Some(rx);
        self.pawnio = PawnDialog::Running(install);
    }

    fn pawnio_dialog(&mut self, ctx: &egui::Context) {
        if self.pawnio == PawnDialog::Hidden {
            return;
        }
        let frame = Frame::new()
            .fill(CARD)
            .stroke(Stroke::new(1.0, BORDER))
            .corner_radius(CornerRadius::same(theme::RADIUS))
            .inner_margin(Margin::same(22));
        egui::Modal::new(egui::Id::unique("pawnio")).frame(frame).show(ctx, |ui| {
            ui.set_width(460.0);
            ui.spacing_mut().item_spacing.y = 10.0;
            match self.pawnio.clone() {
                PawnDialog::Ask => {
                    ui.label(RichText::new(tr!("pawnio.title")).font(theme::semibold(18.0)).color(TEXT));
                    ui.label(tr!("pawnio.ask"));
                    let setup = pawnio_setup::setup_path();
                    if setup.exists() {
                        ui.label(RichText::new(tr!("pawnio.ask_setup", path = setup.display())).color(MUTED).size(13.0));
                    } else {
                        ui.label(RichText::new(tr!("pawnio.ask_missing", path = setup.display())).color(WARN).size(13.0));
                    }
                    ui.label(RichText::new(tr!("pawnio.ask_without")).color(MUTED).size(13.0));
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        if widgets::pill(ui, &with_icon(icons::DOWNLOAD, &tr!("pawnio.install")), true).clicked() {
                            self.start_pawnio(true);
                        }
                        if ui.button(tr!("common.not_now")).clicked() {
                            self.pawnio = PawnDialog::Hidden;
                        }
                        if ui.button(tr!("common.dont_ask")).clicked() {
                            self.cfg.pawnio_prompt_dismissed = true;
                            self.pawnio = PawnDialog::Hidden;
                            self.changed();
                        }
                    });
                }
                PawnDialog::Running(install) => {
                    let title = if install { tr!("pawnio.installing") } else { tr!("pawnio.uninstalling") };
                    ui.label(RichText::new(title).font(theme::semibold(18.0)).color(TEXT));
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label(RichText::new(tr!("pawnio.confirm_uac")).color(MUTED));
                    });
                }
                PawnDialog::Done(install, res) => {
                    match &res {
                        Ok(()) => {
                            let title = if install { tr!("pawnio.installed") } else { tr!("pawnio.uninstalled") };
                            ui.label(RichText::new(title).font(theme::semibold(18.0)).color(OK));
                            if install && !self.elevated {
                                ui.label(tr!("pawnio.need_admin"));
                            }
                        }
                        Err(e) => {
                            ui.label(RichText::new(tr!("common.failed")).font(theme::semibold(18.0)).color(ERR));
                            ui.label(e);
                        }
                    }
                    ui.horizontal(|ui| {
                        if install
                            && res.is_ok()
                            && !self.elevated
                            && widgets::pill(ui, &with_icon(icons::SHIELD, &tr!("common.restart_admin")), true).clicked()
                            
                        {
                            self.quit_requested.store(true, Ordering::SeqCst);
                        }
                        if ui.button(tr!("common.close")).clicked() {
                            self.pawnio = PawnDialog::Hidden;
                        }
                    });
                }
                PawnDialog::Hidden => {}
            }
        });
    }

    fn changed(&mut self) {
        self.worker.send(Command::Config(Box::new(self.cfg.clone())));
        self.dirty_since = Some(Instant::now());
    }

    fn lights_allowed(&self) -> bool {
        let e = &self.cfg.external;
        !e.mb_argb_sync && !e.rgb_by_other_software && !e.release_device
    }

    /// Group picker – only when more than one group is connected.
    fn group_picker(&mut self, ui: &mut egui::Ui) {
        let used: Vec<usize> = (0..GROUP_COUNT).filter(|g| self.cfg.quantity[*g] > 0).collect();
        if used.len() > 1 {
            let labels: Vec<(usize, String)> = used.iter().map(|g| (*g, tr!("group.n", n = g + 1))).collect();
            let opts: Vec<(usize, &str)> = labels.iter().map(|(g, l)| (*g, l.as_str())).collect();
            segmented(ui, &mut self.group, &opts);
        }
    }

    // ---------------------------------------------------------------- Fans

    fn fans_ui(&mut self, ui: &mut egui::Ui) {
        let g = self.group;
        let rpm_sync = self.cfg.external.mb_rpm_sync;
        if rpm_sync {
            banner(ui, WARN, &tr!("fans.mb_banner"));
        }

        // Value tiles.
        let (temp, fallback) = self.status.temps.for_source(self.cfg.fans[g].source);
        let w = (ui.available_width() - ui.spacing().item_spacing.x) / 2.0;
        let fmt_t = |t: Option<f32>| t.map(|t| format!("{t:.0}")).unwrap_or_else(|| "–".into());
        let cpu = self.status.temps.cpu;
        let gpu = self.status.temps.gpu;
        let max = self.cfg.fans[g].size.max_rpm() as f32;
        let rpm = self.status.rpm[g];
        let duty = self.status.duty[g];
        ui.horizontal(|ui| {
            stat_tile(ui, w, &with_icon(icons::CPU, "CPU"), &fmt_t(cpu), "°C", temp_color(cpu), cpu.map(|t| t / 100.0));
            stat_tile(ui, w, &with_icon(icons::MONITOR, "GPU"), &fmt_t(gpu), "°C", temp_color(gpu), gpu.map(|t| t / 100.0));
        });
        ui.horizontal(|ui| {
            stat_tile(ui, w, &with_icon(icons::FAN, &tr!("fans.rpm")), &rpm.to_string(), "RPM", ACCENT, Some(rpm as f32 / max));
            let d = match duty {
                Some(1) => tr!("fans.stop"),
                Some(d) => d.to_string(),
                None if rpm_sync => tr!("fans.board"),
                None => "–".to_string(),
            };
            stat_tile(ui, w, &with_icon(icons::GAUGE, &tr!("fans.duty")), &d, if duty.is_some_and(|d| d > 1) { "%" } else { "" }, ACCENT2, duty.map(|d| d as f32 / 100.0));
        });

        let mut changed = false;
        ui.add_enabled_ui(!rpm_sync, |ui| {
            card(ui, &with_icon(icons::SLIDERS_HORIZONTAL, &tr!("fans.profile")), |ui| {
                self.group_picker(ui);
                let fan = &mut self.cfg.fans[g];
                let mut preset = fan.preset;
                if segmented(
                    ui,
                    &mut preset,
                    &[
                        (Preset::Quiet, tr!("preset.quiet").as_str()),
                        (Preset::Standard, tr!("preset.standard").as_str()),
                        (Preset::HighSpeed, tr!("preset.high").as_str()),
                        (Preset::FullSpeed, tr!("preset.full").as_str()),
                        (Preset::Custom, tr!("preset.custom").as_str()),
                        (Preset::Fixed, tr!("preset.fixed").as_str()),
                    ],
                ) {
                    fan.set_preset(preset);
                    changed = true;
                }
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        field_label(ui, &tr!("fans.size"));
                        let mut size = fan.size;
                        if segmented(ui, &mut size, &[(FanSize::Mm120, "120 mm"), (FanSize::Mm140, "140 mm")]) {
                            fan.set_size(size);
                            changed = true;
                        }
                    });
                    ui.add_space(12.0);
                    ui.vertical(|ui| {
                        field_label(ui, &tr!("fans.temp_source"));
                        changed |= segmented(ui, &mut fan.source, &[(Source::Cpu, "CPU"), (Source::Gpu, "GPU")]);
                    });
                });
                if fan.preset == Preset::Fixed {
                    field_label(ui, &tr!("fans.fixed_rpm"));
                    changed |= ui
                        .add(egui::Slider::new(&mut fan.fixed_rpm, fan.size.min_rpm()..=fan.size.max_rpm()).suffix(" RPM"))
                        .changed();
                }
                ui.add_space(4.0);
                changed |= toggle_row(ui, &mut fan.start_stop, &tr!("fans.stop_low"), &tr!("fans.stop_low_desc"));
            });

            let right = temp.map(|t| {
                let mut s = format!("{t:.1} °C → {} RPM", self.cfg.fans[g].target_rpm(t));
                if fallback {
                    s.push_str(&tr!("fans.fallback"));
                }
                RichText::new(s).color(if fallback { WARN } else { MUTED }).size(13.0)
            });
            card_with(ui, &with_icon(icons::CHART_SPLINE, &tr!("fans.curve")), right, |ui| {
                changed |= curve_editor(ui, &mut self.cfg.fans[g], temp, &mut self.dragging, g);
                ui.label(RichText::new(tr!("fans.curve_hint")).size(12.0).color(MUTED));
            });
        });

        if changed {
            self.changed();
        }
    }

    // ---------------------------------------------------------------- Lighting

    fn lighting_ui(&mut self, ui: &mut egui::Ui) {
        let ext = &self.cfg.external;
        let blocked = if ext.release_device {
            Some(tr!("light.blocked_released"))
        } else if ext.rgb_by_other_software {
            Some(tr!("light.blocked_other"))
        } else if ext.mb_argb_sync {
            Some(tr!("light.blocked_board"))
        } else {
            None
        };
        if let Some(msg) = blocked {
            banner(ui, WARN, &tr!("light.blocked", msg = msg));
        }

        let draft = &mut self.light_draft;
        card(ui, &with_icon(icons::SLIDERS_HORIZONTAL, &tr!("light.scope")), |ui| {
            let mut mode = if draft.merged.is_some() {
                2
            } else if draft.groups[0].individual {
                0
            } else {
                1
            };
            if segmented(ui, &mut mode, &[(0, with_icon(icons::CIRCLE_DOT, &tr!("light.scope_split")).as_str()), (1, with_icon(icons::FAN, &tr!("light.scope_whole")).as_str()), (2, with_icon(icons::LAYERS, &tr!("light.scope_merged")).as_str())]) {
                if mode == 2 {
                    let def = effects::mode(&draft.groups[0].whole.mode)
                        .and_then(|m| m.merge)
                        .and_then(effects::mode)
                        .unwrap_or_else(|| effects::modes_for(Scope::Merge).next().unwrap());
                    draft.merged = Some(EffectSetting::new(def));
                } else {
                    draft.merged = None;
                    for gl in draft.groups.iter_mut() {
                        gl.individual = mode == 0;
                    }
                }
            }
            if draft.merged.is_none() {
                let used = (0..GROUP_COUNT).filter(|g| self.cfg.quantity[*g] > 0).count();
                if used > 1 {
                    toggle_row(ui, &mut self.light_all_groups, &tr!("light.all_groups"), "");
                }
            }
        });
        if draft.merged.is_none() && !self.light_all_groups {
            self.group_picker(ui);
        }

        let g = if self.light_all_groups { 0 } else { self.group };
        let fans = self.cfg.quantity[g].max(1) as usize;
        let draft = &mut self.light_draft;
        let saved = &mut self.cfg.saved_colors;
        let mut changed = false;
        let mut saved_changed = false;
        let mut edit = |ui: &mut egui::Ui, scope: Scope, s: &mut EffectSetting| {
            let (c, sv) = effect_editor(ui, scope, s, fans, saved);
            changed |= c;
            saved_changed |= sv;
        };
        if let Some(m) = draft.merged.as_mut() {
            card(ui, &with_icon(icons::LAYERS, &tr!("light.card_merged")), |ui| edit(ui, Scope::Merge, m));
        } else {
            let gl = &mut draft.groups[g];
            if gl.individual {
                card(ui, &with_icon(icons::CIRCLE_DOT, &tr!("light.card_inner")), |ui| edit(ui, Scope::Inner, &mut gl.inner));
                card(ui, &with_icon(icons::SQUARE, &tr!("light.card_outer")), |ui| edit(ui, Scope::Outer, &mut gl.outer));
            } else {
                card(ui, &with_icon(icons::FAN, &tr!("light.scope_whole")), |ui| edit(ui, Scope::Whole, &mut gl.whole));
            }
        }
        if saved_changed {
            self.changed();
        }
        let draft = &mut self.light_draft;
        if changed && self.light_all_groups && draft.merged.is_none() {
            let src = draft.groups[0].clone();
            for other in draft.groups.iter_mut().skip(1) {
                *other = src.clone();
            }
        }
        if ui.button(&with_icon(icons::SCAN_EYE, &tr!("light.identify"))).clicked() {
            self.worker.send(Command::Ping(g));
        }
    }

    /// Bar shown while lighting changes are unsaved.
    fn save_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label(RichText::new(with_icon(icons::CIRCLE_ALERT, &tr!("light.unsaved"))).color(WARN));
            ui.label(RichText::new(tr!("light.unsaved_hint")).color(MUTED).size(12.5));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if widgets::pill(ui, &with_icon(icons::CHECK, &tr!("common.save")), true).clicked() {
                    self.cfg.lighting = self.light_draft.clone();
                    self.changed();
                }
                if ui.button(&with_icon(icons::X, &tr!("common.cancel"))).clicked() {
                    self.light_draft = self.cfg.lighting.clone();
                }
            });
        });
    }

    // ---------------------------------------------------------------- Other software

    fn external_ui(&mut self, ui: &mut egui::Ui) {
        ui.label(RichText::new(tr!("ext.intro")).color(MUTED));
        let e = &mut self.cfg.external;
        let mut changed = false;
        card(ui, &with_icon(icons::CIRCUIT_BOARD, &tr!("ext.board")), |ui| {
            changed |= toggle_row(
                ui,
                &mut e.mb_rpm_sync,
                &tr!("ext.mb_rpm"),
                &tr!("ext.mb_rpm_desc"),
            );
            widgets::divider(ui);
            changed |= toggle_row(
                ui,
                &mut e.mb_argb_sync,
                &tr!("ext.mb_argb"),
                &tr!("ext.mb_argb_desc"),
            );
        });
        card(ui, &with_icon(icons::APP_WINDOW, &tr!("ext.other")), |ui| {
            changed |= toggle_row(
                ui,
                &mut e.rgb_by_other_software,
                &tr!("ext.other_rgb"),
                &tr!("ext.other_rgb_desc"),
            );
            if let Some(s) = &self.status.signalrgb {
                ui.label(RichText::new(s).size(12.5).color(MUTED));
            }
            widgets::divider(ui);
            changed |= toggle_row(
                ui,
                &mut e.release_device,
                &tr!("ext.release"),
                &tr!("ext.release_desc"),
            );
        });
        if changed {
            self.changed();
        }
    }

    // ---------------------------------------------------------------- Settings

    fn settings_ui(&mut self, ui: &mut egui::Ui) {
        let mut changed = false;
        card(ui, &with_icon(icons::USB, &tr!("set.controller")), |ui| {
            field_label(ui, &tr!("set.fan_count"));
            ui.horizontal_wrapped(|ui| {
                for g in 0..GROUP_COUNT {
                    ui.vertical(|ui| {
                        ui.label(RichText::new(tr!("group.n", n = g + 1)).color(TEXT));
                        changed |= segmented(ui, &mut self.cfg.quantity[g], &[(0, "0"), (1, "1"), (2, "2"), (3, "3"), (4, "4")]);
                    });
                    ui.add_space(10.0);
                }
            });
            if let Some(fw) = &self.status.firmware {
                ui.label(RichText::new(format!("Firmware {fw}")).color(MUTED).size(13.0));
            }
        });

        card(ui, &with_icon(icons::THERMOMETER, &tr!("set.temps")), |ui| {
            match self.status.cpu_sensor.clone() {
                Some(CpuSensorStatus::Ok) => {
                    ui.label(tr!("set.cpu_ok"));
                }
                Some(CpuSensorStatus::PawnIoMissing) => {
                    ui.label(RichText::new(tr!("set.cpu_missing")).color(WARN));
                    ui.label(RichText::new(tr!("set.cpu_missing_hint")).color(MUTED).size(13.0));
                    if ui.button(&with_icon(icons::DOWNLOAD, &tr!("set.install_pawnio"))).clicked() {
                        self.pawnio = PawnDialog::Ask;
                    }
                }
                Some(CpuSensorStatus::NeedsAdmin) => {
                    ui.label(RichText::new(tr!("set.cpu_admin")).color(WARN));
                    if ui.button(&with_icon(icons::SHIELD, &tr!("common.restart_admin"))).clicked() {
                        self.quit_requested.store(true, Ordering::SeqCst);
                    }
                }
                Some(CpuSensorStatus::Unsupported(e)) => {
                    ui.label(RichText::new(tr!("set.cpu_unsupported", e = e)).color(WARN));
                }
                None => {}
            }
            if let Some(gpu) = &self.status.gpu_name {
                ui.label(RichText::new(tr!("set.gpu", gpu = gpu)).color(TEXT));
            }
            if pawnio_setup::is_installed() && !self.demo {
                widgets::divider(ui);
                ui.horizontal(|ui| {
                    ui.label(RichText::new(tr!("set.pawnio_installed")).color(MUTED).size(13.0));
                    if ui.button(tr!("set.uninstall")).clicked() {
                        self.start_pawnio(false);
                    }
                });
            }
        });

        card(ui, &with_icon(icons::LANGUAGES, &tr!("set.language")), |ui| {
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.set_width((ui.available_width() - 200.0).max(100.0));
                    ui.label(RichText::new(tr!("set.language_app")).font(theme::semibold(14.5)).color(TEXT));
                    ui.label(RichText::new(tr!("set.language_desc")).color(MUTED).size(13.0));
                });
                // Each language name is written in that language.
                let names: Vec<(String, String)> = lang::languages()
                    .into_iter()
                    .map(|l| {
                        let name = lang::name_of(&l);
                        (l, name)
                    })
                    .collect();
                let opts: Vec<&str> = names.iter().map(|(_, n)| n.as_str()).collect();
                let now = lang::current();
                let mut current = names.iter().position(|(l, _)| *l == now).unwrap_or(0);
                if widgets::dropdown(ui, "language", &mut current, &opts, 170.0) {
                    let code = names[current].0.clone();
                    lang::set(&code);
                    self.cfg.language = Some(code);
                    changed = true;
                }
            });
        });

        card(ui, &with_icon(icons::POWER, &tr!("set.startup")), |ui| {
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.set_width((ui.available_width() - 190.0).max(100.0));
                    ui.label(RichText::new(tr!("set.bg_title")).font(theme::semibold(14.5)).color(TEXT));
                    ui.label(
                        RichText::new(tr!("set.bg_desc"))
                            .color(MUTED)
                            .size(13.0),
                    );
                });
                if !self.demo && ui.button(&with_icon(icons::CIRCLE_STOP, &tr!("set.quit_control"))).clicked() {
                    self.worker.send(Command::Quit);
                }
            });
            widgets::divider(ui);
            let mut autostart = self.autostart;
            let desc = if self.elevated {
                tr!("set.autostart_admin")
            } else {
                tr!("set.autostart_user")
            };
            let resp = ui.add_enabled_ui(self.elevated, |ui| toggle_row(ui, &mut autostart, &tr!("set.autostart"), &desc)).inner;
            if resp {
                match system::set_autostart(autostart) {
                    Ok(()) => {
                        self.autostart = autostart;
                        self.message = None;
                    }
                    Err(e) => self.message = Some(format!("Autostart: {e}")),
                }
            }
            if let Some(m) = &self.message {
                ui.label(RichText::new(m).color(ERR));
            }
        });
        ui.label(RichText::new(tr!("set.config_path", path = AppConfig::path().display())).size(12.0).color(MUTED));

        if changed {
            self.changed();
        }
    }

    // ---------------------------------------------------------------- About (English only)

    fn about_ui(&mut self, ui: &mut egui::Ui) {
        card(ui, "", |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                ui.label(RichText::new("INFINI").font(theme::semibold(22.0)).color(ACCENT));
                ui.label(RichText::new("FAN").font(theme::semibold(22.0)).color(ACCENT2));
                ui.add_space(10.0);
                ui.label(RichText::new(format!("version {}", env!("CARGO_PKG_VERSION"))).color(MUTED).size(13.0));
            });
            ui.label(RichText::new("by Krejza9").color(TEXT).size(13.5));
            ui.add_space(2.0);
            ui.label(
                RichText::new(
                    "A small, fast replacement for Lian Li L-Connect 3 for the SL Infinity fan controller, written in Rust. \
                     Control runs in a tiny background process (~10 MB, ~0 % CPU); this window exists only while it is open.",
                )
                .color(TEXT)
                .size(13.5),
            );
        });
        card(ui, &with_icon(icons::INFO, "What it does"), |ui| {
            for (title, text) in [
                ("Fans", "Speed curves by CPU or GPU temperature, the L-Connect presets, a custom curve, fixed speed and stop at low temperature."),
                ("Lighting", "All L-Connect effects plus 11 hidden firmware effects, an accurate live preview, Save / Cancel and a My colors palette."),
                ("Other software", "Motherboard fan and ARGB sync, or hand the lighting over to SignalRGB / OpenRGB."),
                ("System", "CPU temperature via PawnIO, GPU temperature from Windows, autostart, tray icon, recovery after sleep, settings import from L-Connect 3 and switchable languages."),
            ] {
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing.x = 6.0;
                    ui.label(RichText::new(title).font(theme::semibold(13.5)).color(TEXT));
                    ui.label(RichText::new(text).color(MUTED).size(13.0));
                });
            }
        });
        card(ui, &with_icon(icons::HEART, "Credits"), |ui| {
            ui.label(RichText::new("PawnIO by namazso · egui · hidapi · tray-icon · windows-rs · rust-i18n").color(MUTED).size(12.5));
            ui.label(
                RichText::new("Not affiliated with Lian Li. L-Connect, SL Infinity and Lian Li are trademarks of Lian Li Industrial Co., Ltd.")
                    .color(MUTED)
                    .size(12.5),
            );
        });
    }

    // ---------------------------------------------------------------- Window frame

    fn sidebar(&mut self, ui: &mut egui::Ui) {
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.add_space(10.0);
            ui.spacing_mut().item_spacing.x = 6.0;
            ui.spacing_mut().item_spacing.x = 0.0;
            ui.label(RichText::new("INFINI").font(theme::semibold(18.0)).color(ACCENT));
            ui.label(RichText::new("FAN").font(theme::semibold(18.0)).color(ACCENT2));
        });
        ui.add_space(18.0);
        ui.spacing_mut().item_spacing.y = 4.0;
        for (tab, icon, label) in [
            (Tab::Fans, icons::FAN, tr!("nav.fans")),
            (Tab::Lighting, icons::LIGHTBULB, tr!("nav.lighting")),
            (Tab::External, icons::PLUG, tr!("nav.external")),
            (Tab::Settings, icons::SETTINGS, tr!("nav.settings")),
            (Tab::About, icons::INFO, "About".to_string()),
        ] {
            if widgets::nav_item(ui, icon, &label, self.tab == tab).clicked() {
                self.tab = tab;
            }
        }

        // Controller status pinned to the bottom of the panel.
        ui.add_space((ui.available_height() - 62.0).max(8.0));
        {
            let (col, title) = if self.status.connected { (OK, tr!("side.connected")) } else { (ERR, tr!("side.disconnected")) };
            Frame::new()
                .fill(col.linear_multiply(0.10))
                .stroke(Stroke::new(1.0, col.linear_multiply(0.45)))
                .corner_radius(CornerRadius::same(10))
                .inner_margin(Margin::symmetric(12, 9))
                .show(ui, |ui| ui.vertical(|ui| {
                    ui.set_width(ui.available_width());
                    ui.spacing_mut().item_spacing.y = 2.0;
                    ui.label(RichText::new(with_icon(if self.status.connected { icons::USB } else { icons::UNPLUG }, &title)).color(col).size(13.0));
                    let sub = if self.status.connected {
                        self.status.firmware.as_ref().map(|f| format!("firmware {f}"))
                    } else {
                        self.status.error.clone()
                    };
                    if let Some(s) = sub {
                        ui.label(RichText::new(s).color(MUTED).size(11.5));
                    }
                }));
        }
    }

    fn fan_column(&mut self, ui: &mut egui::Ui) {
        let g = self.group;
        let fans = self.cfg.quantity[g].max(1) as usize;
        Frame::new()
            .fill(CARD)
            .stroke(Stroke::new(1.0, BORDER))
            .corner_radius(CornerRadius::same(theme::RADIUS))
            .inner_margin(Margin::same(14))
            .show(ui, |ui| {
                ui.vertical_centered(|ui| {
                    ui.label(RichText::new(tr!("group.n", n = g + 1)).font(theme::semibold(14.5)).color(TEXT));
                    ui.add_space(4.0);
                    let lights = self.lights_allowed();
                    let lighting = if self.tab == Tab::Lighting { &self.light_draft } else { &self.cfg.lighting };
                    self.preview.show(ui, FAN_COLUMN - 28.0, lighting, g, self.cfg.quantity, self.status.rpm[g], lights);
                    ui.add_space(6.0);
                    let size = self.cfg.fans[g].size.label();
                    ui.label(RichText::new(format!("{fans}× {size} · {} RPM", self.status.rpm[g])).size(12.5).color(MUTED));
                });
            });
    }
}

fn temp_color(t: Option<f32>) -> Color32 {
    match t {
        Some(t) if t >= 80.0 => ERR,
        Some(t) if t >= 60.0 => WARN,
        _ => OK,
    }
}

impl eframe::App for App {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.status = self.worker.status();

        if let Some(r) = &self.remote {
            // Config from the background process (after connecting) – the window adopts it.
            if let Some(c) = r.config.lock().unwrap().take() {
                if self.light_draft == self.cfg.lighting {
                    self.light_draft = c.lighting.clone();
                }
                self.cfg = c;
            }
            if r.show.swap(false, Ordering::SeqCst) {
                ctx.send_viewport_cmd(ViewportCommand::Visible(true));
                ctx.send_viewport_cmd(ViewportCommand::Minimized(false));
                ctx.send_viewport_cmd(ViewportCommand::Focus);
            }
            // The background process exited or is closing the window – the window exits with it.
            if r.close.load(Ordering::SeqCst) && !self.quitting {
                self.quitting = true;
                ctx.send_viewport_cmd(ViewportCommand::Close);
            }
        }
        if self.quit_requested.swap(false, Ordering::SeqCst) {
            // The background process restarts as administrator (and reopens the window).
            self.worker.send(Command::RestartElevated);
        }
        // The background process saves the config; the window saves only in demo mode.
        if let Some(t) = self.dirty_since
            && t.elapsed() >= SAVE_DELAY
        {
            if self.demo {
                self.cfg.save();
            }
            self.dirty_since = None;
        }
        // Offer to install PawnIO at startup (when missing and not dismissed).
        if !self.pawnio_checked && self.status.cpu_sensor.is_some() {
            self.pawnio_checked = true;
            let demo_prompt = self.demo && std::env::var_os("INFINIFAN_DEMO_PAWNIO").is_some();
            if demo_prompt
                || (self.status.cpu_sensor == Some(CpuSensorStatus::PawnIoMissing)
                    && !self.cfg.pawnio_prompt_dismissed
                    && !self.demo
                    && !pawnio_setup::is_installed())
            {
                self.pawnio = PawnDialog::Ask;
            }
        }
        if let Some(rx) = &self.pawnio_job
            && let Ok(res) = rx.try_recv()
        {
            let install = matches!(self.pawnio, PawnDialog::Running(true));
            if res.is_ok() {
                self.worker.send(Command::ReloadSensors);
            }
            self.pawnio = PawnDialog::Done(install, res);
            self.pawnio_job = None;
        }
        ctx.request_repaint_after(Duration::from_millis(500));
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.pawnio_dialog(&ctx);
        egui::Panel::left("nav")
            .exact_size(210.0)
            .resizable(false)
            .show_separator_line(false)
            .frame(Frame::new().fill(SIDEBAR).inner_margin(Margin::symmetric(12, 16)))
            .show(ui, |ui| self.sidebar(ui));

        if matches!(self.tab, Tab::Fans | Tab::Lighting) {
            egui::Panel::right("fans")
                .exact_size(FAN_COLUMN + 18.0)
                .resizable(false)
                .show_separator_line(false)
                .frame(Frame::new().fill(BG).inner_margin(Margin { left: 0, right: 18, top: 18, bottom: 18 }))
                .show(ui, |ui| {
                    egui::ScrollArea::vertical().id_salt("fan-col").show(ui, |ui| self.fan_column(ui));
                });
        }

        if self.tab == Tab::Lighting && self.light_draft != self.cfg.lighting {
            egui::Panel::bottom("savebar")
                .resizable(false)
                .show_separator_line(false)
                .frame(
                    Frame::new()
                        .fill(CARD)
                        .stroke(Stroke::new(1.0, WARN.linear_multiply(0.5)))
                        .inner_margin(Margin::symmetric(18, 10)),
                )
                .show(ui, |ui| self.save_bar(ui));
        }
        egui::CentralPanel::default()
            .frame(Frame::new().fill(BG).inner_margin(Margin::same(18)))
            .show(ui, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    ui.spacing_mut().item_spacing.y = 12.0;
                    match self.tab {
                        Tab::Fans => self.fans_ui(ui),
                        Tab::Lighting => self.lighting_ui(ui),
                        Tab::External => self.external_ui(ui),
                        Tab::Settings => self.settings_ui(ui),
                        Tab::About => self.about_ui(ui),
                    }
                });
            });
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        if self.dirty_since.is_some() && self.demo {
            self.cfg.save();
        }
    }
}

/// Curve editor: drag points with the mouse.
fn curve_editor(
    ui: &mut egui::Ui,
    fan: &mut FanGroupConfig,
    temp: Option<f32>,
    dragging: &mut Option<usize>,
    group: usize,
) -> bool {
    let max = fan.size.max_rpm() as f64;
    let editable = fan.preset != Preset::Fixed;
    let mut phases: Vec<[f64; 2]> = if fan.preset == Preset::Fixed {
        vec![[0.0, fan.fixed_rpm as f64], [100.0, fan.fixed_rpm as f64]]
    } else {
        fan.phases().iter().map(|&(t, r)| [t as f64, r as f64]).collect()
    };
    // With stop at low temperature the curve starts with a vertical jump from 0 RPM (spin-up).
    // It is drawn grey without fill, together with the stopped range from 0 °C.
    let spin_up: Option<Vec<[f64; 2]>> = (phases.len() > 1 && phases[0][1] == 0.0).then(|| {
        let first = phases.remove(0);
        vec![[0.0, 0.0], first, phases[0]]
    });
    let user: Vec<[f64; 2]> = fan.points.iter().map(|&(t, r)| [t as f64, r as f64]).collect();
    let current = temp.map(|t| [t as f64, fan.target_rpm(t) as f64]);

    let mut new_point: Option<(usize, PlotPoint)> = None;
    let mut drag_state = *dragging;
    Plot::new(("curve", group))
        .height(300.0)
        .show_background(false)
        .allow_drag(false)
        .allow_zoom(false)
        .allow_scroll(false)
        .allow_boxed_zoom(false)
        .allow_double_click_reset(false)
        .default_x_bounds(0.0, 100.0)
        .default_y_bounds(0.0, max * 1.05)
        .x_axis_formatter(|m, _| format!("{:.0} °C", m.value))
        .y_axis_formatter(|m, _| format!("{:.0}", m.value))
        .label_formatter(move |h| {
            let p = match h {
                HoverPosition::NearDataPoint { position, .. } => position,
                HoverPosition::Elsewhere { position } => position,
            };
            Some(format!("{:.0} °C · {:.0} RPM ({:.0} %)", p.x, p.y, p.y / max * 100.0))
        })
        .show(ui, |pui| {
            if let Some(s) = spin_up {
                pui.line(Line::new(tr!("fans.curve"), PlotPoints::from(s)).color(MUTED).width(2.0).style(LineStyle::dashed_dense()));
            }
            pui.line(
                Line::new(tr!("fans.curve"), PlotPoints::from(phases))
                    .color(ACCENT)
                    .width(2.5)
                    .fill(0.0)
                    .fill_alpha(0.10),
            );
            if editable {
                pui.points(Points::new(tr!("plot.points"), PlotPoints::from(user.clone())).radius(6.5).color(TEXT).filled(true));
            }
            if let Some(c) = current {
                pui.vline(VLine::new(tr!("plot.current_temp"), c[0]).color(WARN.linear_multiply(0.7)));
                pui.points(Points::new(tr!("plot.current"), PlotPoints::from(vec![c])).radius(5.0).color(WARN));
            }
            if !editable {
                return;
            }
            let resp = pui.response().clone();
            if resp.drag_started()
                && let Some(pos) = resp.interact_pointer_pos()
            {
                drag_state = user
                    .iter()
                    .enumerate()
                    .map(|(i, p)| (i, pui.transform().position_from_point(&PlotPoint::new(p[0], p[1])).distance(pos)))
                    .filter(|(_, d)| *d < 16.0)
                    .min_by(|a, b| a.1.total_cmp(&b.1))
                    .map(|(i, _)| i);
            }
            if resp.dragged()
                && let (Some(i), Some(p)) = (drag_state, pui.pointer_coordinate())
            {
                new_point = Some((i, p));
            }
            if resp.drag_stopped() {
                drag_state = None;
            }
        });
    *dragging = drag_state;

    if let Some((i, p)) = new_point {
        let lo = if i == 0 { 0 } else { fan.points[i - 1].0 + 1 };
        let hi = if i + 1 == fan.points.len() { 100 } else { fan.points[i + 1].0 - 1 };
        let t = (p.x.round() as i32).clamp(lo, hi.max(lo));
        // Speed must not fall as the temperature rises: stay between the neighbours' speeds.
        let r_lo = if i == 0 { 0 } else { fan.points[i - 1].1 };
        let r_hi = if i + 1 == fan.points.len() { fan.size.max_rpm() } else { fan.points[i + 1].1 };
        let r = (p.y.round() as i32).clamp(r_lo, r_hi.max(r_lo));
        if fan.points[i] != (t, r) {
            fan.points[i] = (t, r);
            fan.preset = Preset::Custom;
            return true;
        }
    }
    false
}

/// Effect editor: effect tiles, colors, speed, brightness, direction.
fn effect_editor(ui: &mut egui::Ui, scope: Scope, s: &mut EffectSetting, fans: usize, saved: &mut Vec<[u8; 3]>) -> (bool, bool) {
    let mut changed = false;
    let mut saved_changed = false;
    let current: &'static ModeDef = match effects::mode(&s.mode).filter(|m| m.scope == scope) {
        Some(m) => m,
        None => {
            let d = effects::modes_for(scope).next().unwrap();
            s.switch_mode(d);
            changed = true;
            d
        }
    };

    // Effect tiles in three columns.
    let gap = 5.0;
    let w = ((ui.available_width() - 2.0 * gap) / 3.0).floor();
    for hidden in [false, true] {
        let modes: Vec<&'static ModeDef> = effects::modes_for(scope).filter(|m| m.hidden == hidden).collect();
        if modes.is_empty() {
            continue;
        }
        if hidden {
            ui.add_space(6.0);
            field_label(ui, &with_icon(icons::SPARKLES, &tr!("fxui.hidden")));
        }
        for row in modes.chunks(3) {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = gap;
                for m in row {
                    if widgets::tile(ui, &m.name(), m.id == current.id, w).clicked() && m.id != current.id {
                        s.switch_mode(m);
                        changed = true;
                    }
                }
            });
            ui.add_space(-ui.spacing().item_spacing.y + gap);
        }
    }
    let def = effects::mode(&s.mode).unwrap_or(current);

    ui.add_space(6.0);
    let per_fan = def.id.starts_with("StaticColor") || (def.id.starts_with("Breathing") && !def.id.starts_with("BreathingRainbow"));
    let count = if per_fan { fans.min(def.color_count()) } else { def.color_count() };
    if count == 0 {
        ui.label(RichText::new(tr!("fxui.no_colors")).color(MUTED).size(12.5));
    } else {
        if per_fan {
            field_label(ui, &with_icon(icons::PALETTE, &tr!("fxui.per_fan")));
        } else {
            field_label(ui, &with_icon(icons::PALETTE, &tr!("fxui.colors", count = count)));
        }
        ui.horizontal(|ui| {
            for (i, c) in s.colors.iter_mut().take(count).enumerate() {
                ui.vertical(|ui| {
                    let r = widgets::color_picker(ui, c, saved);
                    changed |= r.color;
                    saved_changed |= r.saved;
                    let label = if per_fan { tr!("fxui.fan_n", n = i + 1) } else { tr!("fxui.color_n", n = i + 1) };
                    ui.label(RichText::new(label).size(11.0).color(MUTED));
                });
            }
        });
    }
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = 16.0;
        if def.speed {
            ui.vertical(|ui| {
                field_label(ui, &with_icon(icons::GAUGE, &tr!("fxui.speed")));
                changed |= segmented(ui, &mut s.speed, &[(0, "1"), (25, "2"), (50, "3"), (75, "4"), (100, "5")]);
            });
        }
        if def.brightness {
            ui.vertical(|ui| {
                field_label(ui, &with_icon(icons::SUN, &tr!("fxui.brightness")));
                changed |= segmented(ui, &mut s.brightness, &[(0, tr!("fxui.off").as_str()), (25, "25"), (50, "50"), (75, "75"), (100, "100")]);
            });
        }
        if def.direction {
            ui.vertical(|ui| {
                field_label(ui, &tr!("fxui.direction"));
                changed |= segmented(ui, &mut s.reverse, &[(false, with_icon(icons::ARROW_RIGHT, &tr!("fxui.right")).as_str()), (true, with_icon(icons::ARROW_LEFT, &tr!("fxui.left")).as_str())]);
            });
        }
    });
    (changed, saved_changed)
}

