//! HeartBeat: colour 1 pulses up and down like a heartbeat (whole fan: the outer frame follows
//! the inner ring with a delay).

use crate::fxsim::Fx;
use crate::fxsim::blocks::{Canvas, RING_T0, Ring, level, steps};
use crate::fxsim::color::scale;

/// Whole-fan pulse at step `x` of the pulse.
fn pulse(x: i64) -> u32 {
    (match x {
        0..60 => 15 + 4 * x,
        60..90 => 255,
        90..150 => 255 - 4 * (x - 90),
        _ => 15,
    }) as u32
}

pub(crate) fn whole(fx: &Fx, cv: &mut Canvas, t: f64) {
    let n = steps(t, 2.0 + 2.0 * fx.d(), 10.0) % 360;
    let col = |l: u32| level(fx.pal(0), l.min(254));
    cv.all_inner(col(pulse(n)));
    cv.all_outer(col(pulse(n - 104)));
}

/// Ring envelope (184 ticks, as in the verified model): 15 ×4, rise 15+4k (k = 0..59),
/// 254 ×31, fall 254−4k (k = 1..59), 15 ×30.
fn heartbeat_level(n: i64) -> u32 {
    let m = n.rem_euclid(184);
    (match m {
        0..=3 => 15,
        4..=63 => 15 + 4 * (m - 4),
        64..=94 => 254,
        95..=153 => 254 - 4 * (m - 94),
        _ => 15,
    }) as u32
}

pub(crate) fn ring(fx: &Fx, r: &mut Ring, t: f64) {
    let n = steps(t, 6.0 + 5.0 * fx.d(), RING_T0);
    let c = scale(fx.pal(0), (heartbeat_level(n) + 1) >> fx.shift);
    r.paint(|_, _| c);
}
