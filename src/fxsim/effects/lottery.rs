//! Lottery: a colour-1 block (outer) or dot (inner) runs around over a colour-2 background.

use crate::fxsim::Fx;
use crate::fxsim::blocks::{RING_T0, Ring, steps};
use crate::fxsim::color::shr;

/// Triangle 0, 1, …, half, half−1, …, 1, 0, 1, … (period 2·half).
fn tri(n: i64, half: i64) -> i64 {
    let m = n.rem_euclid(2 * half);
    if m <= half { m } else { 2 * half - m }
}

pub(crate) fn ring(fx: &Fx, r: &mut Ring, t: f64) {
    let (s, rev, qi) = (fx.shift, fx.reverse, r.q as i64);
    let (c0, c1) = (shr(fx.pal(0), s), shr(fx.pal(1), s));
    if r.outer {
        let n = steps(t, 21.0 + 10.0 * fx.d(), RING_T0);
        let len = 12 * qi;
        let h = len / 2 - 1 + 4;
        let pos = tri(n, h);
        let w = 6 - s as i64; // block length 6/5/4/3 at GUI 100/75/50/25
        let lo = if n.rem_euclid(2 * h) < h { pos - w + 1 } else { pos - 5 };
        let hi = lo + w - 1;
        r.paint(|f, x| {
            let v = (12 * f + x) as i64;
            let vm = v.min(len - 1 - v);
            if (lo..=hi).contains(&vm) { c0 } else { c1 }
        });
    } else {
        let n = steps(t, 51.0 + 15.0 * fx.d(), RING_T0);
        let pos = if rev { 7 - n % 8 } else { n % 8 };
        r.paint(|_, x| if x as i64 == pos { c0 } else { c1 });
    }
}
