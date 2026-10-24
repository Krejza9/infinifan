//! SL Infinity fan preview: square frame with a continuous LED strip on each side
//! (outer light; both sides run in sync top to bottom), a smoothly lit blade ring in the middle
//! (inner light, 8 LEDs around) and a dark hub. Fans are stacked vertically.
//! LED colours come from the effect model (`fxsim`); effects it does not cover are drawn with
//! an approximate animation.

use std::f32::consts::{PI, TAU};

use eframe::egui::epaint::{Mesh, Vertex, WHITE_UV};
use eframe::egui::{self, Color32, CornerRadius, Pos2, Rect, Sense, Shape, Stroke, StrokeKind, Ui, vec2};

use crate::effects::{self, EffectSetting, GroupLighting, LightingConfig, Scope};
use crate::fxsim::{self, Rgb8};

const INNER_LEDS: usize = 8;
/// Positions on one side of the frame (12 LEDs = 6 per side, both sides identical).
const SIDE_LEDS: usize = 6;

type Rgb = [f32; 3];
const BLACK: Rgb = [0.0; 3];

fn fr(x: f32) -> f32 {
    x - x.floor()
}

fn tri(x: f32) -> f32 {
    1.0 - (fr(x) * 2.0 - 1.0).abs()
}

fn mix(a: Rgb, b: Rgb, k: f32) -> Rgb {
    [a[0] + (b[0] - a[0]) * k, a[1] + (b[1] - a[1]) * k, a[2] + (b[2] - a[2]) * k]
}

fn sc(c: Rgb, k: f32) -> Rgb {
    let k = k.clamp(0.0, 1.0);
    [c[0] * k, c[1] * k, c[2] * k]
}

fn hsv(h: f32) -> Rgb {
    let h = fr(h) * 6.0;
    let x = 1.0 - (h % 2.0 - 1.0).abs();
    let c = match h as i32 {
        0 => [1.0, x, 0.0],
        1 => [x, 1.0, 0.0],
        2 => [0.0, 1.0, x],
        3 => [0.0, x, 1.0],
        4 => [x, 0.0, 1.0],
        _ => [1.0, 0.0, x],
    };
    [c[0] * 255.0, c[1] * 255.0, c[2] * 255.0]
}

fn hash(n: f32) -> f32 {
    fr((n * 127.1 + 311.7).sin() * 43758.547)
}

fn to_color(c: Rgb) -> Color32 {
    Color32::from_rgb(c[0].clamp(0.0, 255.0) as u8, c[1].clamp(0.0, 255.0) as u8, c[2].clamp(0.0, 255.0) as u8)
}

/// Parameters of one ring for computing LEDs.
struct Fx<'a> {
    base: &'a str,
    colors: Vec<Rgb>,
    speed: f32,
    dir: f32,
    merge: bool,
}

impl<'a> Fx<'a> {
    fn new(s: &'a EffectSetting, merge: bool) -> Self {
        // Mode id without the _Inner/_Outer/_Merge suffix → shared animation.
        let base = s.mode.split('_').next().unwrap_or("");
        let count = effects::mode(&s.mode).map(|m| m.color_count()).unwrap_or(0);
        let colors = s.colors.iter().take(count).map(|c| [c[0] as f32, c[1] as f32, c[2] as f32]).collect();
        // Calibrated against real fans (the preview was slower than the hardware).
        let speed = match s.speed {
            0 => 0.8,
            25 => 1.3,
            75 => 2.7,
            100 => 3.8,
            _ => 1.9,
        };
        Self { base, colors, speed, dir: if s.reverse { -1.0 } else { 1.0 }, merge }
    }

    fn pal(&self, k: i64) -> Rgb {
        if self.colors.is_empty() {
            return [255.0; 3];
        }
        let n = self.colors.len() as i64;
        self.colors[k.rem_euclid(n) as usize]
    }

