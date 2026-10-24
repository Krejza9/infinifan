//! Clean-room simulation of the SL Infinity lighting effects: what the LEDs show for a lighting
//! config `t` milliseconds after it was applied.
//!
//! Written only from the black-box behaviour specification in `research/fx-recordings/spec`
//! (`whole.md`, `ring.md`, `merge.md` and their reference models), which in turn was derived from
//! recorded LED output. Terminology (step counter `n`, speed index `i`, brightness shift `s`,
//! "level strip", "four-slot strip", "whole-fan strip", "spill", …) follows those documents.
//!
//! * [`effects`] – one module per effect with all its scopes: Whole (`whole.md`), Inner and
//!   Outer (`ring.md`) and Merge (`merge.md`);
//! * [`blocks`] – building blocks shared by several effects (scope views, step counters,
//!   bouncing segments, stacking, the whole-fan strip and its spill, brightness helpers);
//! * [`color`] – colour tables and per-channel helpers.
//!
//! This module maps mode ids to [`Effect`]s, dispatches them per scope and applies the
//! scope-wide rules (clearing, missing fans, brightness, spill).
//!
//! Everything here is allocation free; one [`render`] call costs a few microseconds.

mod blocks;
mod color;
mod effects;

#[cfg(test)]
mod tests;

use crate::effects::{EffectSetting, LightingConfig, Scope, mode};
use blocks::{Canvas, LATENCY, MAX_FANS, Ring, Strips, WholeFanStrip};
use color::{mul_div, shr};

pub type Rgb8 = [u8; 3];
/// LEDs of one group in device order: [0..32) inner ring (4 fans × 8, LED 0 at top, clockwise),
/// [32..80) outer frame (4 fans × 12, even = left strip top→bottom, odd = right strip top→bottom).
pub type GroupLeds = [Rgb8; 80];

pub(crate) const BLACK: Rgb8 = [0, 0, 0];
/// First LED of the outer frame in [`GroupLeds`].
pub(crate) const OUTER_BASE: usize = 32;
/// Inner LED → level (row) of the ring: LED 0 at the top = level 0, LED 4 at the bottom = level 4;
/// LEDs 1/7, 2/6 and 3/5 share a level (the "5-position" / "level" strip of the specs).
pub(crate) const LEVEL: [usize; 8] = [0, 1, 2, 3, 4, 3, 2, 1];

/// Effects the simulation knows, by the base of their mode id (`Runway_Inner` → `Runway`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Effect {
    Rainbow,
    RainbowMorph,
    StaticColor,
    Breathing,
    BreathingRainbow,
    Runway,
    MopUp,
    Meteor,
    Warning,
    Voice,
    Mixing,
    Stack,
    Tide,
    Scan,
    Door,
    HeartBeat,
    HeartBeatRunway,
    Disco,
    ElectricCurrent,
    Taichi,
    ColorCycle,
    MeteorRainbow,
    ColorfulMeteor,
    Lottery,
    DoubleMeteor,
    MeteorContest,
    MeteorMix,
    ReturnArc,
    DoubleArc,
    Reflect,
    FwStrobe,
    FwStrobeRainbow,
    FwWave,
    FwSparkle,
    FwTwinkle,
    FwRunner,
    FwConverge,
    FwConveyor,
    FwConveyorWipe,
    FwPastel,
}

impl Effect {
    fn from_mode_id(id: &str) -> Option<Self> {
        use Effect::*;
        Some(match id.split('_').next()? {
            "Rainbow" => Rainbow,
            "RainbowMorph" => RainbowMorph,
            "StaticColor" => StaticColor,
            "Breathing" => Breathing,
            "BreathingRainbow" => BreathingRainbow,
            "Runway" => Runway,
            "MopUp" => MopUp,
            "Meteor" => Meteor,
            "Warning" => Warning,
            "Voice" => Voice,
            "Mixing" => Mixing,
            "Stack" => Stack,
            "Tide" => Tide,
            "Scan" => Scan,
            "Door" => Door,
            "HeartBeat" => HeartBeat,
            "HeartBeatRunway" => HeartBeatRunway,
            "Disco" => Disco,
            "ElectricCurrent" => ElectricCurrent,
            "Taichi" => Taichi,
            "ColorCycle" => ColorCycle,
            "MeteorRainbow" => MeteorRainbow,
            "ColorfulMeteor" => ColorfulMeteor,
            "Lottery" => Lottery,
            "DoubleMeteor" => DoubleMeteor,
            "MeteorContest" => MeteorContest,
            "MeteorMix" => MeteorMix,
            "ReturnArc" => ReturnArc,
            "DoubleArc" => DoubleArc,
            "Reflect" => Reflect,
            "FwStrobe" => FwStrobe,
            "FwStrobeRainbow" => FwStrobeRainbow,
            "FwWave" => FwWave,
            "FwSparkle" => FwSparkle,
            "FwTwinkle" => FwTwinkle,
            "FwRunner" => FwRunner,
            "FwConverge" => FwConverge,
            "FwConveyor" => FwConveyor,
            "FwConveyorWipe" => FwConveyorWipe,
            "FwPastel" => FwPastel,
            _ => return None, // FwComets and anything unknown
        })
    }
}

