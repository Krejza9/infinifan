//! Shared UI pieces: cards, toggles, segmented controls, value tiles.

use eframe::egui::{
    self, Align2, Color32, CornerRadius, CursorIcon, Frame, Margin, Response, RichText, Sense, Stroke, StrokeKind,
    Ui, WidgetText, pos2, vec2,
};

use crate::theme::{self, ACCENT, ACCENT2, BORDER, CARD, CARD_HI, MUTED, RADIUS, TEXT};

/// Card with a title and an optional label on the right.
pub fn card<R>(ui: &mut Ui, title: &str, add: impl FnOnce(&mut Ui) -> R) -> R {
    card_with(ui, title, None, add)
}

pub fn card_with<R>(ui: &mut Ui, title: &str, right: Option<RichText>, add: impl FnOnce(&mut Ui) -> R) -> R {
    Frame::new()
        .fill(CARD)
        .stroke(Stroke::new(1.0, BORDER))
        .corner_radius(CornerRadius::same(RADIUS))
        .inner_margin(Margin::same(18))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            if !title.is_empty() {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(title).font(theme::semibold(15.5)).color(TEXT));
                    if let Some(r) = right {
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| ui.label(r));
                    }
                });
                ui.add_space(6.0);
            }
            add(ui)
        })
        .inner
}

/// Colored strip with text (notice).
pub fn banner(ui: &mut Ui, color: Color32, text: &str) {
    Frame::new()
        .fill(color.linear_multiply(0.12))
        .stroke(Stroke::new(1.0, color.linear_multiply(0.5)))
        .corner_radius(CornerRadius::same(10))
        .inner_margin(Margin::symmetric(14, 10))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(RichText::new(with_icon(crate::icons::TRIANGLE_ALERT, text)).color(color));
        });
}

/// iOS / L-Connect style toggle.
pub fn toggle(ui: &mut Ui, on: &mut bool) -> Response {
    let size = vec2(44.0, 24.0);
    let (rect, mut resp) = ui.allocate_exact_size(size, Sense::click());
    if resp.clicked() {
        *on = !*on;
        resp.mark_changed();
    }
    let t = ui.ctx().animate_bool_responsive(resp.id, *on);
    let p = ui.painter();
    let bg = lerp_color(CARD_HI, ACCENT, t);
    p.rect_filled(rect, CornerRadius::same(12), bg);
    p.rect_stroke(rect, CornerRadius::same(12), Stroke::new(1.0, lerp_color(BORDER, ACCENT, t)), StrokeKind::Inside);
    let x = egui::lerp(rect.left() + 12.0..=rect.right() - 12.0, t);
    p.circle_filled(pos2(x, rect.center().y), 8.5, Color32::WHITE);
    resp.on_hover_cursor(CursorIcon::PointingHand)
}

/// Row with a toggle, title and description.
pub fn toggle_row(ui: &mut Ui, on: &mut bool, title: &str, desc: &str) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.set_width((ui.available_width() - 70.0).max(100.0));
            ui.label(RichText::new(title).font(theme::semibold(14.5)).color(TEXT));
            if !desc.is_empty() {
                ui.label(RichText::new(desc).color(MUTED).size(13.0));
            }
        });
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            changed = toggle(ui, on).changed();
        });
    });
    changed
}

/// Segmented control (pills). Returns true on change.
pub fn segmented<T: PartialEq + Copy>(ui: &mut Ui, value: &mut T, options: &[(T, &str)]) -> bool {
    let mut changed = false;
    Frame::new()
        .fill(Color32::from_rgb(14, 17, 23))
        .stroke(Stroke::new(1.0, BORDER))
        .corner_radius(CornerRadius::same(10))
        .inner_margin(Margin::same(3))
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            ui.horizontal(|ui| {
                for (v, label) in options {
                    let selected = *value == *v;
                    if pill(ui, label, selected).clicked() && !selected {
                        *value = *v;
                        changed = true;
                    }
                }
            });
        });
    changed
}