    /// Colour of LED `i` of `n` on fan `fan` (of `fans`) at time `t`.
    fn led(&self, i: usize, n: usize, fan: usize, fans: usize, t: f32) -> Rgb {
        let d = self.dir;
        let p = if self.merge { (fan as f32 + i as f32 / n as f32) / fans as f32 } else { i as f32 / n as f32 };
        let tt = t * self.speed;
        let pal = |k: f32| self.pal(k.floor() as i64);
        let breath = |x: f32| (PI * fr(x)).sin().powi(2);
        let beat = |x: f32| {
            let f = fr(x);
            (-((f - 0.1) / 0.04).powi(2)).exp().max(0.7 * (-((f - 0.28) / 0.05).powi(2)).exp())
        };
        let meteor = |head: f32, tail: f32| (1.0 - fr(d * (head - p)) / tail).max(0.0).powf(1.6);

        match self.base {
            "StaticColor" => self.pal(fan as i64),
            "Breathing" => sc(self.pal(fan as i64), 0.05 + 0.95 * breath(tt * 0.35)),
            "BreathingRainbow" => sc(hsv((tt * 0.35).floor() * 0.17), 0.05 + 0.95 * breath(tt * 0.35)),
            "Rainbow" => hsv(p - d * tt * 0.3),
            "RainbowMorph" => hsv(tt * 0.12),
            "Meteor" => sc(pal(tt * 0.5), meteor(tt * 0.5, 0.35)),
            "MeteorRainbow" => sc(hsv(p), meteor(tt * 0.5, 0.4)),
            "ColorfulMeteor" => sc(hsv(tt * 0.2), meteor(tt * 0.5, 0.45)),
            "DoubleMeteor" => sc(pal(tt * 0.5), meteor(tt * 0.5, 0.25).max(meteor(tt * 0.5 + 0.5, 0.25))),
            "MeteorContest" => {
                let a = meteor(tt * 0.45, 0.25);
                let b = (1.0 - fr(-d * (tt * 0.45 - p)) / 0.25).max(0.0).powf(1.6);
                if a >= b { sc(self.pal(0), a) } else { sc(self.pal(1), b) }
            }
            "MeteorMix" => {
                let g = fr(d * (tt * 0.5 - p));
                sc(mix(self.pal(0), self.pal(1), (g / 0.4).min(1.0)), (1.0 - g / 0.45).max(0.0).powf(1.3))
            }
            "Runway" | "Taichi" => {
                let x = fr(p - d * tt * 0.3);
                let k = if self.base == "Taichi" { 0.5 + 0.5 * (TAU * x).cos() } else if x < 0.5 { 0.0 } else { 1.0 };
                mix(self.pal(0), self.pal(1), k)
            }
            "ColorCycle" => self.pal((fr(p - d * tt * 0.2) * self.colors.len().max(1) as f32).floor() as i64),
            "MopUp" => {
                let k = (tt * 0.4).floor();
                if fr(d * p) < fr(tt * 0.4) { pal(k) } else { pal(k - 1.0) }
            }
            "Stack" => self.stack(p, if self.merge { n * fans } else { n }, tt),
            "Tide" => sc(pal(tt * 0.25), 0.12 + 0.88 * (0.5 + 0.5 * (TAU * (p * 2.0 - d * tt * 0.4)).sin()).powi(2)),
            "Scan" | "Reflect" => {
                let q = tri(tt * 0.35);
                let dist = if self.base == "Reflect" {
                    (p - q / 2.0).abs().min((p - (1.0 - q / 2.0)).abs())
                } else {
                    (p - q).abs()
                };
                sc(self.pal(0), (1.0 - dist / 0.14).max(0.0))
            }
            "Door" => {
                if (p - 0.5).abs() * 2.0 < tri(tt * 0.3) { pal(tt * 0.3) } else { sc(self.pal(0), 0.03) }
            }
            "Warning" => {
                if fr(tt * 1.2) < 0.5 { pal(tt * 1.2) } else { BLACK }
            }
            "Voice" => {
                let lvl = 0.25 + 0.7 * (0.5 + 0.5 * (tt * 3.1).sin() * (tt * 1.7 + fan as f32).sin());
                let x = (p - 0.5).abs() * 2.0;
                if x < lvl { mix(self.pal(0), self.pal(1), x) } else { sc(self.pal(0), 0.04) }
            }
            "Mixing" => mix(self.pal(0), self.pal(1), 0.5 + 0.5 * (tt * 1.6).sin()),
            "HeartBeat" => sc(self.pal(0), 0.04 + 0.96 * beat(tt * 0.6)),
            "HeartBeatRunway" => sc(self.pal(0), (0.04 + 0.96 * beat(tt * 0.6)) * (0.3 + 0.7 * meteor(tt * 0.6, 0.6))),
            "Disco" => {
                let k = (tt * 4.0).floor();
                if hash(k * 31.0 + i as f32 * 7.0 + fan as f32 * 131.0) > 0.45 {
                    self.pal((hash(k * 17.0 + i as f32 + fan as f32 * 53.0) * 4.0).floor() as i64)
                } else {
                    BLACK
                }
            }
            "Lottery" => {
                let k = (tt * 3.0).floor();
                if i == (hash(k + fan as f32 * 9.0) * n as f32).floor() as usize { pal(k) } else { sc(self.pal(0), 0.05) }
            }
            "ElectricCurrent" => {
                let g = fr(d * (tt * 0.8 - p));
                let fl = hash((tt * 20.0).floor() + i as f32 * 3.0 + fan as f32 * 11.0);
                if g < 0.25 {
                    sc(mix(self.pal(0), [255.0; 3], 0.5), 0.3 + 0.7 * fl)
                } else {
                    sc(self.pal(0), if fl > 0.92 { 0.5 } else { 0.03 })
                }
            }
            "ReturnArc" => {
                if p.min(1.0 - p) < tri(tt * 0.35) * 0.5 { pal(tt * 0.35) } else { sc(self.pal(0), 0.03) }
            }
            "DoubleArc" => {
                if p.min(1.0 - p).min((p - 0.5).abs()) < tri(tt * 0.35) * 0.25 { pal(tt * 0.35) } else { sc(self.pal(0), 0.03) }
            }
            _ => BLACK,
        }
    }

