//! Fan curves, computed the same way as in L-Connect 3.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum FanSize {
    Mm120,
    Mm140,
}

impl FanSize {
    pub fn max_rpm(self) -> i32 {
        match self {
            FanSize::Mm120 => 2100,
            FanSize::Mm140 => 1600,
        }
    }

    pub fn min_rpm(self) -> i32 {
        match self {
            FanSize::Mm120 => 210,
            FanSize::Mm140 => 200,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            FanSize::Mm120 => "120 mm",
            FanSize::Mm140 => "140 mm",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Source {
    Cpu,
    Gpu,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Preset {
    Quiet,
    Standard,
    HighSpeed,
    FullSpeed,
    Custom,
    /// Fixed speed
    Fixed,
}

impl Preset {
    /// Points (°C, % duty), same as L-Connect 3; Custom starts as Quiet.
    fn duty_points(self) -> [(i32, i32); 5] {
        match self {
            Preset::Quiet | Preset::Custom | Preset::Fixed => [(25, 20), (45, 45), (65, 52), (80, 75), (90, 100)],
            Preset::Standard => [(25, 20), (40, 50), (55, 62), (70, 75), (90, 100)],
            Preset::HighSpeed => [(25, 45), (35, 55), (50, 70), (70, 85), (85, 100)],
            Preset::FullSpeed => [(25, 100), (40, 100), (55, 100), (70, 100), (90, 100)],
        }
    }

    /// RPM points for the given size (round(duty/100 × max), banker's rounding like .NET).
    pub fn points(self, size: FanSize) -> Vec<(i32, i32)> {
        let max = size.max_rpm() as f64;
        self.duty_points()
            .iter()
            .map(|&(t, d)| (t, (d as f64 / 100.0 * max).round_ties_even() as i32))
            .collect()
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FanGroupConfig {
    pub size: FanSize,
    pub source: Source,
    pub preset: Preset,
    /// 5 user points (°C, RPM), sorted by temperature.
    pub points: Vec<(i32, i32)>,
    /// Stop the fans below the first temperature of the curve.
    pub start_stop: bool,
    /// Speed for the Fixed preset.
    pub fixed_rpm: i32,
}

impl FanGroupConfig {
    pub fn new(size: FanSize) -> Self {
        Self {
            size,
            source: Source::Cpu,
            preset: Preset::Standard,
            points: Preset::Standard.points(size),
            start_stop: false,
            fixed_rpm: size.max_rpm() / 2,
        }
    }

    /// Repairs points from a hand-edited or imported config: sorted by temperature, at least 1 °C apart
    /// within 0–100 °C, and speed never falling as the temperature rises.
    pub fn normalize(&mut self) {
        if self.points.is_empty() {
            self.points = Preset::Standard.points(self.size);
        }
        let n = self.points.len() as i32;
        let max = self.size.max_rpm();
        self.points.sort_by_key(|p| p.0);
        for i in 0..self.points.len() {
            let lo = if i == 0 { 0 } else { self.points[i - 1].0 + 1 };
            // Leave room for the remaining points below 100 °C.
            let hi = 100 - (n - 1 - i as i32);
            self.points[i].0 = self.points[i].0.clamp(lo, hi.max(lo));
            let min_rpm = if i == 0 { 0 } else { self.points[i - 1].1 };
            self.points[i].1 = self.points[i].1.clamp(min_rpm, max.max(min_rpm));
        }
        self.fixed_rpm = self.fixed_rpm.clamp(self.size.min_rpm(), max);
    }

    pub fn set_preset(&mut self, p: Preset) {
        if p != Preset::Custom && p != Preset::Fixed {
            self.points = p.points(self.size);
        }
        self.preset = p;
    }

    pub fn set_size(&mut self, size: FanSize) {
        let scale = size.max_rpm() as f64 / self.size.max_rpm() as f64;
        for p in &mut self.points {
            p.1 = (p.1 as f64 * scale).round() as i32;
        }
        self.fixed_rpm = (self.fixed_rpm as f64 * scale).round() as i32;
        self.size = size;
        if self.preset != Preset::Custom && self.preset != Preset::Fixed {
            self.points = self.preset.points(size);
        }
    }

    /// Phases as L-Connect 3 sends them to its service.
    pub fn phases(&self) -> Vec<(i32, i32)> {
        if self.preset == Preset::Fixed {
            let mut v = Vec::new();
            if self.start_stop {
                v.push((0, 0));
            }
            v.push((0, self.fixed_rpm));
            return v;
        }
        let mut v = Vec::with_capacity(self.points.len() + 2);
        if self.start_stop {
            v.push((self.points[0].0, 0));
        } else {
            v.push((0, self.size.min_rpm()));
        }
        v.extend(self.points.iter().copied());
        v.push((100, self.size.max_rpm()));
        v
    }

    /// Target RPM for a temperature.
    pub fn target_rpm(&self, temp: f32) -> i32 {
        calculate_speed(&self.phases(), temp as i32)
    }

    /// Duty for the controller: 1 = stop, otherwise max(10, round(rpm/max × 100)).
    pub fn duty(&self, temp: f32) -> u8 {
        let rpm = self.target_rpm(temp);
        if rpm == 0 {
            return 1;
        }
        let d = (rpm as f64 / self.size.max_rpm() as f64 * 100.0).round_ties_even() as i32;
        d.clamp(10, 100) as u8
    }
}

/// Piecewise linear interpolation truncated to an integer, same as L-Connect 3.
pub fn calculate_speed(phases: &[(i32, i32)], temp: i32) -> i32 {
    let Some(&first) = phases.first() else { return 0 };
    if phases.len() == 1 {
        return first.1.max(0);
    }
    let mut prev = first;
    for &cur in &phases[1..] {
        if temp <= prev.0 {
            return prev.1.max(0);
        }
        if prev.0 < temp && temp < cur.0 {
            let dt = cur.0 - prev.0;
            if dt == 0 {
                return prev.1.max(0);
            }
            let v = (cur.1 - prev.1) as f64 / dt as f64 * (temp - prev.0) as f64 + prev.1 as f64;
            return (v as i32).max(0);
        }
        prev = cur;
    }
    prev.1.max(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn presets_140mm_match_spec() {
        assert_eq!(Preset::Standard.points(FanSize::Mm140), vec![(25, 320), (40, 800), (55, 992), (70, 1200), (90, 1600)]);
        assert_eq!(Preset::HighSpeed.points(FanSize::Mm140), vec![(25, 720), (35, 880), (50, 1120), (70, 1360), (85, 1600)]);
        assert_eq!(Preset::Quiet.points(FanSize::Mm140), vec![(25, 320), (45, 720), (65, 832), (80, 1200), (90, 1600)]);
    }

    #[test]
    fn worked_example_standard_30c() {
        let c = FanGroupConfig::new(FanSize::Mm140);
        assert_eq!(c.target_rpm(30.9), 480);
        assert_eq!(c.duty(30.0), 30);
        // (0,200)→(25,320) at 10 °C = 248 RPM = 15.5 % → banker's rounding gives 16
        assert_eq!(c.duty(10.0), 16);
    }

    #[test]
    fn normalize_repairs_order_and_falling_speed() {
        let mut c = FanGroupConfig::new(FanSize::Mm140);
        c.points = vec![(50, 600), (45, 900), (45, 500), (120, 1500), (90, 1400)];
        c.normalize();
        assert_eq!(c.points, vec![(45, 900), (46, 900), (50, 900), (90, 1400), (100, 1500)]);
        c.points.clear();
        c.normalize();
        assert_eq!(c.points.len(), 5);
    }

    #[test]
    fn start_stop_stops_below_first_point() {
        let mut c = FanGroupConfig::new(FanSize::Mm140);
        c.start_stop = true;
        assert_eq!(c.duty(20.0), 1);
        assert_eq!(c.duty(25.0), 1);
        assert!(c.duty(26.0) >= 10);
    }

    #[test]
    fn full_and_fixed() {
        let mut c = FanGroupConfig::new(FanSize::Mm140);
        c.set_preset(Preset::FullSpeed);
        assert_eq!(c.duty(50.0), 100);
        c.set_preset(Preset::Fixed);
        c.fixed_rpm = 800;
        assert_eq!(c.duty(80.0), 50);
    }
}
