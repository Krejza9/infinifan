//! DoubleArc: two mirrored arcs grow level by level down each inner ring and shrink back, in the
//! next palette colour every cycle.

use crate::fxsim::blocks::{RING_T0, Ring, period, steps};
use crate::fxsim::{BLACK, Fx, LEVEL};

pub(crate) fn inner(fx: &Fx, r: &mut Ring, t: f64) {
    let rev = fx.reverse;
    let n = steps(t, period(fx, 63, 7), RING_T0);
    let (c, m) = (n / 11, n % 11);
    let k = if m <= 5 { m } else { 10 - m };
    let col = fx.pal_s(c);
    r.paint(|_, x| {
        let lv = if rev { 4 - LEVEL[x] as i64 } else { LEVEL[x] as i64 };
        if lv < k { col } else { BLACK }
    });
}
