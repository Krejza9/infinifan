//! Rainbow: a colour wheel spread over the LEDs of each ring and frame, rotating step by step.

use crate::fxsim::Fx;
use crate::fxsim::blocks::{Canvas, RING_T0, Ring, period, steps};
use crate::fxsim::color::{shr, wheel, wheel128};

pub(crate) fn whole(fx: &Fx, cv: &mut Canvas, t: f64) {
    let rev = fx.reverse;
    let n = steps(t, 21.0 + 5.0 * fx.d(), 7.0);
    let hh = (7.5 * n as f64).rem_euclid(960.0);
    let hue = |x: usize| {
        let off = 59.75 * x as f64;
        let h = if rev {
            (hh + off).rem_euclid(960.0)
        } else if hh - off < 0.0 {
            hh - off + 954.0 // the forward wrap lands 6 units short
        } else {
            hh - off
        };
        wheel(h / 960.0, 160.0)
    };
    for f in 0..4 {
        for i in 0..8 {
            cv.inner(f, i, hue(i));
        }
        for j in 0..12 {
            cv.outer(f, j, hue(j));
        }
    }
}

pub(crate) fn ring(fx: &Fx, r: &mut Ring, t: f64) {
    let s = fx.shift;
    let n = steps(t, period(fx, 41, 5), RING_T0);
    let sign = if fx.reverse { 1 } else { -1 };
    r.paint(|_, x| shr(wheel128(n + sign * 8 * x as i64), s));
}