    /// Stacking: a light travels to the end, the next one up to it… until full; then again in the next colour.
    fn stack(&self, p: f32, n: usize, tt: f32) -> Rgb {
        let n_f = n as f32;
        let total = n_f * (n_f + 1.0) / 2.0;
        let cycle = total + n_f * 0.6;
        let steps = tt * n_f * 0.9;
        let c = (steps / cycle).floor();
        let mut rem = steps - c * cycle;
        let mut k = 0.0;
        while k < n_f && rem >= n_f - k {
            rem -= n_f - k;
            k += 1.0;
        }
        let x = fr(if self.dir > 0.0 { p } else { 1.0 - p }) * n_f;
        let col = self.pal(c as i64);
        if k >= n_f || x >= n_f - k {
            col
        } else if (x - rem).abs() < 0.6 {
            col
        } else {
            sc(self.pal(c as i64 + 1), 0.04)
        }
    }
}

/// Smooth interpolation between LEDs (diffuser) – `wrap` for a ring, otherwise for a strip.
fn smooth(arr: &[Rgb], p: f32, wrap: bool) -> Rgb {
    let n = arr.len() as i64;
    let x = p * n as f32 - 0.5;
    let a = x.floor();
    let f = x - a;
    let (mut i0, mut i1) = (a as i64, a as i64 + 1);
    if wrap {
        i0 = i0.rem_euclid(n);
        i1 = i1.rem_euclid(n);
    } else {
        i0 = i0.clamp(0, n - 1);
        i1 = i1.clamp(0, n - 1);
    }
    mix(arr[i0 as usize], arr[i1 as usize], f)
}

fn brightness_scale(b: u8) -> f32 {
    match b {
        0 => 0.0,
        25 => 0.25,
        50 => 0.5,
        75 => 0.75,
        _ => 1.0,
    }
}

#[derive(Default)]
pub struct FanPreview {
    angle: f32,
    last: Option<f64>,
    clock: Option<EffectClock>,
}

/// Time since the shown lighting was applied (restarts on every change, like sending it to the controller).
struct EffectClock {
    key: (LightingConfig, [u8; 4]),
    t_ms: f64,
}

