//! MopUp: a short segment sweeps over a dark strip, changing palette colour at every turn.

use crate::fxsim::blocks::{Canvas, RING_T0, Ring, Strips, merge_runway, period, steps, whole_runway};
use crate::fxsim::{BLACK, Fx};

pub(crate) fn whole(fx: &Fx, cv: &mut Canvas, t: f64) {
    whole_runway(fx, cv, t, |pass| (fx.pal(pass), BLACK));
}

pub(crate) fn ring(fx: &Fx, r: &mut Ring, t: f64) {
    let (qi, nl) = (r.q as i64, r.n as i64);
    let n = steps(t, period(fx, if r.outer { 42 } else { 62 }, 7), RING_T0);
    let len = nl * qi;
    let (c, m) = (n / (2 * len + 1), n % (2 * len + 1));
    let fwd = m <= len;
    let k = if fwd { m } else { m - len - 1 };
    let col = fx.pal_s(2 * c + i64::from(!fwd));
    r.paint(|f, x| {
        let x = nl * f as i64 + x as i64;
        let lit = if fwd {
            k < len && x >= k && x <= k + 2
        } else {
            let xm = len - 1 - x;
            k < len - 1 && xm <= k && xm >= k - 2
        };
        if lit { col } else { BLACK }
    });
}

pub(crate) fn merge(fx: &Fx, st: &mut Strips, tau: f64) {
    merge_runway(fx, st, tau, |cycle, back| (fx.pal(2 * cycle + i64::from(back)), BLACK), false);
}
