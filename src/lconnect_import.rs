//! One-time import of settings from L-Connect 3
//! (C:\ProgramData\Lian-Li\L-Connect 3\device\<md5>\*.0 = gzipped JSON).

use std::io::Read;
use std::path::PathBuf;

use serde::Deserialize;
use serde_json::Value;

use crate::config::AppConfig;
use crate::curve::{FanSize, Preset, Source};
use crate::effects::{self, EffectSetting, Scope};

/// Order of L-Connect 3's lighting mode enum (saved files store its ordinal, not the wire byte).
const LIGHTING_MODE_ORDINALS: &[&str] = &[
    "None", "Breathing", "BreathingRainbow", "ColorCycle", "ColorfulCity", "Contest", "Disco", "Door",
    "ElectricCurrent", "Groove", "HeartBeat", "HeartBeatRunway", "Meteor", "Mixing", "MopUp", "Neon", "Rainbow",
    "RainbowMorph", "Render", "Runway", "Scan", "SpanningTeacups", "Spring", "Stack", "StackMulti", "Staggered",
    "StaticColor", "Taichi", "TailChasing", "Tornado", "Tide", "Tunnel", "Twinkle", "Voice", "Warning", "Wave",
    "Breathing_Inner", "BreathingRainbow_Inner", "ColorCycle_Inner", "ColorfulCity_Inner", "Disco_Inner",
    "Door_Inner", "DoubleArc_Inner", "DoubleMeteor_Inner", "HeartBeat_Inner", "HeartBeatRunway_Inner",
    "Lottery_Inner", "Meteor_Inner", "MeteorContest_Inner", "MeteorMix_Inner", "MeteorRainbow_Inner",
    "Mixing_Inner", "MopUp_Inner", "PacMan_Inner", "Rainbow_Inner", "RainbowMorph_Inner", "Render_Inner",
    "ReturnArc_Inner", "Runway_Inner", "Scan_Inner", "Spring_Inner", "Stack_Inner", "StaticColor_Inner",
    "Taichi_Inner", "TailChasing_Inner", "Tide_Inner", "Twinkle_Inner", "Voice_Inner", "Warning_Inner",
    "Wave_Inner", "Breathing_Outer", "BreathingColorful_Outer", "BreathingRainbow_Outer", "ColorCycle_Outer",
    "ColorfulCity_Outer", "ColorfulMeteor_Outer", "Disco_Outer", "Door_Outer", "HeartBeat_Outer",
    "Lottery_Outer", "Meteor_Outer", "MeteorRainbow_Outer", "Mixing_Outer", "MopUp_Outer", "Rainbow_Outer",
    "RainbowMorph_Outer", "Reflect_Outer", "Render_Outer", "Runway_Outer", "Scan_Outer", "Spring_Outer",
    "Stack_Outer", "StaticColor_Outer", "StaticColorful_Outer", "TailChasing_Outer", "Tide_Outer",
    "Twinkle_Outer", "Voice_Outer", "Warning_Outer", "Wave_Outer", "Contest_Merge", "Door_Merge",
    "ElectricCurrent_Merge", "HeartBeatRunway_Merge", "Meteor_Merge", "Mixing_Merge", "MopUp_Merge",
    "Runway_Merge", "Scan_Merge", "Spring_Merge", "Stack_Merge", "StackMulti_Merge", "TailChasing_Merge",
    "Tide_Merge", "Wave_Merge",
];

#[derive(Deserialize)]
struct Entry {
    #[serde(rename = "DeviceID")]
    device_id: String,
    #[serde(rename = "Type")]
    kind: String,
    #[serde(rename = "Data")]
    data: Value,
}

fn device_dirs() -> Vec<PathBuf> {
    let base = std::env::var_os("ProgramData").map(PathBuf::from).unwrap_or_else(|| "C:\\ProgramData".into());
    let dir = base.join("Lian-Li").join("L-Connect 3").join("device");
    std::fs::read_dir(dir).map(|rd| rd.flatten().map(|e| e.path()).filter(|p| p.is_dir()).collect()).unwrap_or_default()
}

fn read_entry(path: &std::path::Path) -> Option<Entry> {
    let file = std::fs::File::open(path).ok()?;
    let mut s = String::new();
    flate2::read::GzDecoder::new(file).read_to_string(&mut s).ok()?;
    serde_json::from_str(&s).ok()
}

fn speed_value(b: u64) -> u8 {
    match b {
        2 => 0,
        1 => 25,
        255 => 75,
        254 => 100,
        _ => 50,
    }
}

fn brightness_value(b: u64) -> u8 {
    match b {
        0 => 100,
        1 => 75,
        3 => 25,
        4 | 8 => 0,
        _ => 50,
    }
}

