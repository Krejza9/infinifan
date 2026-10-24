//! ElectricCurrent: a short pulse runs slowly from the ends of the inner strip to its centre,
//! then fast outwards from the centre of the outer strip, in the next palette colour every cycle.

use crate::fxsim::Fx;
use crate::fxsim::blocks::{Canvas, Strips, end};

pub(crate) fn whole(fx: &Fx, cv: &mut Canvas, t: f64) {
    let (d, qi) = (fx.d(), cv.q as i64);
    let (no, ni5) = (12 * qi, 5 * qi);
    let ts = 61.0 + 10.0 * d;
    let tf = 8.174 + (24.547 - 8.174) * d / 4.0;
    let di = (ni5 - 1) / 2; // largest distance from an end on the inner strip
    let s_steps = di + 4;
    let f_steps = 14 * qi + 1;
    let cyc = s_steps as f64 * ts + f_steps as f64 * tf;
    let u = (t - 5.0).max(0.0) + 2.0 * ts; // the first cycle starts at slow step 2
    let k = (u / cyc).floor();
    let r = u - k * cyc;
    let slow = r < s_steps as f64 * ts;
    let h = if slow {
        (r / ts).floor() as i64
    } else {
        s_steps + ((r - s_steps as f64 * ts) / tf).floor() as i64
    };
    let col = fx.pal(k as i64);
    if slow {
        for x in 0..ni5 {
            let dist = x.min(ni5 - 1 - x);
            if dist >= h - 3 && dist <= h {
                cv.inner_p(x as usize, col);
            }
        }
    }
    let ho = h - di;
    if ho >= 1 {
        for x in 0..no {
            let e = (no / 2 - 1) - x.min(no - 1 - x);
            if e >= ho - 3 && e <= ho {
                cv.outer_j(x as usize, col);
            }
        }
    }
}

pub(crate) fn merge(fx: &Fx, st: &mut Strips, tau: f64) {
    let ni = st.n as i64;
    let (lo, rows) = (12 * ni, 5 * ni);
    let ts = (111 - 10 * fx.speed) as f64;
    let tf = (24 - 4 * fx.speed) as f64;
    let top_end = (rows - 1) / 2; // last row of the upper half
    let d = top_end + 4; // slow ticks until the inner rows are dark
    let p = d as f64 * ts + (lo / 2 + 2) as f64 * tf;
    let cyc = (tau / p).floor();
    let tt = tau - cyc * p;
    let col = fx.pal(cyc as i64); // next palette colour every cycle
    let (h, j) = if tt < d as f64 * ts {
        let h = (tt / ts).floor() as i64;
        (h, h - top_end)
    } else {
        (d, 4 + ((tt - d as f64 * ts) / tf).floor() as i64)
    };
    for r in 0..rows {
        let x = end(r, rows);
        if h - 3 <= x && x <= h {
            st.row(r as usize, col, 1, false);
        }
    }
    if j >= 1 {
        for x in 0..lo {
            // cen(x, L): 0 at the two middle LEDs
            let y = if x >= lo / 2 { x - lo / 2 } else { lo / 2 - 1 - x };
            if j - 3 <= y && y <= j {
                st.outer(x as usize, col, 1, false);
            }
        }
    }
}
