//! Mixing: two 3-LED meteors in colours 1 and 2 approach from both ends, cross in the centre and
//! spread out as an alternating band.

use crate::fxsim::blocks::{Canvas, MAX_FANS, RING_T0, Ring, Strips, level, steps};
use crate::fxsim::color::shr;
use crate::fxsim::{BLACK, Fx, LEVEL, Rgb8};

/// Whole-fan Mixing on a strip of `m` positions (`m` even), step `n` (whole.md §1.11).
fn whole_strip(n: i64, m: usize, c1: Rgb8, c2: Rgb8, buf: &mut [Rgb8; 48]) {
    let len = m as i64;
    let c = len / 2;
    let k = n % (4 * c);
    buf[..m].fill(BLACK);
    let mut set = |x: i64, col: Rgb8| {
        if (0..len).contains(&x) {
            buf[x as usize] = col;
        }
    };
    if k < 2 * c {
        let a = k / 2;
        for (d, v) in [(0, 255), (1, 127), (2, 63)] {
            set(a - d, level(c1, v));
            set(len - 1 - a + d, level(c2, v));
        }
    } else if k < 2 * c + 2 {
        let pat: [(u32, bool); 6] = if k == 2 * c {
            [(63, true), (127, true), (255, false), (255, true), (127, false), (63, false)]
        } else {
            [(63, true), (255, false), (255, true), (127, false), (127, true), (63, false)]
        };
        for (i, (v, first)) in pat.into_iter().enumerate() {
            set(c - 3 + i as i64, level(if first { c1 } else { c2 }, v));
        }
    } else {
        let mm = k - (2 * c + 2);
        for p in 0..len {
            let on = if mm <= c - 3 {
                p >= c - 3 - mm && p <= c + 2 + mm
            } else {
                let g = mm - (c - 3);
                p < c - g || p > c + g - 1
            };
            if on {
                set(p, if (p - c + mm).rem_euclid(2) == 1 { c2 } else { c1 });
            }
        }
    }
}

pub(crate) fn whole(fx: &Fx, cv: &mut Canvas, t: f64) {
    let qi = cv.q as i64;
    let (no, ni5) = (12 * qi, 5 * qi);
    let n = steps(t, 19.0 + 6.0 * fx.d(), 7.0);
    let (co, ci) = (6 * qi, (5 * qi + 1) / 2);
    let n_in = (n as f64 * ci as f64 / co as f64 + 0.05).floor() as i64;
    let mut buf = [BLACK; 48];
    whole_strip(n, 2 * co as usize, fx.pal(0), fx.pal(1), &mut buf);
    for (x, &c) in buf[..no as usize].iter().enumerate() {
        cv.outer_j(x, c);
    }
    whole_strip(n_in, 2 * ci as usize, fx.pal(0), fx.pal(1), &mut buf);
    for (x, &c) in buf[..ni5 as usize].iter().enumerate() {
        cv.inner_p(x, c);
    }
}

/// Ring Mixing on a strip of `len` positions (ring.md §4.34); `buf` gets the full-brightness colours.
fn ring_strip(n: i64, len: usize, q: i64, c0: Rgb8, c1: Rgb8, buf: &mut [Rgb8; 48]) {
    let l = (len + len % 2) as i64; // even virtual length (inner: one phantom position)
    let h = l / 2;
    let tt = 12 * q;
    let n = n.rem_euclid(2 * tt);
    for v in buf.iter_mut() {
        *v = BLACK;
    }
    let mut set = |x: i64, c: Rgb8| {
        if (0..l).contains(&x) {
            buf[x as usize] = c;
        }
    };
    let tr0 = [c0, shr(c0, 1), shr(c0, 2)];
    let tr1 = [c1, shr(c1, 1), shr(c1, 2)];
    if n < tt {
        // approach: two 3-LED meteors
        let x = n * h / tt;
        for j in 0..3 {
            set(x - j as i64, tr0[j]);
            set(l - 1 - x + j as i64, tr1[j]);
        }
        return;
    }
    let y = (n - tt) * l / tt;
    if y < 2 {
        let cross = if y == 0 {
            [tr0[2], tr0[1], c1, c0, tr1[1], tr1[2]]
        } else {
            [tr0[2], c1, c0, tr1[1], tr0[1], tr1[2]]
        };
        for (i, c) in cross.into_iter().enumerate() {
            set(h - 3 + i as i64, c);
        }
        return;
    }
    let k = y - 2;
    for u in 0..l {
        let on = if k <= h - 3 {
            u >= h - 3 - k && u < h + 3 + k
        } else {
            let w = k - (h - 3);
            u < h - w || u >= h + w
        };
        if on {
            set(u, if (u - (h - 3) + k).rem_euclid(2) == 0 { c1 } else { c0 });
        }
    }
}

