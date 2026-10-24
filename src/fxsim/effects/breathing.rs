//! Breathing: every fan fades its palette colour out and back in.

use crate::fxsim::Fx;
use crate::fxsim::blocks::{Canvas, Ring, level, ring_breath, whole_breath};
use crate::fxsim::color::scale;

pub(crate) fn whole(fx: &Fx, cv: &mut Canvas, t: f64) {
    let (_, l) = whole_breath(fx, t);
    for f in 0..4 {
        cv.fan(f, level(fx.pal(f as i64), l));
    }
}

pub(crate) fn ring(fx: &Fx, r: &mut Ring, t: f64) {
    let (_, w) = ring_breath(fx, t);
    r.paint(|f, _| scale(fx.pal(f as i64), w));
}