impl FanPreview {
    /// Column of `fans` fans stacked vertically, `width` wide.
    /// `quantity` = fan count of all groups (merged effects depend on all of them).
    pub fn show(&mut self, ui: &mut Ui, width: f32, cfg: &LightingConfig, group: usize, quantity: [u8; 4], rpm: u16, lights_on: bool) {
        let fans = quantity[group].max(1) as usize;
        let now = ui.input(|i| i.time);
        let elapsed = self.last.map(|l| now - l).unwrap_or(0.0);
        let dt = (elapsed as f32).min(0.1);
        self.last = Some(now);
        // Real speed would just be a blur; the preview spins at ~1/40.
        self.angle = (self.angle + dt * rpm as f32 / 60.0 * TAU / 40.0).rem_euclid(TAU);
        let t = now as f32;

        let fans = fans.clamp(1, 4);
        let (rect, _) = ui.allocate_exact_size(vec2(width, width * fans as f32), Sense::hover());
        let p = ui.painter_at(rect.expand(2.0));

        let (inner, outer, merge): (&EffectSetting, &EffectSetting, bool) = match (&cfg.merged, &cfg.groups[group]) {
            (Some(m), _) => (m, m, true),
            (None, GroupLighting { individual: true, inner, outer, .. }) => (inner, outer, false),
            (None, gl) => (&gl.whole, &gl.whole, false),
        };
        let merge = merge && effects::mode(&inner.mode).is_some_and(|m| m.scope == Scope::Merge);
        // The side strips of all fans in a group are chained as one strip,
        // so the frame effect flows from the first fan to the last. The centre is separate.
        let (fi, fo) = (Fx::new(inner, merge), Fx::new(outer, true));
        let (bi, bo) = (brightness_scale(inner.brightness), brightness_scale(outer.brightness));

        // Effect model, otherwise approximate animation.
        let exact = if lights_on { self.model_rings(cfg, group, quantity, elapsed) } else { None };

        for f in 0..fans {
            let r = Rect::from_min_size(rect.min + vec2(0.0, f as f32 * width), vec2(width, width));
            let (ring, left, right): (Vec<Rgb>, Vec<Rgb>, Vec<Rgb>) = if !lights_on {
                (vec![[40.0; 3]; INNER_LEDS], vec![[40.0; 3]; SIDE_LEDS], vec![[40.0; 3]; SIDE_LEDS])
            } else if let Some((inner, outer)) = &exact {
                let fl = |c: &Rgb8| [c[0] as f32, c[1] as f32, c[2] as f32];
                let ring = inner[f * 8..f * 8 + 8].iter().map(fl).collect();
                // The 12 frame LEDs alternate left and right, top to bottom.
                let o = &outer[f * 12..f * 12 + 12];
                (ring, o.iter().step_by(2).map(fl).collect(), o.iter().skip(1).step_by(2).map(fl).collect())
            } else {
                let side: Vec<Rgb> = (0..SIDE_LEDS).map(|i| sc(fo.led(i, SIDE_LEDS, f, fans, t), bo)).collect();
                ((0..INNER_LEDS).map(|i| sc(fi.led(i, INNER_LEDS, f, fans, t), bi)).collect(), side.clone(), side)
            };
            draw_fan(
                &p,
                r,
                self.angle + f as f32 * 0.7,
                |u| smooth(&ring, u, true),
                |u| smooth(&left, u, false),
                |u| smooth(&right, u, false),
            );
        }
        // ~30 frames/s is enough for the preview animation (saves CPU).
        ui.ctx().request_repaint_after(std::time::Duration::from_millis(33));
    }
}

impl FanPreview {
    /// (inner 32 LEDs, outer 48 LEDs) of the group from the effect model, advanced by the elapsed time.
    fn model_rings(&mut self, cfg: &LightingConfig, group: usize, quantity: [u8; 4], elapsed: f64) -> Option<(Vec<Rgb8>, Vec<Rgb8>)> {
        if !fxsim::supports(cfg) {
            self.clock = None;
            return None;
        }
        let clock = match &mut self.clock {
            Some(c) if c.key.0 == *cfg && c.key.1 == quantity => c,
            slot => slot.insert(EffectClock { key: (cfg.clone(), quantity), t_ms: 0.0 }),
        };
        // After the window was hidden, don't jump ahead whole minutes – just continue smoothly.
        clock.t_ms += (elapsed * 1000.0).clamp(0.0, 200.0);
        let leds = fxsim::render(cfg, quantity, clock.t_ms)[group];
        Some((leds[..32].to_vec(), leds[32..].to_vec()))
    }
}

