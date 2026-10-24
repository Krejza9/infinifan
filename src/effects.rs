//! SL Infinity lighting effects – mode bytes, defaults and menu order as used by L-Connect 3.

use serde::{Deserialize, Serialize};

use crate::device::{GROUP_COUNT, Rgb, SlInfinity};

const RED: Rgb = Rgb(255, 0, 0);
const BLUE: Rgb = Rgb(0, 0, 255);
const GREEN: Rgb = Rgb(0, 255, 85);
const ORANGE: Rgb = Rgb(255, 105, 0);
const WHITE: Rgb = Rgb(255, 255, 255);
const BLACK: Rgb = Rgb(0, 0, 0);

const RBGO: &[Rgb] = &[RED, BLUE, GREEN, ORANGE];
const RB: &[Rgb] = &[RED, BLUE];
const R: &[Rgb] = &[RED];
const DISCO: &[Rgb] = &[BLUE, ORANGE, BLUE, ORANGE];
const W4: &[Rgb] = &[WHITE, WHITE, WHITE, WHITE];
const NONE: &[Rgb] = &[];

/// Which part of the fan an effect applies to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Scope {
    /// Whole fan, written to the inner port (2g).
    Whole,
    Inner,
    Outer,
    /// Effect across all groups, port 0.
    Merge,
}

/// How the color array is built for an effect (same as L-Connect 3).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ColorLayout {
    /// 16 colors: a 4-color palette repeated 4×.
    Palette,
    /// 32 colors: 4 fans × 8 LEDs, color i = fan i.
    PerFanInner,
    /// 48 colors: 4 fans × 12 LEDs.
    PerFanOuter,
}

#[derive(Debug)]
pub struct ModeDef {
    /// Stable identifier (stored in the config); its base also keys the translated name.
    pub id: &'static str,
    pub scope: Scope,
    /// Effect byte on the wire. For "split" modes it applies to both the inner and outer port.
    pub byte: u8,
    /// Whole fan split into inner (2g) + outer (2g+1) with the same byte.
    pub split: bool,
    pub default_colors: &'static [Rgb],
    pub speed: bool,
    pub brightness: bool,
    pub direction: bool,
    /// Default speed (0/25/50/75/100).
    pub default_speed: u8,
    /// Merged counterpart of a whole-fan mode.
    pub merge: Option<&'static str>,
    /// Hidden firmware effect (not offered by L-Connect).
    pub hidden: bool,
    layout: ColorLayout,
}

impl ModeDef {
    /// Translated display name; effects with the same base id share one name.
    pub fn name(&self) -> String {
        let base = self.id.split('_').next().unwrap_or(self.id);
        let key = format!("fx.{base}");
        crate::tr!(&key)
    }

    pub fn color_count(&self) -> usize {
        self.default_colors.len()
    }
}

macro_rules! m {
    ($id:literal, $scope:ident, $byte:literal, $split:literal, $colors:ident, $s:literal, $d:literal, $spd:literal, $merge:expr, $layout:ident) => {
        ModeDef {
            id: $id,
            scope: Scope::$scope,
            byte: $byte,
            split: $split,
            default_colors: $colors,
            speed: $s,
            brightness: true,
            direction: $d,
            default_speed: $spd,
            merge: $merge,
            hidden: false,
            layout: ColorLayout::$layout,
        }
    };
}

/// Hidden firmware effect: 16-color palette, always has speed; brightness and direction per firmware.
macro_rules! h {
    ($id:literal, $scope:ident, $byte:literal, $split:literal, $colors:ident, $d:literal, $b:literal) => {
        ModeDef {
            id: $id,
            scope: Scope::$scope,
            byte: $byte,
            split: $split,
            default_colors: $colors,
            speed: true,
            brightness: $b,
            direction: $d,
            default_speed: 75,
            merge: None,
            hidden: true,
            layout: ColorLayout::Palette,
        }
    };
}

