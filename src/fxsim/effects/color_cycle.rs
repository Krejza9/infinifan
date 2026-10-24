//! ColorCycle: the next palette colour wipes over the previous one (inner: per fan over its
//! 8 LEDs, outer: one wipe over the four-slot frame strip).

use crate::fxsim::Fx;
use crate::fxsim::blocks::{RING_T0, Ring, period, steps};

pub(crate) fn ring(fx: &Fx, r: &mut Ring, t: f64) {
    let (rev, outer) = (fx.reverse, r.outer);
    // Wipe cycle (len, hold = 2): inner per fan over 8 LEDs, outer one four-slot wipe.
    let (len, p) = if outer { (48, period(fx, 42, 7)) } else { (8, period(fx, 62, 7)) };
    let n = steps(t, p, RING_T0);
    let (c, k) = (n / (len + 2), (n % (len + 2)).min(len));
    let (new, old) = (fx.pal_s(c), fx.pal_s(c - 1));
    r.paint(|f, x| {
        let j = if outer { (12 * f + x) as i64 } else { x as i64 };
        let filled = if rev { j >= len - k } else { j < k };
        if filled { new } else { old }
    });
}
