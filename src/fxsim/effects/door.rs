//! Door: the strip fills from both ends towards the centre and opens again, in the next palette
//! colour every cycle.

use crate::fxsim::blocks::{Canvas, RING_T0, Ring, Strips, end, period, steps, tide_mask, x254};
use crate::fxsim::{BLACK, Fx, LEVEL};

/// Fill sequence `1, 2, …, c, c−1, …, 1, 0, 0` at index `x` (length 2c + 1).
fn door_seq(x: i64, c: i64) -> i64 {
    if x < c {
        x + 1
    } else if x < 2 * c - 1 {
        2 * c - 1 - x
    } else {
        0
    }
}

pub(crate) fn whole(fx: &Fx, cv: &mut Canvas, t: f64) {
    let qi = cv.q as i64;
    let (no, ni5) = (12 * qi, 5 * qi);
    let (co, ci) = (no / 2, (ni5 + 1) / 2);
    let l = 2 * co + 1;
    let n = steps(t, 25.0 + 7.0 * fx.d(), 7.0) + 1; // master step index
    // start-up quirk: during the first 8 steps the outer strip lags 2 steps behind
    let n_out = if n < 8 { n - 2 } else { n };
    let fo = if n_out < 0 { 0 } else { door_seq(n_out % l, co) };
    let col_o = x254(fx.pal(if n_out < 0 { 0 } else { n_out / l }));
    let k = n % l;
    let f = door_seq(k, co);
    let fi = if k >= l - 2 {
        0
    } else if k < co {
        f * ci / co
    } else {
        (f * ci + co - 1) / co
    };
    let col_i = x254(fx.pal(n / l));
    for x in 0..no {
        let lit = x < fo || x > no - 1 - fo;
        cv.outer_j(x as usize, if lit { col_o } else { BLACK });
    }
    for x in 0..ni5 {
        let lit = x < fi || x > ni5 - 1 - fi;
        cv.inner_p(x as usize, if lit { col_i } else { BLACK });
    }
}

pub(crate) fn ring(fx: &Fx, r: &mut Ring, t: f64) {
    let qi = r.q as i64;
    let so = 6 * qi;
    let n = steps(t, period(fx, 53, 7), RING_T0);
    let (c, m) = (n / (2 * so + 1), n % (2 * so + 1));
    let col = fx.pal_s(c);
    if r.outer {
        let k = if m <= so { m } else { 2 * so - m }.clamp(0, so);
        r.paint(|f, x| if tide_mask(k, 12 * qi, (12 * f + x) as i64) { col } else { BLACK });
    } else {
        let li = 5 * qi;
        let si = (li + 1) / 2;
        let ki = if m <= so { m * si / so } else { si - (m - so) * si / so }.clamp(0, si);
        r.paint(|f, x| if tide_mask(ki, li, (5 * f + LEVEL[x]) as i64) { col } else { BLACK });
    }
}

pub(crate) fn merge(fx: &Fx, st: &mut Strips, tau: f64) {
    let ni = st.n as i64;
    let (lo, rows) = (12 * ni, 5 * ni);
    let u = steps(tau, (51 - 10 * fx.speed) as f64, 0.0);
    let (lh, rh) = (lo / 2, rows / 2);
    let nc = 2 * lh + 2;
    let col = fx.pal(u / nc);
    let v = u % nc;
    let depth = (lh - (v - lh).abs()).max(0); // 0, 1, …, Lh, …, 1, 0, 0
    let ci = v * rh / lh; // inner counter scaled to the outer one
    let depth_i = (rh - (ci - rh).abs()).max(0);
    for x in 0..lo {
        if end(x, lo) < depth {
            st.outer(x as usize, col, 1, true);
        }
    }
    for r in 0..rows {
        if end(r, rows) < depth_i {
            st.row(r as usize, col, 1, true);
        }
    }
}
