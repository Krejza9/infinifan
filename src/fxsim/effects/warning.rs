//! Warning: slow flashing through the palette colours (whole fan: inner and outer alternate).

use crate::fxsim::blocks::{Canvas, RING_T0, Ring, steps};
use crate::fxsim::color::shr;
use crate::fxsim::{BLACK, Fx};

pub(crate) fn whole(fx: &Fx, cv: &mut Canvas, t: f64) {
    let m = steps(t, 175.0 + 100.0 * fx.d(), 8.0);
    let c = fx.pal(m / 2 + 1);
    if m % 2 == 0 {
        cv.all_inner(c);
    } else {
        cv.all_outer(c);
    }
}

pub(crate) fn ring(fx: &Fx, r: &mut Ring, t: f64) {
    let n = steps(t, 175.0 + 50.0 * fx.d(), RING_T0);
    let c = if n % 2 == 0 { shr(fx.pal(n / 2), fx.shift) } else { BLACK };
    r.paint(|_, _| c);
}
