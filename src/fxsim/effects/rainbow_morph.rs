//! RainbowMorph: all LEDs show the same colour, slowly morphing around the colour wheel.

use crate::fxsim::Fx;
use crate::fxsim::blocks::{Canvas, RING_T0, Ring, period, steps};
use crate::fxsim::color::{shr, wheel, wheel128};

pub(crate) fn whole(fx: &Fx, cv: &mut Canvas, t: f64) {
    let n = steps(t, 19.0 + 22.0 * fx.d(), 7.0);
    let c = wheel(n as f64 / 128.0, 160.0);
    cv.all_inner(c);
    cv.all_outer(c);
}

pub(crate) fn ring(fx: &Fx, r: &mut Ring, t: f64) {
    let c = shr(wheel128(steps(t, period(fx, 107, 22), RING_T0)), fx.shift);
    r.paint(|_, _| c);
}