/// Order matches the L-Connect 3 menus.
pub static MODES: &[ModeDef] = &[
    // --- whole fan ---
    m!("Rainbow", Whole, 50, false, NONE, true, true, 75, None, Palette),
    m!("RainbowMorph", Whole, 63, false, NONE, true, false, 75, None, Palette),
    m!("StaticColor", Whole, 1, true, RBGO, false, false, 75, None, Palette),
    m!("Breathing", Whole, 2, true, RBGO, true, false, 75, None, Palette),
    m!("BreathingRainbow", Whole, 6, true, NONE, true, false, 75, None, Palette),
    m!("Runway", Whole, 26, true, RB, true, false, 75, Some("Runway_Merge"), Palette),
    m!("MopUp", Whole, 68, false, RBGO, true, false, 75, Some("MopUp_Merge"), Palette),
    m!("Meteor", Whole, 25, true, RBGO, true, true, 75, None, Palette),
    m!("Warning", Whole, 54, false, RBGO, true, false, 50, None, Palette),
    m!("Voice", Whole, 55, false, RB, true, true, 75, None, Palette),
    m!("Mixing", Whole, 56, false, RB, true, false, 75, Some("Mixing_Merge"), Palette),
    m!("Stack", Whole, 57, false, RB, true, true, 75, Some("Stack_Merge"), Palette),
    m!("Tide", Whole, 58, false, RBGO, true, false, 75, Some("Tide_Merge"), Palette),
    m!("Scan", Whole, 59, false, RB, true, false, 75, Some("Scan_Merge"), Palette),
    m!("Door", Whole, 60, false, RBGO, true, false, 75, Some("Door_Merge"), Palette),
    m!("HeartBeat", Whole, 66, false, R, true, false, 75, None, Palette),
    m!("HeartBeatRunway", Whole, 67, false, R, true, true, 75, Some("HeartBeatRunway_Merge"), Palette),
    m!("Disco", Whole, 35, true, DISCO, true, true, 75, None, Palette),
    m!("ElectricCurrent", Whole, 65, false, W4, true, false, 75, Some("ElectricCurrent_Merge"), Palette),
    // --- inner ring ---
    m!("Rainbow_Inner", Inner, 5, false, NONE, true, true, 75, None, Palette),
    m!("RainbowMorph_Inner", Inner, 4, false, NONE, true, false, 75, None, Palette),
    m!("StaticColor_Inner", Inner, 1, false, RBGO, false, false, 75, None, PerFanInner),
    m!("Breathing_Inner", Inner, 2, false, RBGO, true, false, 75, None, PerFanInner),
    m!("BreathingRainbow_Inner", Inner, 6, false, NONE, true, false, 75, None, Palette),
    m!("Taichi_Inner", Inner, 28, false, RB, true, true, 75, None, Palette),
    m!("ColorCycle_Inner", Inner, 24, false, RBGO, true, true, 75, None, Palette),
    m!("Runway_Inner", Inner, 26, false, RB, true, false, 75, None, Palette),
    m!("MopUp_Inner", Inner, 27, false, RBGO, true, false, 75, None, Palette),
    m!("Meteor_Inner", Inner, 25, false, RBGO, true, true, 75, None, Palette),
    m!("MeteorRainbow_Inner", Inner, 8, false, NONE, true, true, 75, None, Palette),
    m!("Lottery_Inner", Inner, 38, false, RB, true, true, 75, None, Palette),
    m!("Warning_Inner", Inner, 41, false, RBGO, true, false, 75, None, Palette),
    m!("Voice_Inner", Inner, 42, false, RBGO, true, true, 75, None, Palette),
    m!("Mixing_Inner", Inner, 43, false, RB, true, false, 75, None, Palette),
    m!("Stack_Inner", Inner, 44, false, RB, true, true, 75, None, Palette),
    m!("Tide_Inner", Inner, 45, false, RBGO, true, false, 75, None, Palette),
    m!("Scan_Inner", Inner, 46, false, R, true, false, 75, None, Palette),
    m!("DoubleMeteor_Inner", Inner, 29, false, RBGO, true, false, 75, None, Palette),
    m!("MeteorContest_Inner", Inner, 30, false, RB, true, true, 75, None, Palette),
    m!("MeteorMix_Inner", Inner, 31, false, RB, true, false, 75, None, Palette),
    m!("ReturnArc_Inner", Inner, 32, false, RBGO, true, true, 75, None, Palette),
    m!("DoubleArc_Inner", Inner, 33, false, RBGO, true, true, 75, None, Palette),
    m!("Door_Inner", Inner, 34, false, RBGO, true, false, 75, None, Palette),
    m!("HeartBeat_Inner", Inner, 36, false, R, true, false, 75, None, Palette),
    m!("HeartBeatRunway_Inner", Inner, 69, false, R, true, true, 75, None, Palette),
    m!("Disco_Inner", Inner, 35, false, DISCO, true, true, 75, None, Palette),
    // --- outer ring ---
    m!("Rainbow_Outer", Outer, 5, false, NONE, true, true, 75, None, Palette),
    m!("RainbowMorph_Outer", Outer, 4, false, NONE, true, false, 75, None, Palette),
    m!("StaticColor_Outer", Outer, 1, false, RBGO, false, false, 75, None, PerFanOuter),
    m!("Breathing_Outer", Outer, 2, false, RBGO, true, false, 75, None, PerFanOuter),
    m!("BreathingRainbow_Outer", Outer, 6, false, NONE, true, false, 75, None, Palette),
    m!("ColorCycle_Outer", Outer, 24, false, RBGO, true, true, 75, None, Palette),
    m!("Runway_Outer", Outer, 26, false, RB, true, false, 75, None, Palette),
    m!("MopUp_Outer", Outer, 27, false, RBGO, true, false, 75, None, Palette),
    m!("Meteor_Outer", Outer, 25, false, RBGO, true, true, 75, None, Palette),
    m!("MeteorRainbow_Outer", Outer, 8, false, NONE, true, true, 75, None, Palette),
    m!("ColorfulMeteor_Outer", Outer, 39, false, NONE, true, true, 75, None, Palette),
    m!("Lottery_Outer", Outer, 38, false, RB, true, true, 75, None, Palette),
    m!("Warning_Outer", Outer, 41, false, RBGO, true, false, 75, None, Palette),
    m!("Voice_Outer", Outer, 42, false, RBGO, true, true, 75, None, Palette),
    m!("Mixing_Outer", Outer, 43, false, RB, true, false, 75, None, Palette),
    m!("Stack_Outer", Outer, 44, false, RB, true, true, 75, None, Palette),
    m!("Tide_Outer", Outer, 45, false, RBGO, true, false, 75, None, Palette),
    m!("Scan_Outer", Outer, 46, false, R, true, false, 75, None, Palette),
    m!("Door_Outer", Outer, 34, false, RBGO, true, false, 75, None, Palette),
    m!("HeartBeat_Outer", Outer, 36, false, R, true, false, 75, None, Palette),
    m!("Disco_Outer", Outer, 35, false, DISCO, true, true, 75, None, Palette),
    m!("Reflect_Outer", Outer, 48, false, R, true, false, 75, None, Palette),
    // --- hidden effects of firmware 1.4 (found by firmware analysis and emulation) ---
    h!("FwStrobe", Whole, 9, true, R, true, false),
    h!("FwStrobe_Inner", Inner, 9, false, R, true, false),
    h!("FwStrobe_Outer", Outer, 9, false, R, true, false),
    h!("FwStrobeRainbow", Whole, 10, true, NONE, true, false),
    h!("FwStrobeRainbow_Inner", Inner, 10, false, NONE, true, false),
    h!("FwStrobeRainbow_Outer", Outer, 10, false, NONE, true, false),
    h!("FwWave", Whole, 11, true, NONE, false, false),
    h!("FwWave_Inner", Inner, 11, false, NONE, false, false),
    h!("FwWave_Outer", Outer, 11, false, NONE, false, false),
    h!("FwSparkle", Whole, 12, true, NONE, false, false),
    h!("FwSparkle_Inner", Inner, 12, false, NONE, false, false),
    h!("FwSparkle_Outer", Outer, 12, false, NONE, false, false),
    h!("FwTwinkle", Whole, 13, true, NONE, false, false),
    h!("FwTwinkle_Inner", Inner, 13, false, NONE, false, false),
    h!("FwTwinkle_Outer", Outer, 13, false, NONE, false, false),
    h!("FwRunner", Whole, 14, true, NONE, false, true),
    h!("FwRunner_Inner", Inner, 14, false, NONE, false, true),
    h!("FwRunner_Outer", Outer, 14, false, NONE, false, true),
    h!("FwConverge", Whole, 16, true, NONE, false, true),
    h!("FwConverge_Inner", Inner, 16, false, NONE, false, true),
    h!("FwConverge_Outer", Outer, 16, false, NONE, false, true),
    h!("FwConveyor_Inner", Inner, 21, false, NONE, true, true),
    h!("FwConveyor_Outer", Outer, 21, false, NONE, true, true),
    h!("FwConveyorWipe_Inner", Inner, 22, false, NONE, true, true),
    h!("FwConveyorWipe_Outer", Outer, 22, false, NONE, true, true),
    h!("FwPastel_Inner", Inner, 23, false, NONE, true, true),
    h!("FwPastel_Outer", Outer, 23, false, NONE, true, true),
    h!("FwComets_Inner", Inner, 49, false, RB, false, true),
    h!("FwComets_Outer", Outer, 49, false, RB, false, true),
    // --- merged across all fans ---
    m!("Runway_Merge", Merge, 70, false, RB, true, false, 75, None, Palette),
    m!("MopUp_Merge", Merge, 71, false, RBGO, true, false, 75, None, Palette),
    m!("Mixing_Merge", Merge, 72, false, RB, true, false, 75, None, Palette),
    m!("Stack_Merge", Merge, 73, false, RB, true, true, 75, None, Palette),
    m!("Tide_Merge", Merge, 74, false, RBGO, true, false, 75, None, Palette),
    m!("Scan_Merge", Merge, 75, false, RB, true, false, 75, None, Palette),
    m!("Door_Merge", Merge, 76, false, RBGO, true, false, 75, None, Palette),
    m!("HeartBeatRunway_Merge", Merge, 77, false, R, true, true, 75, None, Palette),
    m!("ElectricCurrent_Merge", Merge, 78, false, W4, true, false, 75, None, Palette),
];

