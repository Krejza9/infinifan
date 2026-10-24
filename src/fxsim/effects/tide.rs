//! Tide: the next palette colour floods in from both ends of the strip until it covers it.

use crate::fxsim::blocks::{Canvas, RING_T0, Ring, Strips, end, period, steps, tide_mask, x254};
use crate::fxsim::{Fx, LEVEL};

pub(crate) fn whole(fx: &Fx, cv: &mut Canvas, t: f64) {
    let qi = cv.q as i64;
    let (no, ni5) = (12 * qi, 5 * qi);
    let tt = 62.0 + 10.0 * fx.d();
    let (po, pi) = (no / 2 + 1, ni5 / 2 + 1);
    let u = ((t - 5.0) / tt).max(0.0); // outer steps since start
    for (len, p, outer) in [(no, po, true), (ni5, pi, false)] {
        let n = (u * p as f64 / po as f64).floor() as i64 - 1;
        let (phase, k) = (n.div_euclid(p), n.rem_euclid(p) + 1);
        let (new, old) = (x254(fx.pal(phase + 1)), x254(fx.pal(phase)));
        for x in 0..len {
            let c = if x < k || x > len - 1 - k { new } else { old };
            if outer { cv.outer_j(x as usize, c) } else { cv.inner_p(x as usize, c) }
        }
    }
}

pub(crate) fn ring(fx: &Fx, r: &mut Ring, t: f64) {
    let qi = r.q as i64;
    let p = period(fx, 62, 10);
    let lv = 12 * qi;
    let c_len = lv / 2 + 1;
    let n = steps(t, p, RING_T0);
    let (c, k) = (n / c_len, n % c_len);
    let (old, new) = (fx.pal_s(c), fx.pal_s(c + 1));
    if r.outer {
        r.paint(|f, x| if tide_mask(k, lv, (12 * f + x) as i64) { new } else { old });
    } else {
        // inner levels fill one level pair every 2.25 outer ticks
        let u = (t - RING_T0).max(0.0).rem_euclid(c_len as f64 * p);
        let h = (2.0 * u / p).floor() as i64;
        let ki = (2 * h - 1).div_euclid(9).max(0);
        let li = 5 * qi;
        r.paint(|f, x| if tide_mask(ki, li, (5 * f + LEVEL[x]) as i64) { new } else { old });
    }
}

pub(crate) fn merge(fx: &Fx, st: &mut Strips, tau: f64) {
    let ni = st.n as i64;
    let (lo, rows) = (12 * ni, 5 * ni);
    let u = steps(tau, (29 - 5 * fx.speed) as f64, 0.0);
    let (lh, rh) = (lo / 2, rows / 2);
    let nc = lh + 6; // flood Lh ticks, then hold 6
    let (p, v) = (u / nc, u % nc);
    let (old, new) = (fx.pal(p), fx.pal(p + 1));
    let depth = v.min(lh);
    let depth_i = rh.min(v * rh / lh);
    for x in 0..lo {
        st.outer(x as usize, if end(x, lo) < depth { new } else { old }, 1, true);
    }
    for r in 0..rows {
        st.row(r as usize, if end(r, rows) < depth_i { new } else { old }, 1, true);
    }
}
