//! FwConverge: a colour fills the strip from both ends towards the centre and drains back, over a
//! dim rainbow background; the colours move on every round.

use crate::fxsim::blocks::{Canvas, RING_T0_B, Ring, period, ring_rainbow_bg, steps};
use crate::fxsim::color::{rainbow12, scale, sixth};
use crate::fxsim::{Fx, Rgb8};

/// Whole-fan fill colours.
const CONV_FG: [Rgb8; 6] =
    [[177, 66, 0], [66, 177, 0], [0, 177, 66], [0, 66, 177], [66, 0, 177], [177, 0, 66]];

pub(crate) fn whole(fx: &Fx, cv: &mut Canvas, t: f64) {
    let d = fx.d();
    // independent clocks: outer (48, t0 6, 28 + 14D), inner (32, t0 9, 67.9 + 14.03D)
    let ti = 67.9 + (124.0 - 67.9) * d / 4.0;
    for (m, outer, t0, tt) in [(48i64, true, 6.0, 28.0 + 14.0 * d), (32, false, 9.0, ti)] {
        let h = m / 2;
        let p = 2 * h + 1;
        let n = steps(t, tt, t0) - 1;
        let (c, mm) = (n.div_euclid(p), n.rem_euclid(p));
        let lev = if mm < h - 1 {
            mm + 1
        } else if mm == h - 1 {
            h
        } else if mm < 2 * h - 1 {
            2 * h - 1 - mm
        } else {
            0
        };
        let fg = CONV_FG[(if mm < h - 1 { c } else { c + 1 }).rem_euclid(6) as usize];
        let bg = sixth(if mm == 2 * h { c + 1 } else { c }, 63, 32);
        for x in 0..m {
            let c = if x.min(m - 1 - x) < lev { fg } else { bg };
            if outer { cv.outer_j(x as usize, c) } else { cv.inner_i(x as usize, c) }
        }
    }
}

pub(crate) fn ring(fx: &Fx, r: &mut Ring, t: f64) {
    let (s, nl) = (fx.shift, r.n as i64);
    let p = period(fx, if r.outer { 84 } else { 124 }, 14);
    let n = steps(t, p, RING_T0_B);
    let m_len = 4 * nl;
    let h = m_len / 2;
    let (rr, c) = (n / (2 * h + 1), n % (2 * h + 1));
    let bg = ring_rainbow_bg(rr, s);
    let (fill, reach) = if c < h {
        (scale(rainbow12(2 * rr + 1), 255 >> s), c) // converge
    } else {
        (scale(rainbow12(2 * rr + 3), 255 >> s), 2 * h - c) // diverge
    };
    r.paint(|f, x| {
        let j = nl * f as i64 + x as i64;
        if j.min(m_len - 1 - j) < reach { fill } else { bg }
    });
}
