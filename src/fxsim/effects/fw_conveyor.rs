//! FwConveyor: R, G and B sine bumps travel along the whole-fan strip (inner and outer LEDs of
//! each fan in turn), spilling into the other ring.

use crate::fxsim::Fx;
use crate::fxsim::blocks::{RING_T0, Ring, WholeFanStrip, p75_bump, period, steps};

fn value(st: &WholeFanStrip, s: usize) -> [i16; 3] {
    let (l, n) = (st.len, st.n);
    let phase = if st.reverse { [26 + n, (l - 26) - n, 1 + n] } else { [-n, 25 - n, 50 - n] };
    p75_bump(phase, s)
}

/// The effect on the whole-fan strip of an Inner (`outer` = false) or Outer effect.
pub(crate) fn strip(fx: &Fx, outer: bool, t: f64) -> WholeFanStrip {
    WholeFanStrip::new(fx, outer, steps(t, period(fx, 51, 10), RING_T0), 0, value)
}

pub(crate) fn ring(fx: &Fx, r: &mut Ring, t: f64) {
    strip(fx, r.outer, t).paint(r, fx.shift);
}