/// Dropdown in the app style: dark field with an accent border, options drawn like pills.
pub fn dropdown(ui: &mut Ui, id: &str, value: &mut usize, options: &[&str], width: f32) -> bool {
    let mut changed = false;
    ui.scope(|ui| {
        let v = ui.visuals_mut();
        for w in [&mut v.widgets.inactive, &mut v.widgets.hovered, &mut v.widgets.active, &mut v.widgets.open] {
            w.weak_bg_fill = Color32::from_rgb(14, 17, 23);
            w.bg_stroke = Stroke::new(1.0, ACCENT);
            w.corner_radius = CornerRadius::same(8);
            w.fg_stroke.color = TEXT;
            w.expansion = 0.0;
        }
        egui::ComboBox::from_id_salt(id)
            .width(width)
            .selected_text(RichText::new(options.get(*value).copied().unwrap_or("")).size(13.5).color(TEXT))
            .show_ui(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 2.0;
                for (i, label) in options.iter().enumerate() {
                    if pill_sized(ui, label, i == *value, Some(width - 8.0)).clicked() && i != *value {
                        *value = i;
                        changed = true;
                    }
                }
            });
    });
    changed
}

/// A single pill (selectable button).
pub fn pill(ui: &mut Ui, label: &str, selected: bool) -> Response {
    pill_sized(ui, label, selected, None)
}

/// Pill with a fixed width (`None` = fit the text); fixed-width pills are left-aligned like menu rows.
fn pill_sized(ui: &mut Ui, label: &str, selected: bool, width: Option<f32>) -> Response {
    let galley = WidgetText::from(RichText::new(label).size(13.5)).into_galley(
        ui,
        Some(egui::TextWrapMode::Extend),
        f32::INFINITY,
        egui::TextStyle::Button,
    );
    let size = vec2(width.unwrap_or(galley.size().x + 24.0), 28.0);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    let t = ui.ctx().animate_bool_responsive(resp.id, selected);
    let hover = resp.hovered() && !selected;
    let p = ui.painter();
    if t > 0.0 {
        gradient_rect(p, rect, 8.0, ACCENT.linear_multiply(t), ACCENT2.linear_multiply(t));
    } else if hover {
        p.rect_filled(rect, CornerRadius::same(8), CARD_HI);
    }
    let color = if selected { Color32::WHITE } else if hover { TEXT } else { MUTED };
    let pos = match width {
        Some(_) => egui::pos2(rect.left() + 10.0, rect.center().y - galley.size().y / 2.0),
        None => rect.center() - galley.size() / 2.0,
    };
    p.galley(pos, galley, color);
    resp.on_hover_cursor(CursorIcon::PointingHand)
}

/// Tile with a large value (temperature, RPM…).
pub fn stat_tile(ui: &mut Ui, width: f32, label: &str, value: &str, unit: &str, color: Color32, fraction: Option<f32>) {
    Frame::new()
        .fill(CARD)
        .stroke(Stroke::new(1.0, BORDER))
        .corner_radius(CornerRadius::same(RADIUS))
        .inner_margin(Margin::symmetric(16, 12))
        .show(ui, |ui| ui.vertical(|ui| {
            ui.set_width(width - 34.0);
            ui.spacing_mut().item_spacing.y = 4.0;
            ui.label(RichText::new(label.to_uppercase()).size(11.5).color(MUTED));
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                ui.label(RichText::new(value).font(theme::semibold(26.0)).color(TEXT));
                ui.label(RichText::new(unit).size(14.0).color(MUTED));
            });
            let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 4.0), Sense::hover());
            ui.painter().rect_filled(rect, CornerRadius::same(2), CARD_HI);
            if let Some(f) = fraction {
                let mut r = rect;
                r.set_width(rect.width() * f.clamp(0.0, 1.0));
                ui.painter().rect_filled(r, CornerRadius::same(2), color);
            }
        }));
}

/// Label with a Lucide icon in front.
pub fn with_icon(icon: &str, text: &str) -> String {
    format!("{icon}  {text}")
}