fn draw_fan(
    p: &egui::Painter,
    r: Rect,
    angle: f32,
    inner: impl Fn(f32) -> Rgb,
    left: impl Fn(f32) -> Rgb,
    right: impl Fn(f32) -> Rgb,
) {
    let s = r.width() / 200.0; // drawing coordinates are 0..200
    let at = |x: f32, y: f32| r.min + vec2(x * s, y * s);
    let c = at(100.0, 100.0);

    // Frame.
    p.rect_filled(r.shrink(0.5), CornerRadius::same((5.0 * s) as u8), Color32::from_rgb(38, 40, 45));
    p.rect_stroke(r.shrink(0.5), CornerRadius::same((5.0 * s) as u8), Stroke::new(1.0, Color32::from_rgb(52, 55, 62)), StrokeKind::Inside);
    p.rect_filled(r.shrink(5.0 * s), CornerRadius::same((3.0 * s) as u8), Color32::from_rgb(43, 45, 51));

    // Side strips: a continuous line corner–side–corner, top to bottom. Drawn as a triangle mesh
    // with a colour at each vertex (smooth gradient at the cost of a single shape).
    let sides: [(&dyn Fn(f32) -> Rgb, [f32; 4]); 2] =
        [(&left, [52.0, 10.0, 10.0, 52.0]), (&right, [148.0, 190.0, 190.0, 148.0])];
    for (side, xs) in sides {
        let pts = [at(xs[0], 10.0), at(xs[1], 52.0), at(xs[2], 148.0), at(xs[3], 190.0)];
        let samples = polyline_samples(&pts, 24);
        for (width, glow) in [(10.0, true), (3.6, false)] {
            let col = |u: f32| {
                let c = side(u);
                to_color(if glow { sc(c, 0.22) } else { c })
            };
            p.add(strip_mesh(&samples, width * s, &col));
            p.circle_filled(samples[0].0, width * s / 2.0, col(0.0));
            p.circle_filled(samples[samples.len() - 1].0, width * s / 2.0, col(1.0));
        }
    }

    // Blade ring: 8 smoothly blended LEDs as a single annulus coloured along its circumference.
    p.circle_filled(c, 86.0 * s, Color32::from_rgb(29, 31, 36));
    {
        const SEG: u32 = 48;
        let mut mesh = Mesh::default();
        for j in 0..=SEG {
            let u = j as f32 / SEG as f32;
            let a = u * TAU - TAU / 4.0;
            let dir = vec2(a.cos(), a.sin());
            let col = to_color(inner(u));
            mesh.vertices.push(Vertex { pos: c + dir * 20.0 * s, uv: WHITE_UV, color: col });
            mesh.vertices.push(Vertex { pos: c + dir * 82.0 * s, uv: WHITE_UV, color: col });
        }
        for j in 0..SEG {
            let i = j * 2;
            mesh.indices.extend([i, i + 1, i + 3, i, i + 3, i + 2]);
        }
        p.add(mesh);
    }
    // Light near the hub and a darker edge: radial alpha gradient in a single mesh.
    p.add(radial_mesh(
        c,
        s,
        &[
            (24.0, Color32::from_white_alpha(16)),
            (46.0, Color32::TRANSPARENT),
            (70.0, Color32::TRANSPARENT),
            (82.0, Color32::from_black_alpha(70)),
        ],
    ));

    // Blades – thin lines, spinning.
    for b in 0..9 {
        let a = angle + b as f32 / 9.0 * TAU;
        let pts: Vec<Pos2> = (0..=10)
            .map(|k| {
                let u = k as f32 / 10.0;
                let rad = (1.0 - u).powi(2) * 26.0 + 2.0 * (1.0 - u) * u * 52.0 + u * u * 80.0;
                let da = 2.0 * (1.0 - u) * u * 0.2 + u * u * 0.45;
                c + vec2((a + da).cos(), (a + da).sin()) * rad * s
            })
            .collect();
        p.add(Shape::line(pts, Stroke::new(1.4 * s, Color32::from_white_alpha(30))));
    }

    // Hub.
    p.circle_filled(c, 24.0 * s, Color32::from_rgb(17, 18, 22));
    p.circle_stroke(c, 24.0 * s, Stroke::new(3.0 * s, to_color(inner(0.0))));
    p.circle_filled(c, 19.0 * s, Color32::from_rgb(8, 9, 11));
}

