//! FwWave: a brightness wave travels around every ring and frame in a slowly changing hue.

use crate::fxsim::Fx;
use crate::fxsim::blocks::{Canvas, RING_T0, Ring, period, steps};
use crate::fxsim::color::{hsv8, shr};

/// Half-wave envelope (5 sub-steps per LED).
const WAVE_E26: [u32; 26] = [
    127, 121, 114, 108, 102, 95, 89, 83, 78, 72, 67, 62, 57, 53, 49, 45, 41, 38, 35, 32, 30, 28, 27,
    26, 25, 25,
];

pub(crate) fn whole(fx: &Fx, cv: &mut Canvas, t: f64) {
    let n = steps(t, 27.0 + 7.0 * fx.d(), 5.5);
    let val = hsv8(n / 5 + 1);
    let wave = |o: i64| {
        let k = (o - n).rem_euclid(50);
        let l = WAVE_E26[k.min(50 - k) as usize];
        val.map(|v| (((u32::from(v) + 1) * l) >> 8) as u8)
    };
    for f in 0..4 {
        for i in 0..8 {
            cv.inner(f, i, wave(5 * i as i64));
        }
        for j in 0..12 {
            let o = match j {
                10 => 0,
                11 => 49,
                _ => 5 * j as i64,
            };
            cv.outer(f, j, wave(o));
        }
    }
}

pub(crate) fn ring(fx: &Fx, r: &mut Ring, t: f64) {
    let (s, outer) = (fx.shift, r.outer);
    let n = steps(t, period(fx, 55, 7), RING_T0);
    let val = hsv8(n / 5 + 1);
    r.paint(|_, x| {
        // LED offsets 5x; on the outer frame wire 10 repeats wire 0 and wire 11 is 49.
        let o = match (outer, x) {
            (true, 10) => 0,
            (true, 11) => 49,
            _ => 5 * x as i64,
        };
        let ph = (o - n).rem_euclid(50);
        let e = WAVE_E26[ph.min(50 - ph) as usize];
        shr(val.map(|v| ((u32::from(v) + 1) * e >> 8) as u8), s)
    });
}
