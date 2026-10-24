//! StaticColor: every fan lit in its palette colour; brightness is linear (`floor(c · b / 100)`).

use crate::fxsim::Fx;
use crate::fxsim::blocks::{Canvas, Ring};
use crate::fxsim::color::mul_div;

/// Full colours; the frame applies the linear brightness afterwards.
pub(crate) fn whole(fx: &Fx, cv: &mut Canvas, _t: f64) {
    for f in 0..4 {
        cv.fan(f, fx.pal(f as i64));
    }
}

pub(crate) fn ring(fx: &Fx, r: &mut Ring, _t: f64) {
    r.paint(|f, _| mul_div(fx.pal(f as i64), fx.brightness, 100));
}