/// Sidebar navigation item.
pub fn nav_item(ui: &mut Ui, icon: &str, label: &str, selected: bool) -> Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 42.0), Sense::click());
    let t = ui.ctx().animate_bool_responsive(resp.id, selected);
    let p = ui.painter();
    if t > 0.0 {
        p.rect_filled(rect, CornerRadius::same(10), CARD_HI.linear_multiply(t));
        let bar = egui::Rect::from_min_size(rect.min + vec2(0.0, 10.0), vec2(3.0, rect.height() - 20.0));
        p.rect_filled(bar, CornerRadius::same(2), ACCENT.linear_multiply(t));
    } else if resp.hovered() {
        p.rect_filled(rect, CornerRadius::same(10), CARD);
    }
    let color = if selected { TEXT } else if resp.hovered() { TEXT } else { MUTED };
    let icon_color = if selected { ACCENT } else { color };
    p.text(rect.left_center() + vec2(26.0, 0.0), Align2::CENTER_CENTER, icon, egui::FontId::proportional(19.0), icon_color);
    p.text(rect.left_center() + vec2(46.0, 0.0), Align2::LEFT_CENTER, label, theme::semibold(14.5), color);
    resp.on_hover_cursor(CursorIcon::PointingHand)
}

/// Rectangle with a horizontal color gradient.
pub fn gradient_rect(p: &egui::Painter, rect: egui::Rect, radius: f32, left: Color32, right: Color32) {
    use egui::epaint::{Mesh, Vertex, WHITE_UV};
    // Rounded ends: the gradient fills the middle, the end caps are drawn in solid colors.
    let r = radius.min(rect.height() / 2.0);
    let inner = rect.shrink2(vec2(r, 0.0));
    let mut mesh = Mesh::default();
    let v = |pos, color| Vertex { pos, uv: WHITE_UV, color };
    mesh.vertices.extend([
        v(inner.left_top(), left),
        v(inner.right_top(), right),
        v(inner.right_bottom(), right),
        v(inner.left_bottom(), left),
    ]);
    mesh.indices.extend([0, 1, 2, 0, 2, 3]);
    let lr = egui::Rect::from_min_max(rect.min, pos2(inner.left() + 0.5, rect.max.y));
    let rr = egui::Rect::from_min_max(pos2(inner.right() - 0.5, rect.min.y), rect.max);
    let r8 = r as u8;
    p.rect_filled(lr, CornerRadius { nw: r8, sw: r8, ne: 0, se: 0 }, left);
    p.rect_filled(rr, CornerRadius { ne: r8, se: r8, nw: 0, sw: 0 }, right);
    p.add(mesh);
}

pub fn lerp_color(a: Color32, b: Color32, t: f32) -> Color32 {
    let l = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color32::from_rgb(l(a.r(), b.r()), l(a.g(), b.g()), l(a.b(), b.b()))
}

/// Section label inside a card.
pub fn field_label(ui: &mut Ui, text: &str) {
    ui.label(RichText::new(text).size(12.5).color(MUTED));
}

/// Thin divider in the border color.
pub fn divider(ui: &mut Ui) {
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 9.0), Sense::hover());
    ui.painter().hline(rect.x_range(), rect.center().y, Stroke::new(1.0, BORDER));
}

/// Tile in the effect grid.
pub fn tile(ui: &mut Ui, label: &str, selected: bool, width: f32) -> Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(width, 28.0), Sense::click());
    let p = ui.painter();
    let hover = resp.hovered() && !selected;
    let (fill, stroke, text) = if selected {
        (ACCENT, ACCENT, Color32::from_rgb(6, 34, 44))
    } else if hover {
        (Color32::from_rgb(14, 17, 23), ACCENT.linear_multiply(0.5), TEXT)
    } else {
        (Color32::from_rgb(14, 17, 23), BORDER, Color32::from_rgb(170, 177, 194))
    };
    p.rect_filled(rect, CornerRadius::same(7), fill);
    p.rect_stroke(rect, CornerRadius::same(7), Stroke::new(1.0, stroke), StrokeKind::Inside);
    let font = if selected { theme::semibold(12.5) } else { egui::FontId::proportional(12.5) };
    p.text(rect.center(), Align2::CENTER_CENTER, label, font, text);
    resp.on_hover_cursor(CursorIcon::PointingHand)
}