/// "Rainbow, brightness off" byte – L-Connect 3 uses it to blank ports 1–7 during a merged effect.
const BLANK_MODE: u8 = 50;
const BRIGHTNESS_OFF: u8 = 8;
pub const SET_FRAME_SYNC: u16 = 1;

pub fn mode(id: &str) -> Option<&'static ModeDef> {
    MODES.iter().find(|m| m.id == id)
}

pub fn modes_for(scope: Scope) -> impl Iterator<Item = &'static ModeDef> {
    MODES.iter().filter(move |m| m.scope == scope)
}

/// One effect setting as the GUI holds it (values 0/25/50/75/100 like L-Connect 3).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EffectSetting {
    pub mode: String,
    pub colors: Vec<[u8; 3]>,
    pub speed: u8,
    pub brightness: u8,
    /// false = doprava, true = doleva
    pub reverse: bool,
}

impl EffectSetting {
    pub fn new(m: &ModeDef) -> Self {
        Self {
            mode: m.id.to_string(),
            colors: m.default_colors.iter().map(|c| [c.0, c.1, c.2]).collect(),
            speed: m.default_speed,
            brightness: 100,
            reverse: false,
        }
    }

    /// On a mode change keeps the user's colors where possible and pads/trims to the mode's color count.
    pub fn switch_mode(&mut self, m: &ModeDef) {
        let n = m.color_count();
        let mut colors: Vec<[u8; 3]> = self.colors.iter().copied().take(n).collect();
        for c in m.default_colors.iter().skip(colors.len()) {
            colors.push([c.0, c.1, c.2]);
        }
        self.mode = m.id.to_string();
        self.colors = colors;
        if !m.speed {
            self.speed = m.default_speed;
        }
    }
}