/// One effect setting, resolved for rendering.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Fx<'a> {
    pub effect: Effect,
    /// Speed index `i = speed / 25`: 0 = slowest (GUI 0) … 4 = fastest (GUI 100).
    pub speed: i64,
    /// Brightness shift `s`: GUI 100/75/50/25/0 → 0/1/2/3/4 (one halving per 25 %).
    pub shift: u32,
    /// GUI brightness 0–100 (only StaticColor uses it directly: `floor(c · b / 100)`).
    pub brightness: u32,
    pub reverse: bool,
    /// The user palette `pal[0..]`, cut to the effect's colour count.
    pub pal: &'a [Rgb8],
}

impl Fx<'_> {
    /// whole.md's `D = (100 - speed) / 25` (4 = slowest … 0 = fastest).
    pub fn d(&self) -> f64 {
        (4 - self.speed) as f64
    }

    /// Palette colour `pal[k mod len]` (black for an empty palette).
    pub fn pal(&self, k: i64) -> Rgb8 {
        match self.pal.len() {
            0 => BLACK,
            n => self.pal[k.rem_euclid(n as i64) as usize],
        }
    }
}

// ----------------------------------------------------------------------------------- dispatch

/// Draws a Whole effect at full brightness on the 4 fan slots of a group.
type WholeFn = fn(&Fx, &mut Canvas, f64);
/// Draws an Inner or Outer effect on its own ring of the installed fans.
type RingFn = fn(&Fx, &mut Ring, f64);
/// Draws a Merge effect on the merged strips at `τ = t − LATENCY` (never negative).
type MergeFn = fn(&Fx, &mut Strips, f64);

/// Whole-fan variant of an effect, if implemented.
fn whole_fn(e: Effect) -> Option<WholeFn> {
    use Effect::*;
    use effects::*;
    let f: WholeFn = match e {
        Rainbow => rainbow::whole,
        RainbowMorph => rainbow_morph::whole,
        StaticColor => static_color::whole,
        Breathing => breathing::whole,
        BreathingRainbow => breathing_rainbow::whole,
        Runway => runway::whole,
        MopUp => mop_up::whole,
        Meteor => meteor::whole,
        Warning => warning::whole,
        Voice => voice::whole,
        Mixing => mixing::whole,
        Stack => stack::whole,
        Tide => tide::whole,
        Scan => scan::whole,
        Door => door::whole,
        HeartBeat => heart_beat::whole,
        HeartBeatRunway => heart_beat_runway::whole,
        Disco => disco::whole,
        ElectricCurrent => electric_current::whole,
        FwStrobe => fw_strobe::whole,
        FwStrobeRainbow => fw_strobe_rainbow::whole,
        FwWave => fw_wave::whole,
        FwSparkle => fw_sparkle::whole,
        FwTwinkle => fw_twinkle::whole,
        FwRunner => fw_runner::whole,
        FwConverge => fw_converge::whole,
        _ => return None,
    };
    Some(f)
}