pub(crate) fn ring(fx: &Fx, r: &mut Ring, t: f64) {
    let (s, q, outer) = (fx.shift, r.q, r.outer);
    let n = steps(t, 19.0 + 6.0 * fx.d(), RING_T0);
    let len = if outer { 12 * q } else { 5 * q };
    let mut buf = [BLACK; 48];
    ring_strip(n, len, q as i64, fx.pal(0), fx.pal(1), &mut buf);
    r.paint(|f, x| {
        let pos = if outer { 12 * f + x } else { 5 * f + LEVEL[x] };
        shr(buf[pos], s)
    });
}

/// Merged Mixing on a strip of `len` positions at counter `c` (merge.md §9). Writes
/// (colour, divisor) per position; divisor 0 = black.
fn merge_frame(c: i64, len: usize, red: Rgb8, blue: Rgb8, inner: bool, buf: &mut [(Rgb8, u32)]) {
    let l = len as i64;
    let m = l / 2;
    let buf = &mut buf[..len];
    buf.fill((BLACK, 0));
    let mut set = |x: i64, col: Rgb8, div: u32| {
        if (0..l).contains(&x) {
            buf[x as usize] = (col, div);
        }
    };
    if c < 2 * m + 4 {
        if inner && c >= 2 * m {
            // inner only: crossing / mixing frames
            let pat = if c < 2 * m + 2 {
                [(red, 4), (red, 2), (blue, 1), (red, 1), (blue, 2), (blue, 4)]
            } else {
                [(red, 4), (blue, 1), (red, 1), (blue, 2), (red, 2), (blue, 4)]
            };
            for (i, (col, div)) in pat.into_iter().enumerate() {
                set(m - 3 + i as i64, col, div);
            }
            return;
        }
        // two comets approach the centre (trail first, head last)
        let h = (c / 2).min(m - 1);
        for (d, div) in [(2, 4), (1, 2), (0, 1)] {
            set(h - d, red, div);
            set(l - 1 - h + d, blue, div);
        }
        return;
    }
    let alt = |x: i64| if (x + c).rem_euclid(2) == 1 { blue } else { red };
    if c <= 3 * m + 1 {
        // the alternating band grows from the centre
        let w = c - 2 * m - 1;
        for x in m - w..m + w {
            set(x, alt(x), 1);
        }
    } else {
        // the band shrinks (outer: towards raw 0; inner: towards both ends)
        let n = 4 * m + 1 - c;
        for x in 0..l {
            if x < n || (inner && x >= l - n) {
                set(x, alt(x), 1);
            }
        }
    }
}

pub(crate) fn merge(fx: &Fx, st: &mut Strips, tau: f64) {
    let lo = 12 * st.n as i64;
    let (lo_u, li) = (lo as usize, 4 * st.n);
    let (n_o, n_i) = (2 * lo + 2, 2 * li as i64 + 2);
    let u = steps(tau, (29 - 5 * fx.speed) as f64, 0.0) % n_o;
    let (red, blue) = (fx.pal(0), fx.pal(1));
    let mut buf = [(BLACK, 0u32); 12 * MAX_FANS];
    merge_frame(u, lo_u, red, blue, false, &mut buf);
    for (x, &(c, div)) in buf[..lo_u].iter().enumerate() {
        st.outer(x, c, div.max(1), false);
    }
    merge_frame(u * n_i / n_o, li, red, blue, true, &mut buf);
    for (x, &(c, div)) in buf[..li].iter().enumerate() {
        st.pair(x, c, div.max(1), false);
    }
}
