//! ColorfulMeteor: a 2-LED meteor runs down the stacked outer frames, changing through the
//! 7-colour rainbow at every step.

use crate::fxsim::blocks::{RING_T0_B, Ring, period, steps};
use crate::fxsim::color::{RAINBOW7, shr};
use crate::fxsim::{BLACK, Fx};

pub(crate) fn outer(fx: &Fx, r: &mut Ring, t: f64) {
    let (rev, qi) = (fx.reverse, r.q as i64);
    let n = steps(t, period(fx, 63, 10), RING_T0_B);
    let pos = n % (12 * qi + 10);
    let c = shr(RAINBOW7[(pos % 7) as usize], fx.shift);
    r.paint(|f, x| {
        let xx = (12 * f + x) as i64;
        let d = pos - if rev { 47 - xx } else { xx };
        if d == 0 || d == 1 { c } else { BLACK }
    });
}