fn effect(v: &Value) -> Option<(&'static effects::ModeDef, EffectSetting)> {
    let name = *LIGHTING_MODE_ORDINALS.get(v["Mode"].as_u64()? as usize)?;
    let def = effects::mode(name)?;
    let mut s = EffectSetting::new(def);
    let colors: Vec<[u8; 3]> = v["Colors"]
        .as_array()
        .map(|a| a.iter().map(|c| [c["R"].as_u64().unwrap_or(0) as u8, c["G"].as_u64().unwrap_or(0) as u8, c["B"].as_u64().unwrap_or(0) as u8]).collect())
        .unwrap_or_default();
    for (dst, src) in s.colors.iter_mut().zip(colors) {
        *dst = src;
    }
    s.speed = speed_value(v["Speed"].as_u64().unwrap_or(0));
    s.brightness = brightness_value(v["Brightness"].as_u64().unwrap_or(2));
    s.reverse = v["Direction"].as_u64() == Some(1);
    Some((def, s))
}

/// Returns the configuration imported from L-Connect for the SL Infinity controller, if any.
pub fn import() -> Option<AppConfig> {
    let mut cfg = AppConfig::default();
    let mut found = false;
    let mut ports: [Option<(&'static effects::ModeDef, EffectSetting)>; 8] = Default::default();

    for dir in device_dirs() {
        let Ok(rd) = std::fs::read_dir(&dir) else { continue };
        for f in rd.flatten() {
            let Some(e) = read_entry(&f.path()) else { continue };
            if !e.device_id.to_ascii_lowercase().contains("vid_0cf2&pid_a102") {
                continue;
            }
            found = true;
            let d = &e.data;
            if let Some(g) = e.kind.strip_prefix("FanGroupSpeed").and_then(|n| n.parse::<usize>().ok()).filter(|g| *g < 4) {
                let fan = &mut cfg.fans[g];
                let size = if d["MaxSpeed"].as_i64() == Some(2100) { FanSize::Mm120 } else { FanSize::Mm140 };
                fan.set_size(size);
                fan.source = if d["Reference"].as_i64() == Some(1) { Source::Gpu } else { Source::Cpu };
                let phases: Vec<(i32, i32)> = d["Phases"]
                    .as_array()
                    .map(|a| a.iter().map(|p| (p["Temperature"].as_i64().unwrap_or(0) as i32, p["Speed"].as_i64().unwrap_or(0) as i32)).collect())
                    .unwrap_or_default();
                if phases.last().is_some_and(|p| p.0 == 0) {
                    // Fixed speed: [(0, rpm)] or [(0,0),(0,rpm)]
                    fan.preset = Preset::Fixed;
                    fan.start_stop = phases.len() == 2;
                    fan.fixed_rpm = phases.last().unwrap().1;
                } else if phases.len() >= 3 {
                    fan.start_stop = phases[0].1 == 0;
                    fan.points = phases[1..phases.len() - 1].to_vec();
                    fan.preset = [Preset::Quiet, Preset::Standard, Preset::HighSpeed, Preset::FullSpeed]
                        .into_iter()
                        .find(|p| p.points(size) == fan.points)
                        .unwrap_or(Preset::Custom);
                }
            } else if let Some(p) = e.kind.strip_prefix("LightingPort").and_then(|n| n.parse::<usize>().ok()).filter(|p| *p < 8) {
                ports[p] = effect(d);
            } else if e.kind == "FanQuantity"
                && let Some(q) = d.as_array()
            {
                for (g, v) in q.iter().take(4).enumerate() {
                    cfg.quantity[g] = v.as_u64().unwrap_or(3).min(4) as u8;
                }
            }
        }
    }
    if !found {
        return None;
    }

    if let Some((def, s)) = &ports[0]
        && def.scope == Scope::Merge
    {
        cfg.lighting.merged = Some(s.clone());
    }
    for g in 0..4 {
        let gl = &mut cfg.lighting.groups[g];
        match (&ports[g * 2], &ports[g * 2 + 1]) {
            (Some((di, si)), _) if di.scope == Scope::Whole => {
                gl.individual = false;
                gl.whole = si.clone();
            }
            (inner, outer) => {
                gl.individual = true;
                if let Some((d, s)) = inner
                    && d.scope == Scope::Inner
                {
                    gl.inner = s.clone();
                }
                if let Some((d, s)) = outer
                    && d.scope == Scope::Outer
                {
                    gl.outer = s.clone();
                }
            }
        }
    }
    log::info!("Settings imported from L-Connect 3");
    Some(cfg)
}
