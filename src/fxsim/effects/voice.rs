//! Voice: a VU meter that fills the fans up and back down, over a dim background.

use crate::fxsim::blocks::{Canvas, RING_T0, Ring, steps, x254};
use crate::fxsim::color::{mul_div, scale};
use crate::fxsim::{Fx, LEVEL};

pub(crate) fn whole(fx: &Fx, cv: &mut Canvas, t: f64) {
    let rev = fx.reverse;
    let q = cv.q;
    let qi = q as i64;
    let no = 12 * qi;
    let n = steps(t, 11.0 + 5.0 * fx.d(), 7.0);
    // level sequence: for k = q … 1 a rise 0..12k and a fall 12k..2 (2·12k steps)
    let total = 12 * qi * (qi + 1);
    let mut r = if total > 0 { n % total } else { 0 };
    let (mut lo, mut li) = (0, 0);
    for k in (1..=qi).rev() {
        let (po, pi) = (12 * k, 5 * k);
        if r < 2 * po {
            let tri_at = |x: i64, p: i64| if x <= p { x } else { 2 * p + 1 - x };
            lo = tri_at(r, po);
            li = tri_at((r * pi / po).min(2 * pi - 1), pi);
            break;
        }
        r -= 2 * po;
    }
    let (ci, co) = (x254(fx.pal(0)), x254(fx.pal(1)));
    let (di, dout) = (mul_div(fx.pal(0), 14, 255), mul_div(fx.pal(1), 14, 255));
    for x in 0..no {
        let jj = if rev { no - 1 - x } else { x };
        cv.outer_j(x as usize, if jj < lo { co } else { dout });
    }
    for f in 0..q {
        let ff = if rev { qi - 1 - f as i64 } else { f as i64 };
        let k = (li - 5 * ff).clamp(0, 5) as usize;
        for i in 0..8 {
            // reverse: each ring rotated by 4 (fill starts at LED 4)
            let l = if rev { LEVEL[(i + 4) % 8] } else { LEVEL[i] };
            cv.inner(f, i, if l < k { ci } else { di });
        }
    }
}

/// VU-meter script (ring.md §4.33) at index `j`: (height in 1/12 fan, rising?).
fn voice_script(mut j: i64, q: i64) -> (i64, bool) {
    for k in (1..=q).rev() {
        let m = 12 * k;
        if j < m + 2 {
            return (j.min(m), true); // up 0..=M, top held one extra tick
        }
        if j < 2 * m {
            return (m - 1 - (j - m - 2), false); // down M−1 .. 2
        }
        j -= 2 * m;
    }
    (0, true)
}

pub(crate) fn ring(fx: &Fx, r: &mut Ring, t: f64) {
    let (s, rev, qi, outer) = (fx.shift, fx.reverse, r.q as i64, r.outer);
    let n = steps(t, 4.0 + 5.0 * fx.d(), RING_T0);
    let total = 12 * qi * (qi + 1);
    let (h, rising) = voice_script(n % total, qi);
    let base_c = fx.pal(n / total);
    let (on, off) = (scale(base_c, 255 >> s), scale(base_c, 15 >> s));
    r.paint(|f, x| {
        // Reverse fills from the last fan (and on the outer frame from wire 11).
        let fs = if rev { qi - 1 - f as i64 } else { f as i64 };
        let hf = (h - 12 * fs).clamp(0, 12);
        let lit = if outer {
            let xs = if rev { 11 - x as i64 } else { x as i64 };
            xs < hf
        } else {
            let hr = h - 12 * fs;
            let lvl = if rising { 5 * hf / 12 } else { (5 * hr + 18).div_euclid(12).clamp(0, 5) };
            let d = LEVEL[x] as i64;
            (if rev { 4 - d } else { d }) < lvl
        };
        if lit { on } else { off }
    });
}