/// Maximum number of colors in the "My colors" palette.
pub const SAVED_COLORS_MAX: usize = 24;

fn to_hex(c: [u8; 3]) -> String {
    format!("#{:02X}{:02X}{:02X}", c[0], c[1], c[2])
}

fn parse_hex(s: &str) -> Option<[u8; 3]> {
    let h = s.trim().trim_start_matches('#');
    if h.len() != 6 {
        return None;
    }
    let v = u32::from_str_radix(h, 16).ok()?;
    Some([(v >> 16) as u8, (v >> 8) as u8, v as u8])
}

/// Result of the color picker.
#[derive(Default)]
pub struct Picked {
    /// The selected color changed.
    pub color: bool,
    /// The "My colors" palette changed.
    pub saved: bool,
}

/// Color swatch; a click opens the picker with a HEX field and the saved-colors palette.
pub fn color_picker(ui: &mut Ui, c: &mut [u8; 3], saved: &mut Vec<[u8; 3]>) -> Picked {
    let mut out = Picked::default();
    let (rect, resp) = ui.allocate_exact_size(vec2(44.0, 26.0), Sense::click());
    let open = egui::Popup::is_id_open(ui.ctx(), egui::Popup::default_response_id(&resp));
    ui.painter().rect_filled(rect, CornerRadius::same(7), Color32::from_rgb(c[0], c[1], c[2]));
    let stroke = if open || resp.hovered() { Stroke::new(2.0, ACCENT) } else { Stroke::new(1.0, Color32::from_rgb(58, 65, 82)) };
    ui.painter().rect_stroke(rect, CornerRadius::same(7), stroke, StrokeKind::Outside);
    let resp = resp.on_hover_cursor(CursorIcon::PointingHand);
    // For screenshots: INFINIFAN_DEMO_PICKER opens the picker on the first swatch (--demo only).
    static DEMO_OPENED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    if std::env::var_os("INFINIFAN_DEMO_PICKER").is_some()
        && std::env::args().any(|a| a == "--demo")
        && !DEMO_OPENED.swap(true, std::sync::atomic::Ordering::SeqCst)
    {
        egui::Popup::open_id(ui.ctx(), egui::Popup::default_response_id(&resp));
    }

    let frame = Frame::new()
        .fill(Color32::from_rgb(27, 32, 48))
        .stroke(Stroke::new(1.0, Color32::from_rgb(46, 53, 70)))
        .corner_radius(CornerRadius::same(12))
        .inner_margin(Margin::same(14));
    egui::Popup::from_toggle_button_response(&resp)
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
        .frame(frame)
        .show(|ui| {
            ui.set_width(250.0);
            out.color |= hsv_picker(ui, resp.id.with("hsv"), c);

            // HEX field: keeps its own text while typing, otherwise shows the current color.
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                let (r, _) = ui.allocate_exact_size(vec2(36.0, 24.0), Sense::hover());
                ui.painter().rect_filled(r, CornerRadius::same(6), Color32::from_rgb(c[0], c[1], c[2]));
                let id = resp.id.with("hex");
                let mut text = ui.data_mut(|d| d.get_temp::<String>(id)).unwrap_or_else(|| to_hex(*c));
                let te = ui.add(egui::TextEdit::singleline(&mut text).desired_width(90.0).font(egui::TextStyle::Monospace));
                if te.changed()
                    && let Some(v) = parse_hex(&text)
                {
                    *c = v;
                    out.color = true;
                }
                if te.has_focus() {
                    ui.data_mut(|d| d.insert_temp(id, text));
                } else {
                    ui.data_mut(|d| d.remove::<String>(id));
                }
            });

            ui.add_space(8.0);
            ui.label(RichText::new("Moje barvy").size(11.5).color(MUTED));
            let cell = vec2(25.0, 22.0);
            let mut remove = None;
            let mut items: Vec<Option<[u8; 3]>> = saved.iter().copied().map(Some).collect();
            if saved.len() < SAVED_COLORS_MAX {
                items.push(None); // "+ Save" button
            }
            for row in items.chunks(8) {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 5.0;
                    for it in row {
                        match it {
                            Some(sc) => {
                                let (r, rs) = ui.allocate_exact_size(cell, Sense::click());
                                ui.painter().rect_filled(r, CornerRadius::same(6), Color32::from_rgb(sc[0], sc[1], sc[2]));
                                let st = if rs.hovered() { Stroke::new(2.0, ACCENT) } else { Stroke::new(1.0, Color32::from_rgb(58, 65, 82)) };
                                ui.painter().rect_stroke(r, CornerRadius::same(6), st, StrokeKind::Outside);
                                let rs = rs.on_hover_text(tr!("picker.saved_hint", hex = to_hex(*sc)));
                                if rs.clicked() {
                                    *c = *sc;
                                    out.color = true;
                                }
                                if rs.secondary_clicked() {
                                    remove = saved.iter().position(|x| x == sc);
                                }
                            }
                            None => {
                                let (r, rs) = ui.allocate_exact_size(vec2(cell.x * 2.0 + 5.0, cell.y), Sense::click());
                                ui.painter().rect_filled(r, CornerRadius::same(6), if rs.hovered() { ACCENT2 } else { ACCENT });
                                ui.painter().text(r.center(), Align2::CENTER_CENTER, tr!("picker.save"), theme::semibold(11.5), Color32::from_rgb(6, 34, 44));
                                if rs.on_hover_cursor(CursorIcon::PointingHand).clicked() && !saved.contains(c) {
                                    saved.push(*c);
                                    out.saved = true;
                                }
                            }
                        }
                    }
                });
            }
            if let Some(i) = remove {
                saved.remove(i);
                out.saved = true;
            }
            ui.label(RichText::new(tr!("picker.help")).size(10.5).color(MUTED));
        });
    out
}

