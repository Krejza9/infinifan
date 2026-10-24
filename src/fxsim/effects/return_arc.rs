//! ReturnArc: an arc grows around each inner ring and shrinks back, in the next palette colour
//! every cycle.

use crate::fxsim::blocks::{RING_T0, Ring, period, steps};
use crate::fxsim::{BLACK, Fx};

pub(crate) fn inner(fx: &Fx, r: &mut Ring, t: f64) {
    let rev = fx.reverse;
    let n = steps(t, period(fx, 73, 12), RING_T0);
    let (c, m) = (n / 17, n % 17);
    let k = if m <= 8 { m } else { 16 - m };
    let col = fx.pal_s(c);
    r.paint(|_, x| {
        let i = if rev { 7 - x as i64 } else { x as i64 };
        if i < k { col } else { BLACK }
    });
}
