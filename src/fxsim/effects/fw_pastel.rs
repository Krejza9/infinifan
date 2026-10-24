//! FwPastel: soft pastel R/G/B waves ease in and out along the whole-fan strip, spilling into
//! the other ring.

use crate::fxsim::Fx;
use crate::fxsim::blocks::{Ring, WholeFanStrip, period, steps};

/// Ease-in curve.
const PASTEL25: [u8; 25] = [
    0, 1, 2, 4, 6, 9, 12, 16, 20, 24, 29, 34, 40, 46, 52, 59, 66, 73, 80, 88, 96, 103, 111, 119, 127,
];

fn value(st: &WholeFanStrip, s: usize) -> [i16; 3] {
    let (s, l, n) = (s as i64, st.len, st.n);
    [0, 1, 2].map(|c: i64| {
        if st.reverse && c > 0 {
            // reverse draws only red; the last position's G/B are not written at all
            return if s == l { -1 } else { 0 };
        }
        let phi = 25 * c + 24 - (2 * s - l * (c + 1) / 2).abs();
        let m = (n + phi).rem_euclid(75);
        i16::from(PASTEL25[m.min(74 - m).min(24) as usize])
    })
}

/// The effect on the whole-fan strip of an Inner (`outer` = false) or Outer effect; Pastel writes
/// one position more than the conveyors.
pub(crate) fn strip(fx: &Fx, outer: bool, t: f64) -> WholeFanStrip {
    WholeFanStrip::new(fx, outer, steps(t, period(fx, 46, 10), 5.0), 1, value)
}

pub(crate) fn ring(fx: &Fx, r: &mut Ring, t: f64) {
    strip(fx, r.outer, t).paint(r, fx.shift);
}
