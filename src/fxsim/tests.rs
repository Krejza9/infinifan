use super::*;
use crate::effects::{EffectSetting, GroupLighting, LightingConfig, mode};

fn setting(id: &str) -> EffectSetting {
    EffectSetting::new(mode(id).unwrap())
}

fn whole_cfg(s: EffectSetting) -> LightingConfig {
    let mut cfg = LightingConfig::default();
    for g in &mut cfg.groups {
        g.individual = false;
        g.whole = s.clone();
    }
    cfg
}

#[test]
fn static_color_shows_palette_per_fan() {
    let cfg = whole_cfg(setting("StaticColor"));
    let out = render(&cfg, [4; 4], 1234.0);
    let pal = [[255, 0, 0], [0, 0, 255], [0, 255, 85], [255, 105, 0]];
    for leds in &out {
        for f in 0..4 {
            assert!(leds[8 * f..8 * f + 8].iter().all(|&c| c == pal[f]));
            assert!(leds[32 + 12 * f..32 + 12 * f + 12].iter().all(|&c| c == pal[f]));
        }
    }
    // Default config: StaticColor_Inner / StaticColor_Outer, also per fan.
    let out = render(&LightingConfig::default(), [4; 4], 0.0);
    assert_eq!(out[0][8], [0, 0, 255]);
    assert_eq!(out[0][32 + 47], [255, 105, 0]);
}

#[test]
fn brightness_halves_per_step() {
    let mut s = setting("RainbowMorph");
    let full = render(&whole_cfg(s.clone()), [3; 4], 20.0)[0][0];
    assert_eq!(full, [160, 0, 0]);
    s.brightness = 75;
    assert_eq!(render(&whole_cfg(s.clone()), [3; 4], 20.0)[0][0], [80, 0, 0]);
    s.brightness = 50;
    assert_eq!(render(&whole_cfg(s.clone()), [3; 4], 20.0)[0][0], [40, 0, 0]);
    // StaticColor is linear instead: 255 → 127 at 50 %.
    let mut st = setting("StaticColor");
    st.brightness = 50;
    assert_eq!(render(&whole_cfg(st), [3; 4], 20.0)[0][0], [127, 0, 0]);
}

#[test]
fn missing_fans_are_black() {
    let mut cfg = whole_cfg(setting("Rainbow"));
    cfg.groups[1] = GroupLighting::default();
    for t in [0.0, 500.0, 7777.0] {
        let out = render(&cfg, [2, 1, 0, 4], t);
        for (g, q) in [(0usize, 2usize), (1, 1), (2, 0)] {
            for f in q..4 {
                assert!(out[g][8 * f..8 * f + 8].iter().all(|&c| c == BLACK));
                assert!(out[g][32 + 12 * f..32 + 12 * f + 12].iter().all(|&c| c == BLACK));
            }
        }
        assert!(out[0][0] != BLACK && out[3][79] != BLACK);
    }
    // Merged effects skip empty groups too.
    let cfg = LightingConfig { merged: Some(setting("Tide_Merge")), ..Default::default() };
    let out = render(&cfg, [1, 0, 2, 0], 300.0);
    assert!(out[1].iter().chain(out[3].iter()).all(|&c| c == BLACK));
    assert!(out[0][8..32].iter().all(|&c| c == BLACK));
    assert!(out[2][0] != BLACK);
}

#[test]
fn supports_everything_but_fw_comets() {
    assert!(supports(&LightingConfig::default()));
    let mut cfg = LightingConfig::default();
    cfg.groups[2].inner = setting("FwComets_Inner");
    assert!(!supports(&cfg));
    // The unsupported ring stays black, the rest is still drawn.
    let out = render(&cfg, [4; 4], 100.0);
    assert!(out[2][..32].iter().all(|&c| c == BLACK));
    assert_eq!(out[2][32], [255, 0, 0]);
    // Every other mode is supported.
    for m in crate::effects::MODES.iter().filter(|m| !m.id.starts_with("FwComets")) {
        let s = EffectSetting::new(m);
        let mut cfg = LightingConfig::default();
        match m.scope {
            Scope::Merge => cfg.merged = Some(s),
            Scope::Whole => cfg = whole_cfg(s),
            Scope::Inner => cfg.groups[0].inner = s,
            Scope::Outer => cfg.groups[0].outer = s,
        }
        assert!(supports(&cfg), "{}", m.id);
        for t in [0.0, 3.0, 20.0, 1e3, 1e6, 3.6e7] {
            render(&cfg, [4, 3, 1, 0], t);
        }
    }
}

