//! Stack: dots in colour 2 drop one by one along the strip over a colour-1 background and pile
//! up at the far end, slowly in the first half and fast in the second.

use crate::fxsim::blocks::{Canvas, RING_T0, Ring, Strips, stack_state, x254};
use crate::fxsim::{Fx, LEVEL};

pub(crate) fn whole(fx: &Fx, cv: &mut Canvas, t: f64) {
    let (d, rev, qi) = (fx.d(), fx.reverse, cv.q as i64);
    let (no, ni5) = (12 * qi, 5 * qi);
    let (sv, si) = (no * (no + 1) / 2, ni5 * (ni5 + 1) / 2);
    let (ps, pf) = (6.0 + 5.0 * d, 3.0 + 2.0 * d);
    let h = sv / 2;
    let tc = h as f64 * ps + (sv - h + 2) as f64 * pf; // last state held 3 fast steps
    let u = (t - 5.5).max(0.0).rem_euclid(tc);
    let ko = if u < h as f64 * ps {
        (u / ps).floor() as i64
    } else {
        h + ((u - h as f64 * ps) / pf).floor() as i64
    }
    .min(sv - 1);
    // inner state follows the outer one, mapped separately in the slow and the fast half
    let hi = si / 2;
    let ki = if ko < h {
        (ko as f64 * hi as f64 / h as f64 + 0.4).floor() as i64
    } else {
        hi + ((ko - h) as f64 * (si + 2 - hi) as f64 / (sv + 2 - h) as f64 + 0.4).floor() as i64
    }
    .min(si - 1);
    let (fg, bg) = (x254(fx.pal(1)), x254(fx.pal(0)));
    let lit = |state: i64, len: i64, x: i64| {
        let (pile, p) = stack_state(state, len);
        let pp = if rev { len - 1 - x } else { x };
        pp >= len - pile || pp == p
    };
    for x in 0..no {
        cv.outer_j(x as usize, if lit(ko, no, x) { fg } else { bg });
    }
    for x in 0..ni5 {
        cv.inner_p(x as usize, if lit(ki, ni5, x) { fg } else { bg });
    }
}

/// Virtual stack clock of the ring Stack (ring.md §4.15) for a virtual strip of `lv` positions.
fn stack_clock(fx: &Fx, t: f64, lv: i64) -> i64 {
    let (ps, pf) = ((26 - 5 * fx.speed) as f64, (11 - 2 * fx.speed) as f64);
    let sv = lv * (lv + 1) / 2;
    let h = sv / 2;
    let tc = h as f64 * ps + (sv - h + 2) as f64 * pf;
    let u = (t - RING_T0).max(0.0).rem_euclid(tc);
    let s = if u < h as f64 * ps {
        (u / ps).floor() as i64
    } else {
        h + ((u - h as f64 * ps) / pf).floor() as i64
    };
    s.min(sv - 1)
}

pub(crate) fn ring(fx: &Fx, r: &mut Ring, t: f64) {
    let (rev, qi) = (fx.reverse, r.q as i64);
    let lv = 12 * qi;
    let st = stack_clock(fx, t, lv);
    let (lit_c, bg) = (fx.pal_s(1), fx.pal_s(0));
    if r.outer {
        let (k, pos) = stack_state(st, lv);
        r.paint(|f, x| {
            let xx = (12 * f + x) as i64;
            let i = if rev { lv - 1 - xx } else { xx };
            if i >= lv - k || i == pos { lit_c } else { bg }
        });
    } else {
        let li = 5 * qi;
        let (sv, si) = (lv * (lv + 1) / 2, li * (li + 1) / 2);
        let (k, pos) = stack_state(st * si / sv, li);
        r.paint(|f, x| {
            let u = (5 * f + LEVEL[x]) as i64;
            let i = if rev { li - 1 - u } else { u };
            if i >= li - k || i == pos { lit_c } else { bg }
        });
    }
}

pub(crate) fn merge(fx: &Fx, st: &mut Strips, tau: f64) {
    let ni = st.n as i64;
    let (lo, rows) = (12 * ni, 5 * ni);
    let (si, so) = (rows * (rows + 1) / 2, lo * (lo + 1) / 2);
    let ts = (20 - 2 * fx.speed) as f64; // slow half
    let tf = 2.0; // fast half, independent of speed
    let th = si / 2;
    let p = th as f64 * ts + (si - th + 2) as f64 * tf;
    let tt = tau.rem_euclid(p);
    let s_in = if tt < th as f64 * ts {
        (tt / ts).floor() as i64
    } else {
        si.min(th + ((tt - th as f64 * ts) / tf).floor() as i64)
    };
    let (c0, c1) = (fx.pal(0), fx.pal(1));
    for (outer, len, state) in [(false, rows, s_in), (true, lo, s_in * so / si)] {
        let (h, pos) = stack_state(state, len);
        for x in 0..len {
            let xs = if fx.reverse { len - 1 - x } else { x };
            let lit = xs >= len - h || (xs == pos && h < len);
            let col = if lit { c1 } else { c0 };
            if outer {
                st.outer(x as usize, col, 1, true);
            } else {
                st.row(x as usize, col, 1, true);
            }
        }
    }
}
