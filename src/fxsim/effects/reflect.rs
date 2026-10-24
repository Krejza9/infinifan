//! Reflect: two mirrored comets with fading trails run from the ends of the outer frame strip
//! towards its middle.

use crate::fxsim::blocks::{RING_T0, Ring, period, steps};
use crate::fxsim::color::shr;
use crate::fxsim::{BLACK, Fx};

const REFLECT_TRAIL: [u32; 11] = [0, 0, 1, 1, 1, 2, 2, 2, 3, 3, 3];

pub(crate) fn outer(fx: &Fx, r: &mut Ring, t: f64) {
    let (s, qi) = (fx.shift, r.q as i64);
    let n = steps(t, period(fx, 51, 5), RING_T0);
    let len = 12 * qi;
    let half = len / 2;
    let h = n % (half + REFLECT_TRAIL.len() as i64 + 5) - 1; // head, −1 = dark
    let col = fx.pal(0);
    r.paint(|f, x| {
        let xx = (12 * f + x) as i64;
        let xm = if xx < half { xx } else { len - 1 - xx }; // second half mirrored
        match usize::try_from(h - xm).ok().and_then(|d| REFLECT_TRAIL.get(d)) {
            Some(&sh) => shr(col, sh + 2 * s), // rule R4: v >> 2s
            None => BLACK,
        }
    });
}
