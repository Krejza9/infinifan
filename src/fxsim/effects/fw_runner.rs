//! FwRunner: a bright dot runs along the strip and stacks up at its far end over a dim rainbow
//! background; the colours move on every round.

use crate::fxsim::Fx;
use crate::fxsim::blocks::{Canvas, RING_T0_B, Ring, period, ring_rainbow_bg, stack_state, steps};
use crate::fxsim::color::{rainbow12, scale, shr, sixth};

pub(crate) fn whole(fx: &Fx, cv: &mut Canvas, t: f64) {
    let n = steps(t, 11.0 + 10.0 * fx.d(), 5.0);
    for (m, outer) in [(32i64, false), (48, true)] {
        let k = m * (m + 1) / 2 + 2; // states incl. 2 "full" states
        let (r, st) = (n / k, n % k);
        let (pile, p) = if st >= k - 2 { (m, -1) } else { stack_state(st, m) };
        let bg = sixth(r, 63, 32);
        let run = sixth(r + 2, 254, 127);
        let stk = sixth(r + 2, 63, 32);
        for x in 0..m {
            let c = if x == p {
                run
            } else if x >= m - pile {
                stk
            } else {
                bg
            };
            if outer { cv.outer_j(x as usize, c) } else { cv.inner_i(x as usize, c) }
        }
    }
}

/// Stacking runner (ring.md §3.4) at round step `m` on `len` positions:
/// (falling dot position, stacked LEDs at the far end).
fn runner_state(m: i64, len: i64) -> (i64, i64) {
    let full = len * (len + 1) / 2;
    if m >= full {
        return (m - full, len); // 2 extra steps: all stacked, dot at 0 then 1
    }
    let start = |k: i64| k * len - k * (k - 1) / 2;
    let mut k = 0;
    while start(k + 1) <= m {
        k += 1;
    }
    (m - start(k), k)
}

pub(crate) fn ring(fx: &Fx, r: &mut Ring, t: f64) {
    let (s, nl) = (fx.shift, r.n as i64);
    let n = steps(t, period(fx, 51, 10), RING_T0_B);
    let m_len = 4 * nl; // four-slot strip
    let round = m_len * (m_len + 1) / 2 + 2;
    let (rr, m) = (n / round, n % round);
    let (pos, k) = runner_state(m, m_len);
    let bg = ring_rainbow_bg(rr, s);
    let dotc = rainbow12(2 * rr + 4);
    let dot = scale(dotc, 255 >> s);
    let stk = shr(shr(dotc, 2), s);
    r.paint(|f, x| {
        let j = nl * f as i64 + x as i64;
        if j == pos {
            dot
        } else if j >= m_len - k {
            stk
        } else {
            bg
        }
    });
}