fn hsv_to_rgb(h: f32, s: f32, v: f32) -> [u8; 3] {
    let h = (h.rem_euclid(1.0)) * 6.0;
    let i = h.floor();
    let f = h - i;
    let (p, q, t) = (v * (1.0 - s), v * (1.0 - s * f), v * (1.0 - s * (1.0 - f)));
    let (r, g, b) = match i as i32 {
        0 => (v, t, p),
        1 => (q, v, p),
        2 => (p, v, t),
        3 => (p, q, v),
        4 => (t, p, v),
        _ => (v, p, q),
    };
    [(r * 255.0).round() as u8, (g * 255.0).round() as u8, (b * 255.0).round() as u8]
}

fn rgb_to_hsv(c: [u8; 3]) -> [f32; 3] {
    let [r, g, b] = c.map(|x| x as f32 / 255.0);
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let d = max - min;
    let h = if d == 0.0 {
        0.0
    } else if max == r {
        ((g - b) / d).rem_euclid(6.0) / 6.0
    } else if max == g {
        ((b - r) / d + 2.0) / 6.0
    } else {
        ((r - g) / d + 4.0) / 6.0
    };
    [h, if max == 0.0 { 0.0 } else { d / max }, max]
}

/// Saturation × value area and a rainbow hue slider. Returns true on change.
fn hsv_picker(ui: &mut Ui, id: egui::Id, c: &mut [u8; 3]) -> bool {
    use egui::epaint::{Mesh, Vertex, WHITE_UV};
    // Remember HSV so the hue is not lost for grey/black colors.
    let mut hsv = ui
        .data(|d| d.get_temp::<[f32; 3]>(id))
        .filter(|h| hsv_to_rgb(h[0], h[1], h[2]) == *c)
        .unwrap_or_else(|| rgb_to_hsv(*c));
    let mut changed = false;
    let col = |h: f32, s: f32, v: f32| {
        let [r, g, b] = hsv_to_rgb(h, s, v);
        Color32::from_rgb(r, g, b)
    };

    // Saturation (horizontal) × value (vertical) area.
    let (rect, resp) = ui.allocate_exact_size(vec2(250.0, 150.0), Sense::click_and_drag());
    let mut mesh = Mesh::default();
    const N: u32 = 16;
    for yi in 0..=N {
        for xi in 0..=N {
            let (s, v) = (xi as f32 / N as f32, 1.0 - yi as f32 / N as f32);
            let pos = rect.min + vec2(rect.width() * s, rect.height() * (1.0 - v));
            mesh.vertices.push(Vertex { pos, uv: WHITE_UV, color: col(hsv[0], s, v) });
        }
    }
    for yi in 0..N {
        for xi in 0..N {
            let i = yi * (N + 1) + xi;
            mesh.indices.extend([i, i + 1, i + N + 2, i, i + N + 2, i + N + 1]);
        }
    }
    ui.painter().add(mesh);
    ui.painter().rect_stroke(rect, CornerRadius::same(2), Stroke::new(1.0, BORDER), StrokeKind::Outside);
    if let Some(p) = resp.interact_pointer_pos().filter(|_| resp.dragged() || resp.clicked()) {
        hsv[1] = ((p.x - rect.left()) / rect.width()).clamp(0.0, 1.0);
        hsv[2] = 1.0 - ((p.y - rect.top()) / rect.height()).clamp(0.0, 1.0);
        changed = true;
    }
    let knob = rect.min + vec2(rect.width() * hsv[1], rect.height() * (1.0 - hsv[2]));
    ui.painter().circle_stroke(knob, 6.0, Stroke::new(2.0, Color32::WHITE));
    ui.painter().circle_stroke(knob, 7.5, Stroke::new(1.0, Color32::from_black_alpha(120)));

    // Hue slider.
    ui.add_space(4.0);
    let (bar, resp) = ui.allocate_exact_size(vec2(250.0, 14.0), Sense::click_and_drag());
    let mut mesh = Mesh::default();
    const H: u32 = 36;
    for k in 0..=H {
        let h = k as f32 / H as f32;
        let x = bar.left() + bar.width() * h;
        let cc = col(h, 1.0, 1.0);
        mesh.vertices.push(Vertex { pos: egui::pos2(x, bar.top()), uv: WHITE_UV, color: cc });
        mesh.vertices.push(Vertex { pos: egui::pos2(x, bar.bottom()), uv: WHITE_UV, color: cc });
    }
    for k in 0..H {
        let i = k * 2;
        mesh.indices.extend([i, i + 1, i + 3, i, i + 3, i + 2]);
    }
    ui.painter().add(mesh);
    if let Some(p) = resp.interact_pointer_pos().filter(|_| resp.dragged() || resp.clicked()) {
        hsv[0] = ((p.x - bar.left()) / bar.width()).clamp(0.0, 0.9999);
        changed = true;
    }
    let kx = bar.left() + bar.width() * hsv[0];
    ui.painter().circle_filled(egui::pos2(kx, bar.center().y), 8.0, col(hsv[0], 1.0, 1.0));
    ui.painter().circle_stroke(egui::pos2(kx, bar.center().y), 8.0, Stroke::new(2.0, Color32::WHITE));

    if changed {
        *c = hsv_to_rgb(hsv[0], hsv[1], hsv[2]);
    }
    ui.data_mut(|d| d.insert_temp(id, hsv));
    changed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hsv_roundtrip_and_hex() {
        for c in [[170, 0, 255], [0, 255, 64], [255, 105, 0], [12, 34, 56], [255, 255, 255], [0, 0, 0]] {
            let h = rgb_to_hsv(c);
            assert_eq!(hsv_to_rgb(h[0], h[1], h[2]), c);
        }
        assert_eq!(parse_hex("#AA00FF"), Some([170, 0, 255]));
        assert_eq!(parse_hex("aa00ff"), Some([170, 0, 255]));
        assert_eq!(parse_hex("#AA00F"), None);
        assert_eq!(to_hex([170, 0, 255]), "#AA00FF");
    }
}
