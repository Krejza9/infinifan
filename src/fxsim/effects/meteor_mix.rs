//! MeteorMix: two dots in colours 1 and 2 circle each inner ring in opposite directions,
//! mixing where they meet.

use crate::fxsim::Fx;
use crate::fxsim::blocks::{RING_T0_B, Ring, period, steps, two_dots};

pub(crate) fn inner(fx: &Fx, r: &mut Ring, t: f64) {
    let n = steps(t, period(fx, 106, 15), RING_T0_B);
    let moves = n - n / 5; // every 5th step the merged dots hold
    let (a, b) = ((6 + moves).rem_euclid(8), (6 - moves).rem_euclid(8));
    let (ca, cb) = (fx.pal_s(0), fx.pal_s(1));
    r.paint(|_, x| two_dots(x as i64, a, ca, b, cb));
}