/// Ring variant of an effect for the inner ring (`outer` = false) or the outer frame.
fn ring_fn(e: Effect, outer: bool) -> Option<RingFn> {
    use Effect::*;
    use effects::*;
    let f: RingFn = match (e, outer) {
        (Rainbow, _) => rainbow::ring,
        (RainbowMorph, _) => rainbow_morph::ring,
        (StaticColor, _) => static_color::ring,
        (Breathing, _) => breathing::ring,
        (BreathingRainbow, _) => breathing_rainbow::ring,
        (ColorCycle, _) => color_cycle::ring,
        (Runway, _) => runway::ring,
        (MopUp, _) => mop_up::ring,
        (Meteor, _) => meteor::ring,
        (MeteorRainbow, _) => meteor_rainbow::ring,
        (Lottery, _) => lottery::ring,
        (Warning, _) => warning::ring,
        (Voice, _) => voice::ring,
        (Mixing, _) => mixing::ring,
        (Stack, _) => stack::ring,
        (Tide, _) => tide::ring,
        (Scan, _) => scan::ring,
        (Door, _) => door::ring,
        (HeartBeat, _) => heart_beat::ring,
        (Disco, _) => disco::ring,
        (FwStrobe, _) => fw_strobe::ring,
        (FwStrobeRainbow, _) => fw_strobe_rainbow::ring,
        (FwWave, _) => fw_wave::ring,
        (FwSparkle, _) => fw_sparkle::ring,
        (FwTwinkle, _) => fw_twinkle::ring,
        (FwRunner, _) => fw_runner::ring,
        (FwConverge, _) => fw_converge::ring,
        (FwConveyor, _) => fw_conveyor::ring,
        (FwConveyorWipe, _) => fw_conveyor_wipe::ring,
        (FwPastel, _) => fw_pastel::ring,
        (Taichi, false) => taichi::inner,
        (DoubleMeteor, false) => double_meteor::inner,
        (MeteorContest, false) => meteor_contest::inner,
        (MeteorMix, false) => meteor_mix::inner,
        (ReturnArc, false) => return_arc::inner,
        (DoubleArc, false) => double_arc::inner,
        (HeartBeatRunway, false) => heart_beat_runway::inner,
        (ColorfulMeteor, true) => colorful_meteor::outer,
        (Reflect, true) => reflect::outer,
        _ => return None,
    };
    Some(f)
}

/// Merged variant of an effect, if implemented.
fn merge_fn(e: Effect) -> Option<MergeFn> {
    use Effect::*;
    use effects::*;
    let f: MergeFn = match e {
        Runway => runway::merge,
        MopUp => mop_up::merge,
        Mixing => mixing::merge,
        Stack => stack::merge,
        Tide => tide::merge,
        Scan => scan::merge,
        Door => door::merge,
        HeartBeatRunway => heart_beat_runway::merge,
        ElectricCurrent => electric_current::merge,
        _ => return None,
    };
    Some(f)
}

/// The whole-fan strip of the effects that spill into the other ring (ring.md §4.9).
fn whole_fan_strip(fx: &Fx, outer: bool, t: f64) -> Option<WholeFanStrip> {
    use effects::*;
    match fx.effect {
        Effect::FwConveyor => Some(fw_conveyor::strip(fx, outer, t)),
        Effect::FwConveyorWipe => Some(fw_conveyor_wipe::strip(fx, outer, t)),
        Effect::FwPastel => Some(fw_pastel::strip(fx, outer, t)),
        _ => None,
    }
}

/// Whole-fan effect on one group (whole.md §0.3): the effect draws at full brightness on a
/// canvas of 4 fan slots; afterwards fans `≥ q` are blanked and brightness is applied
/// (StaticColor linear, every other effect one halving per 25 %).
fn render_whole(fx: &Fx, draw: WholeFn, q: usize, t: f64, leds: &mut GroupLeds) {
    leds.fill(BLACK);
    if q == 0 {
        return;
    }
    draw(fx, &mut Canvas { leds: &mut *leds, q }, t);

    // Missing fans are black; then brightness (whole.md §0.3).
    for f in q..4 {
        leds[8 * f..8 * f + 8].fill(BLACK);
        leds[OUTER_BASE + 12 * f..OUTER_BASE + 12 * f + 12].fill(BLACK);
    }
    if fx.effect == Effect::StaticColor {
        for c in leds.iter_mut() {
            *c = mul_div(*c, fx.brightness, 100);
        }
    } else if fx.shift > 0 {
        for c in leds.iter_mut() {
            *c = shr(*c, fx.shift);
        }
    }
}

/// Ring effect on its own ring (`outer` = Outer scope) of one group (ring.md): it draws every
/// installed fan; fans `≥ q` stay black.
fn render_ring(fx: &Fx, draw: RingFn, outer: bool, q: usize, t: f64, leds: &mut GroupLeds) {
    let n_leds = if outer { 12 } else { 8 };
    let base = if outer { OUTER_BASE } else { 0 };
    leds[base..base + 4 * n_leds].fill(BLACK);
    if q == 0 {
        return;
    }
    draw(fx, &mut Ring { leds, base, n: n_leds, q, outer }, t);
}

/// Spill of the whole-fan strip effects (FwConveyor, FwConveyorWipe, FwPastel) into the other
/// ring, drawn after both rings have been rendered. Other effects do not spill.
fn spill(fx: &Fx, outer: bool, q: usize, t: f64, leds: &mut GroupLeds) {
    if let Some(strip) = whole_fan_strip(fx, outer, t) {
        strip.spill(outer, q, fx.shift, leds);
    }
}