// ----------------------------------------------------------------- verification vs. recordings

mod recordings {
    use super::*;
    use serde::Deserialize;
    use std::collections::BTreeMap;
    use std::path::{Path, PathBuf};
    use std::time::Instant;

    #[derive(Deserialize)]
    struct Case {
        file: String,
        effect: String,
        scope: String,
        speed: u8,
        reverse: bool,
        quantity: [u8; 4],
        brightness: u8,
        colors: Vec<[u8; 3]>,
        frames: usize,
        groups: usize,
    }

    fn load(dir: &Path) -> Vec<(PathBuf, Case)> {
        let text = std::fs::read_to_string(dir.join("index.jsonl")).expect("index.jsonl");
        text.lines()
            .filter(|l| !l.trim().is_empty())
            .map(|l| {
                let c: Case = serde_json::from_str(l).expect("index line");
                (dir.join(&c.file), c)
            })
            .collect()
    }

    /// LEDs (group, index) compared for a case: the effect's own LEDs of installed fans
    /// (Whole: all 80 LEDs of the group), plus the spill LEDs of the whole-fan strip effects.
    fn compared_leds(c: &Case) -> (Vec<(usize, usize)>, Vec<(usize, usize)>) {
        let q = usize::from(c.quantity[0]);
        let inner: Vec<_> = (0..8 * q).map(|i| (0, i)).collect();
        let outer: Vec<_> = (0..12 * q).map(|i| (0, 32 + i)).collect();
        let strip = matches!(c.effect.as_str(), "FwConveyor" | "FwConveyorWipe" | "FwPastel");
        // whole-fan strip positions s = 20f + (inner i | 8 + outer wire)
        let last = match (c.scope.as_str(), c.effect.as_str()) {
            ("Inner", "FwPastel") => 32,
            ("Inner", _) => 31,
            (_, "FwPastel") => 48,
            _ => 47,
        };
        let spill_of = |want_outer: bool| -> Vec<(usize, usize)> {
            (0..=last)
                .filter_map(|s| {
                    let (f, r) = (s / 20, s % 20);
                    if f >= q || (r >= 8) != want_outer {
                        return None;
                    }
                    Some((0, if r >= 8 { 32 + 12 * f + r - 8 } else { 8 * f + r }))
                })
                .collect()
        };
        match c.scope.as_str() {
            "Whole" => ((0..80).map(|i| (0, i)).collect(), vec![]),
            "Inner" => (inner, if strip { spill_of(true) } else { vec![] }),
            "Outer" => (outer, if strip { spill_of(false) } else { vec![] }),
            _ => {
                let mut v = vec![];
                for (g, &qg) in c.quantity.iter().enumerate() {
                    let qg = usize::from(qg);
                    v.extend((0..8 * qg).map(|i| (g, i)));
                    v.extend((0..12 * qg).map(|i| (g, 32 + i)));
                }
                (v, vec![])
            }
        }
    }

    fn config(c: &Case) -> Option<LightingConfig> {
        let id = if c.scope == "Whole" { c.effect.clone() } else { format!("{}_{}", c.effect, c.scope) };
        mode(&id)?;
        let s = EffectSetting {
            mode: id,
            colors: c.colors.clone(),
            speed: c.speed,
            brightness: c.brightness,
            reverse: c.reverse,
        };
        let mut cfg = LightingConfig::default();
        match c.scope.as_str() {
            "Merge" => cfg.merged = Some(s),
            "Whole" => {
                cfg.groups[0].individual = false;
                cfg.groups[0].whole = s;
            }
            "Inner" => cfg.groups[0].inner = s,
            "Outer" => cfg.groups[0].outer = s,
            _ => return None,
        }
        Some(cfg)
    }

    fn within(a: Rgb8, b: &[u8]) -> bool {
        (0..3).all(|k| (i16::from(a[k]) - i16::from(b[k])).abs() <= 8)
    }

