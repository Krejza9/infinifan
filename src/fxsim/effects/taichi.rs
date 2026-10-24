//! Taichi: each inner ring half colour 1, half colour 2, rotating.

use crate::fxsim::Fx;
use crate::fxsim::blocks::{RING_T0, Ring, period, steps};

pub(crate) fn inner(fx: &Fx, r: &mut Ring, t: f64) {
    let rev = fx.reverse;
    let n = steps(t, period(fx, 63, 7), RING_T0);
    let (c0, c1) = (fx.pal_s(0), fx.pal_s(1));
    r.paint(|_, x| {
        let x = x as i64;
        let pos = if rev { (x + n).rem_euclid(8) } else { (x - n).rem_euclid(8) };
        if pos < 4 { c0 } else { c1 }
    });
}