/// Lighting of one group (controller port).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GroupLighting {
    /// true = inner and outer ring separately (the L-Connect 3 default)
    pub individual: bool,
    pub whole: EffectSetting,
    pub inner: EffectSetting,
    pub outer: EffectSetting,
}

impl Default for GroupLighting {
    fn default() -> Self {
        Self {
            individual: true,
            whole: EffectSetting::new(mode("StaticColor").unwrap()),
            inner: EffectSetting::new(mode("StaticColor_Inner").unwrap()),
            outer: EffectSetting::new(mode("StaticColor_Outer").unwrap()),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LightingConfig {
    pub groups: [GroupLighting; GROUP_COUNT],
    /// Merged effect across all groups (takes precedence when Some).
    pub merged: Option<EffectSetting>,
}

impl Default for LightingConfig {
    fn default() -> Self {
        Self { groups: Default::default(), merged: None }
    }
}

fn speed_byte(v: u8) -> u8 {
    match v {
        0 => 0x02,
        25 => 0x01,
        75 => 0xFF,
        100 => 0xFE,
        _ => 0x00,
    }
}

/// GUI brightness → byte; the lowest (0) becomes Off (8), like L-Connect 3.
fn brightness_byte(v: u8) -> u8 {
    match v {
        0 => BRIGHTNESS_OFF,
        25 => 3,
        75 => 1,
        100 => 0,
        _ => 2,
    }
}

/// Like L-Connect 3, static colors are also dimmed in software by brightness.
fn static_scale(v: u8) -> f32 {
    match v {
        100 => 1.0,
        75 => 0.75,
        25 => 0.25,
        0 => 0.0,
        _ => 0.5,
    }
}

/// One port write: colors + effect parameters.
#[derive(Clone, Debug, PartialEq)]
pub struct PortWrite {
    pub port: u8,
    pub colors: Vec<Rgb>,
    pub mode: u8,
    pub speed: u8,
    pub direction: u8,
    pub brightness: u8,
}

fn build(port: u8, def: &ModeDef, layout: ColorLayout, s: &EffectSetting) -> PortWrite {
    let is_static = def.id.starts_with("StaticColor");
    let scale = if is_static { static_scale(s.brightness) } else { 1.0 };
    let user: Vec<Rgb> = s
        .colors
        .iter()
        .take(def.color_count())
        .map(|c| {
            let f = |v: u8| (v as f32 * scale).floor() as u8;
            Rgb(f(c[0]), f(c[1]), f(c[2]))
        })
        .collect();
    let at = |i: usize| user.get(i).copied().unwrap_or(BLACK);
    let colors = match layout {
        ColorLayout::Palette => (0..16).map(|i| at(i % 4)).collect(),
        ColorLayout::PerFanInner => (0..32).map(|i| at(i / 8)).collect(),
        ColorLayout::PerFanOuter => (0..48).map(|i| at(i / 12)).collect(),
    };
    PortWrite {
        port,
        colors,
        mode: def.byte,
        speed: if def.speed { speed_byte(s.speed) } else { 0x00 },
        direction: if def.direction { s.reverse as u8 } else { 0 },
        brightness: brightness_byte(s.brightness),
    }
}

/// Writes for the whole lighting config (always all 8 ports, even for a merged effect – this avoids
/// the L-Connect 3 bug where ports 1–7 kept the old effect after merge was turned off).
pub fn plan(cfg: &LightingConfig) -> (Vec<PortWrite>, bool) {
    if let Some(merged) = &cfg.merged
        && let Some(def) = mode(&merged.mode).filter(|d| d.scope == Scope::Merge)
    {
        let mut writes = vec![build(0, def, def.layout, merged)];
        for port in 1..8u8 {
            writes.push(PortWrite {
                port,
                colors: Vec::new(),
                mode: BLANK_MODE,
                speed: 0,
                direction: 0,
                brightness: BRIGHTNESS_OFF,
            });
        }
        return (writes, true);
    }

    let mut writes = Vec::with_capacity(8);
    for (g, gl) in cfg.groups.iter().enumerate() {
        let inner_port = (g * 2) as u8;
        let outer_port = inner_port + 1;
        if gl.individual {
            for (port, s, scope) in [(inner_port, &gl.inner, Scope::Inner), (outer_port, &gl.outer, Scope::Outer)] {
                if let Some(def) = mode(&s.mode).filter(|d| d.scope == scope) {
                    writes.push(build(port, def, def.layout, s));
                }
            }
        } else if let Some(def) = mode(&gl.whole.mode).filter(|d| d.scope == Scope::Whole) {
            if def.split {
                // Like L-Connect 3: StaticColor → StaticColor_Inner (32 LEDs) + StaticColor_Outer (48 LEDs) etc.
                let (li, lo) = match def.id {
                    "StaticColor" | "Breathing" => (ColorLayout::PerFanInner, ColorLayout::PerFanOuter),
                    _ => (ColorLayout::Palette, ColorLayout::Palette),
                };
                writes.push(build(inner_port, def, li, &gl.whole));
                writes.push(build(outer_port, def, lo, &gl.whole));
            } else {
                writes.push(build(inner_port, def, def.layout, &gl.whole));
            }
        }
    }
    (writes, false)
}

/// Sends the lighting to the device (colors, then effect; finally a frame sync like L-Connect 3).
pub fn apply(dev: &SlInfinity, cfg: &LightingConfig) -> anyhow::Result<()> {
    let (writes, merged) = plan(cfg);
    for w in &writes {
        dev.set_colors(w.port, &w.colors)?;
        dev.set_effect(w.port, w.mode, w.speed, w.direction, w.brightness)?;
    }
    if !merged {
        dev.set_frame(SET_FRAME_SYNC)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// L-Connect 3 first-run default: static colors; device.rs handles the R,B,G wire order.
    #[test]
    fn default_matches_original() {
        let (w, merged) = plan(&LightingConfig::default());
        assert!(!merged);
        assert_eq!(w.len(), 8);
        assert_eq!(w[0].port, 0);
        assert_eq!(w[0].mode, 1);
        assert_eq!(w[0].colors.len(), 32);
        assert_eq!(w[0].colors[8], BLUE);
        assert_eq!((w[0].speed, w[0].direction, w[0].brightness), (0, 0, 0));
        assert_eq!(w[1].port, 1);
        assert_eq!(w[1].colors.len(), 48);
        assert_eq!(w[1].colors[47], ORANGE);
    }

    #[test]
    fn merge_blanks_other_ports() {
        let cfg = LightingConfig {
            merged: Some(EffectSetting::new(mode("Tide_Merge").unwrap())),
            ..Default::default()
        };
        let (w, merged) = plan(&cfg);
        assert!(merged);
        assert_eq!((w[0].port, w[0].mode, w[0].speed), (0, 74, 0xFF));
        assert!(w[1..].iter().all(|p| p.mode == 50 && p.brightness == 8 && p.colors.is_empty()));
    }

    #[test]
    fn rainbow_whole_writes_inner_port_only() {
        let mut cfg = LightingConfig::default();
        for g in &mut cfg.groups {
            g.individual = false;
            g.whole = EffectSetting::new(mode("Rainbow").unwrap());
        }
        let (w, _) = plan(&cfg);
        assert_eq!(w.iter().map(|p| p.port).collect::<Vec<_>>(), vec![0, 2, 4, 6]);
        assert!(w.iter().all(|p| p.mode == 50 && p.colors == vec![BLACK; 16]));
    }

    #[test]
    fn static_brightness_scales_colors() {
        let mut s = EffectSetting::new(mode("StaticColor_Inner").unwrap());
        s.brightness = 50;
        let p = build(0, mode("StaticColor_Inner").unwrap(), ColorLayout::PerFanInner, &s);
        assert_eq!(p.colors[0], Rgb(127, 0, 0));
        assert_eq!(p.brightness, 2);
    }
}
