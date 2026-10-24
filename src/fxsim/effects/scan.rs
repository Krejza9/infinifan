//! Scan: a short segment with a fading tail sweeps back and forth over a dark strip.

use crate::fxsim::blocks::{
    Canvas, RING_T0, Ring, Strips, bounce, bounce_segment, level, period, pingpong, seg_lit, steps,
};
use crate::fxsim::color::shr;
use crate::fxsim::{BLACK, Fx, LEVEL};

pub(crate) fn whole(fx: &Fx, cv: &mut Canvas, t: f64) {
    let qi = cv.q as i64;
    let (no, ni5) = (12 * qi, 5 * qi);
    let (fwd, h, _) = bounce(t, 29.0 + 7.0 * fx.d(), 8.0, no, 3, 3);
    let c1 = fx.pal(1);
    for x in 0..no {
        let pos = if fwd { x } else { no - 1 - x };
        let c = match h - pos {
            0 => c1,
            1 => level(c1, 127),
            2 => level(c1, 63),
            _ => BLACK,
        };
        cv.outer_j(x as usize, c);
    }
    let hi = h * (ni5 + 4) / (no + 4);
    let c0 = fx.pal(0);
    for x in 0..ni5 {
        cv.inner_p(x as usize, if seg_lit(fwd, hi, ni5, 5, x) { c0 } else { BLACK });
    }
}

pub(crate) fn ring(fx: &Fx, r: &mut Ring, t: f64) {
    let (s, qi) = (fx.shift, r.q as i64);
    let n = steps(t, period(fx, 57, 7), RING_T0);
    let lv = 12 * qi;
    let half = lv + 5;
    let m = n % (2 * half);
    let col = fx.pal(0);
    if r.outer {
        r.paint(|f, x| match pingpong(m, lv, 3, half, lv, (12 * f + x) as i64) {
            Some(d) => shr(col, d as u32 + s),
            None => BLACK,
        });
    } else {
        let c = shr(col, s);
        r.paint(|f, x| match pingpong(m, 5 * qi, 5, half, lv, (5 * f + LEVEL[x]) as i64) {
            Some(_) => c,
            None => BLACK,
        });
    }
}

pub(crate) fn merge(fx: &Fx, st: &mut Strips, tau: f64) {
    let ni = st.n as i64;
    let (lo, rows) = (12 * ni, 5 * ni);
    let u = steps(tau, (56 - 10 * fx.speed) as f64, 0.0);
    let (_, a, b) = bounce_segment(u, lo, lo, 5, 1);
    let c1 = fx.pal(1);
    for x in 0..lo {
        st.outer(x as usize, if (a..=b).contains(&x) { c1 } else { BLACK }, 1, false);
    }
    let (_, a, b) = bounce_segment(u, rows, lo, 5, 1);
    let c0 = fx.pal(0);
    for r in a.max(0)..=b.min(rows - 1) {
        st.row(r as usize, c0, 1, false);
    }
}