/// Merged effect over the installed fans of all groups (merge.md).
fn render_merge(fx: &Fx, draw: MergeFn, q: [usize; 4], t: f64, out: &mut [GroupLeds; 4]) {
    let mut fans = [(0, 0); MAX_FANS];
    let mut n = 0;
    for (g, &qg) in q.iter().enumerate() {
        for f in 0..qg {
            fans[n] = (g, f);
            n += 1;
        }
    }
    if n == 0 {
        return;
    }
    let tau = (t - LATENCY).max(0.0);
    draw(fx, &mut Strips { out, fans, n, mb: 256 >> fx.shift }, tau);
}

// ---------------------------------------------------------------------------------- public API

fn resolve(s: &EffectSetting, scope: Scope) -> Option<Fx<'_>> {
    let def = mode(&s.mode).filter(|d| d.scope == scope)?;
    let effect = Effect::from_mode_id(def.id)?;
    let implemented = match scope {
        Scope::Whole => whole_fn(effect).is_some(),
        Scope::Inner => ring_fn(effect, false).is_some(),
        Scope::Outer => ring_fn(effect, true).is_some(),
        Scope::Merge => merge_fn(effect).is_some(),
    };
    if !implemented {
        return None;
    }
    let count = def.color_count();
    let pal = if count > 0 { &s.colors[..s.colors.len().min(count)] } else { &s.colors[..] };
    // Snap to the GUI steps (0/25/50/75/100); every timing in the specs is defined only there.
    let speed = ((u32::from(s.speed.min(100)) + 12) / 25) as i64;
    let brightness = u32::from(s.brightness.min(100));
    let shift = (100 - brightness + 12) / 25;
    Some(Fx { effect, speed, shift, brightness, reverse: s.reverse, pal })
}

/// The merged effect, if the config selects one (same rule as `effects::plan`).
fn merged_setting(cfg: &LightingConfig) -> Option<&EffectSetting> {
    cfg.merged.as_ref().filter(|m| mode(&m.mode).is_some_and(|d| d.scope == Scope::Merge))
}

/// True if every effect used by `cfg` is implemented (FwComets is not).
pub fn supports(cfg: &LightingConfig) -> bool {
    if let Some(m) = merged_setting(cfg) {
        return resolve(m, Scope::Merge).is_some();
    }
    cfg.groups.iter().all(|g| {
        if g.individual {
            resolve(&g.inner, Scope::Inner).is_some() && resolve(&g.outer, Scope::Outer).is_some()
        } else {
            resolve(&g.whole, Scope::Whole).is_some()
        }
    })
}

/// What the 4 groups show `t_ms` milliseconds after the lighting was applied.
/// `quantity` = installed fans per group (0–4); LEDs of missing fans are black.
/// LEDs driven by an unsupported effect stay black.
pub fn render(cfg: &LightingConfig, quantity: [u8; 4], t_ms: f64) -> [GroupLeds; 4] {
    let mut out = [[BLACK; 80]; 4];
    let t = if t_ms.is_finite() { t_ms.max(0.0) } else { 0.0 };
    let q = quantity.map(|n| usize::from(n.min(4)));

    if let Some(m) = merged_setting(cfg) {
        if let Some(fx) = resolve(m, Scope::Merge) {
            if let Some(draw) = merge_fn(fx.effect) {
                render_merge(&fx, draw, q, t, &mut out);
            }
        }
        return out;
    }

    for ((g, leds), &qg) in cfg.groups.iter().zip(out.iter_mut()).zip(q.iter()) {
        if g.individual {
            let inner = resolve(&g.inner, Scope::Inner);
            let outer = resolve(&g.outer, Scope::Outer);
            for (fx, is_outer) in [(&inner, false), (&outer, true)] {
                if let Some(fx) = fx {
                    if let Some(draw) = ring_fn(fx.effect, is_outer) {
                        render_ring(fx, draw, is_outer, qg, t, leds);
                    }
                }
            }
            // Whole-fan strip effects overwrite part of the other ring (ring.md §4.9).
            if let Some(fx) = &inner {
                spill(fx, false, qg, t, leds);
            }
            if let Some(fx) = &outer {
                spill(fx, true, qg, t, leds);
            }
        } else if let Some(fx) = resolve(&g.whole, Scope::Whole) {
            if let Some(draw) = whole_fn(fx.effect) {
                render_whole(&fx, draw, qg, t, leds);
            }
        }
    }
    out
}
