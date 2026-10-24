//! MeteorContest: two dots in colours 1 and 2 chase each other around each inner ring, half a
//! ring apart.

use crate::fxsim::Fx;
use crate::fxsim::blocks::{RING_T0_B, Ring, period, steps, two_dots};

pub(crate) fn inner(fx: &Fx, r: &mut Ring, t: f64) {
    let n = steps(t, period(fx, 89, 15), RING_T0_B);
    let pos = if fx.reverse { -n } else { n };
    let (a, b) = (pos.rem_euclid(8), (pos + 4).rem_euclid(8));
    let (ca, cb) = (fx.pal_s(0), fx.pal_s(1));
    r.paint(|_, x| two_dots(x as i64, a, ca, b, cb));
}
