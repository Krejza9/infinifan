//! BreathingRainbow: breathing on all fans, stepping to the next rainbow colour while dark.

use crate::fxsim::Fx;
use crate::fxsim::blocks::{Canvas, Ring, level, ring_breath, whole_breath};
use crate::fxsim::color::{rainbow12, scale};

pub(crate) fn whole(fx: &Fx, cv: &mut Canvas, t: f64) {
    let (n, l) = whole_breath(fx, t);
    let c = level(rainbow12((n + 128) / 192), l); // colour steps while dark
    for f in 0..4 {
        cv.fan(f, c);
    }
}

pub(crate) fn ring(fx: &Fx, r: &mut Ring, t: f64) {
    let (n, w) = ring_breath(fx, t);
    let c = scale(rainbow12((n + 124) / 188), w);
    r.paint(|_, _| c);
}