    #[test]
    #[ignore = "needs INFINIFAN_RECORDINGS = path to research/fx-recordings"]
    fn fxsim_matches_recordings() {
        let root = PathBuf::from(std::env::var("INFINIFAN_RECORDINGS").expect("INFINIFAN_RECORDINGS"));
        let mut summary: BTreeMap<(String, String), Vec<f64>> = BTreeMap::new();
        let (mut calls, mut busy) = (0u64, 0f64);
        for set in ["raw", "raw2"] {
            for (path, c) in load(&root.join(set)) {
                let Some(cfg) = config(&c) else {
                    println!("{set}/{:<52} skipped (no such mode)", c.file);
                    continue;
                };
                let rec = std::fs::read(&path).expect("recording");
                assert_eq!(rec.len(), c.frames * c.groups * 240, "{}", c.file);
                let (own, spill) = compared_leds(&c);
                let (mut ok, mut total, mut sp_ok, mut sp_total) = (0usize, 0usize, 0usize, 0usize);
                for k in 0..c.frames {
                    let t = 20.0 * (k + 1) as f64;
                    let start = Instant::now();
                    let out = render(&cfg, c.quantity, t);
                    busy += start.elapsed().as_secs_f64();
                    calls += 1;
                    let frame = &rec[k * c.groups * 240..(k + 1) * c.groups * 240];
                    for &(g, led) in &own {
                        let at = (g * 80 + led) * 3;
                        total += 1;
                        ok += usize::from(within(out[g][led], &frame[at..at + 3]));
                    }
                    for &(g, led) in &spill {
                        let at = (g * 80 + led) * 3;
                        sp_total += 1;
                        sp_ok += usize::from(within(out[g][led], &frame[at..at + 3]));
                    }
                }
                let all = (ok + sp_ok) as f64 / (total + sp_total) as f64;
                let spill_txt = if sp_total > 0 {
                    format!("  (spill {:.2} %)", 100.0 * sp_ok as f64 / sp_total as f64)
                } else {
                    String::new()
                };
                println!("{set}/{:<52} {:7.2} %{spill_txt}", c.file, 100.0 * all);
                summary.entry((c.effect.clone(), c.scope.clone())).or_default().push(all);
            }
        }

        println!("\nSummary (share of LED samples with R, G, B within ±8; mean / min per effect):");
        println!("{:<18} {:<6} {:>8} {:>8} {:>6}", "effect", "scope", "mean %", "min %", "cases");
        let mut worst = 100.0f64;
        for ((e, s), v) in &summary {
            let mean = 100.0 * v.iter().sum::<f64>() / v.len() as f64;
            let min = 100.0 * v.iter().cloned().fold(1.0, f64::min);
            worst = worst.min(mean);
            println!("{e:<18} {s:<6} {mean:>8.2} {min:>8.2} {:>6}", v.len());
        }
        println!("\nrender(): {calls} calls, {:.2} µs per call on average", 1e6 * busy / calls as f64);

        // A typical preview: 4 groups × 4 fans with individual ring effects, and a merged effect.
        let mut cfg = LightingConfig::default();
        for (g, gl) in cfg.groups.iter_mut().enumerate() {
            gl.inner = setting(["Rainbow_Inner", "FwPastel_Inner", "Voice_Inner", "Mixing_Inner"][g]);
            gl.outer = setting(["Meteor_Outer", "Stack_Outer", "FwConveyor_Outer", "Lottery_Outer"][g]);
        }
        let merged = LightingConfig { merged: Some(setting("Stack_Merge")), ..Default::default() };
        for (name, cfg) in [("4 groups, ring effects", &cfg), ("Stack_Merge, 16 fans", &merged)] {
            let n = 20_000;
            let start = Instant::now();
            let mut sink = 0u32;
            for k in 0..n {
                sink += u32::from(render(cfg, [4; 4], 33.3 * k as f64)[3][79][0]);
            }
            let us = 1e6 * start.elapsed().as_secs_f64() / n as f64;
            println!("render() {name}: {us:.2} µs per call (checksum {sink})");
        }

        assert!(worst >= 99.0, "an effect matches below 99 % ({worst:.2} %)");
    }
}
