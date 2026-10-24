//! Disco: the palette colours side by side around each ring and frame, rotating step by step.

use crate::fxsim::Fx;
use crate::fxsim::blocks::{Canvas, RING_T0, Ring, steps, x254};

pub(crate) fn whole(fx: &Fx, cv: &mut Canvas, t: f64) {
    let to = 11.0 + 15.0 * fx.d();
    let (n_o, n_i) = (steps(t, to, 6.0), steps(t, to + 5.0, 6.0));
    let dir = if fx.reverse { -1 } else { 1 };
    for x in 0..48 {
        cv.outer_j(x as usize, x254(fx.pal((x - dir * n_o).div_euclid(3))));
    }
    for f in 0..4 {
        for i in 0..8 {
            cv.inner(f, i, x254(fx.pal(i as i64 - dir * n_i)));
        }
    }
}

pub(crate) fn ring(fx: &Fx, r: &mut Ring, t: f64) {
    let outer = r.outer;
    let p = if outer { 11.0 } else { 16.0 } + 15.0 * fx.d();
    let n = steps(t, p, RING_T0);
    let m = if fx.reverse { -n } else { n };
    let block = if outer { 3 } else { 1 };
    r.paint(|_, x| fx.pal_s((x as i64 - m).div_euclid(block)));
}
