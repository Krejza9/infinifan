//! FwStrobeRainbow: FwStrobe with the next rainbow colour every lap.

use crate::fxsim::blocks::{Canvas, Ring, ring_strobe, whole_strobe};
use crate::fxsim::color::rainbow12;
use crate::fxsim::{Fx, Rgb8};

/// Rainbow colour of the lap of tick `n` on a strip of `len` LEDs (lap `3N + 8` ticks).
fn lap_colour(n: i64, len: i64) -> Rgb8 {
    rainbow12((n - 2) / (3 * len + 8))
}

pub(crate) fn whole(fx: &Fx, cv: &mut Canvas, t: f64) {
    whole_strobe(fx, cv, t, lap_colour);
}

pub(crate) fn ring(fx: &Fx, r: &mut Ring, t: f64) {
    ring_strobe(fx, r, t, lap_colour);
}
