//! DoubleMeteor: two dots run apart from the same point on each inner ring, in the next palette
//! colour every pass.

use crate::fxsim::Fx;
use crate::fxsim::blocks::{RING_T0_B, Ring, period, steps, two_dots};

pub(crate) fn inner(fx: &Fx, r: &mut Ring, t: f64) {
    let n = steps(t, period(fx, 101, 10), RING_T0_B);
    let (m, k) = (n % 5, n / 5);
    let centre = if k % 2 == 0 { 6 } else { 2 };
    let c = fx.pal_s(k);
    let (a, b) = ((centre + m).rem_euclid(8), (centre - m).rem_euclid(8));
    r.paint(|_, x| two_dots(x as i64, a, c, b, c));
}