/// Points along a polyline with parameter 0..1 (by length).
fn polyline_samples(pts: &[Pos2], n: usize) -> Vec<(Pos2, f32)> {
    let lens: Vec<f32> = pts.windows(2).map(|w| w[0].distance(w[1])).collect();
    let total: f32 = lens.iter().sum();
    (0..=n)
        .map(|k| {
            let mut d = total * k as f32 / n as f32;
            for (i, l) in lens.iter().enumerate() {
                if d <= *l || i + 1 == lens.len() {
                    let u = (d / l).clamp(0.0, 1.0);
                    return (pts[i] + (pts[i + 1] - pts[i]) * u, k as f32 / n as f32);
                }
                d -= l;
            }
            (pts[pts.len() - 1], 1.0)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fx(mode: &str) -> EffectSetting {
        EffectSetting::new(effects::mode(mode).unwrap())
    }

    #[test]
    fn stack_fills_up() {
        let s = fx("Stack_Outer");
        let f = Fx::new(&s, false);
        // At the start of a cycle only the moving light is lit; at the end (before the pause) the strip is full.
        let lit = |t: f32| (0..6).filter(|i| f.led(*i, 6, 0, 1, t)[0] + f.led(*i, 6, 0, 1, t)[2] > 100.0).count();
        assert!(lit(0.01) <= 1);
        // total = 21 steps, speed 2.7 × 6 × 0.9 = 14.58 steps/s → full from 1.44 s to 1.69 s.
        assert_eq!(lit(1.5), 6);
    }

    /// Meteor on the frame across 3 fans: the head moves from the first fan to the last.
    #[test]
    fn frame_meteor_spans_fans() {
        let s = fx("Meteor_Outer");
        let f = Fx::new(&s, true);
        let brightest = |t: f32| {
            (0..3)
                .flat_map(|fan| (0..6).map(move |i| (fan, i)))
                .max_by(|a, b| {
                    let la = f.led(a.1, 6, a.0, 3, t).iter().sum::<f32>();
                    let lb = f.led(b.1, 6, b.0, 3, t).iter().sum::<f32>();
                    la.total_cmp(&lb)
                })
                .unwrap()
                .0
        };
        // speed 4 = 2.7×, head = fr(t × 1.35): 0.1 → 1st fan, 0.5 → 2nd, 0.85 → 3rd.
        assert_eq!(brightest(0.1 / 1.35), 0);
        assert_eq!(brightest(0.5 / 1.35), 1);
        assert_eq!(brightest(0.85 / 1.35), 2);
    }

    #[test]
    fn static_uses_fan_color() {
        let mut s = fx("StaticColor_Inner");
        s.colors = vec![[255, 0, 0], [0, 255, 0], [0, 0, 255], [255, 255, 255]];
        let f = Fx::new(&s, false);
        assert_eq!(f.led(0, 8, 1, 3, 0.0), [0.0, 255.0, 0.0]);
    }
}

/// Strip of the given width along the points, coloured by parameter u (0..1) – one triangle mesh.
fn strip_mesh(samples: &[(Pos2, f32)], width: f32, col: &dyn Fn(f32) -> Color32) -> Mesh {
    let mut mesh = Mesh::default();
    let n = samples.len();
    for i in 0..n {
        let prev = samples[i.saturating_sub(1)].0;
        let next = samples[(i + 1).min(n - 1)].0;
        let t = (next - prev).normalized();
        let normal = vec2(-t.y, t.x) * (width / 2.0);
        let (pos, u) = samples[i];
        let c = col(u);
        mesh.vertices.push(Vertex { pos: pos + normal, uv: WHITE_UV, color: c });
        mesh.vertices.push(Vertex { pos: pos - normal, uv: WHITE_UV, color: c });
    }
    for i in 0..(n as u32 - 1) {
        let k = i * 2;
        mesh.indices.extend([k, k + 1, k + 3, k, k + 3, k + 2]);
    }
    mesh
}

/// Concentric annuli coloured by radius (radial gradient).
fn radial_mesh(c: Pos2, s: f32, stops: &[(f32, Color32)]) -> Mesh {
    const SEG: u32 = 48;
    let mut mesh = Mesh::default();
    for (r, col) in stops {
        for j in 0..=SEG {
            let a = j as f32 / SEG as f32 * TAU;
            mesh.vertices.push(Vertex { pos: c + vec2(a.cos(), a.sin()) * *r * s, uv: WHITE_UV, color: *col });
        }
    }
    let row = SEG + 1;
    for ring in 0..(stops.len() as u32 - 1) {
        for j in 0..SEG {
            let a = ring * row + j;
            let b = a + row;
            mesh.indices.extend([a, b, b + 1, a, b + 1, a + 1]);
        }
    }
    mesh
}
