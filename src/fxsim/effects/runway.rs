//! Runway: a 5-LED block in colour 1 bouncing back and forth over a colour-2 background.

use crate::fxsim::Fx;
use crate::fxsim::blocks::{Canvas, RING_T0, Ring, Strips, merge_runway, period, pingpong, steps, whole_runway};
use crate::fxsim::color::shr;

pub(crate) fn whole(fx: &Fx, cv: &mut Canvas, t: f64) {
    whole_runway(fx, cv, t, |_| (fx.pal(0), fx.pal(1)));
}

pub(crate) fn ring(fx: &Fx, r: &mut Ring, t: f64) {
    let (qi, nl) = (r.q as i64, r.n as i64);
    let n = steps(t, period(fx, 52, 7), RING_T0);
    let lv = 12 * qi;
    let m = n % (2 * lv + 14);
    let (fg, bg) = (shr(fx.pal(0), fx.shift), fx.pal_s(1));
    r.paint(|f, x| {
        let pos = nl * f as i64 + x as i64;
        if pingpong(m, nl * qi, 5, lv + 7, lv, pos).is_some() { fg } else { bg }
    });
}

/// Runway background takes the minus-one brightness path.
pub(crate) fn merge(fx: &Fx, st: &mut Strips, tau: f64) {
    merge_runway(fx, st, tau, |_, _| (fx.pal(0), fx.pal(1)), true);
}
