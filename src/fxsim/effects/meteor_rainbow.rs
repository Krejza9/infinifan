//! MeteorRainbow: a meteor with a trail in rainbow colours (inner: one per ring, the colour
//! changing every lap; outer: one down the stacked frames, coloured from the 7-colour rainbow).

use crate::fxsim::blocks::{METEOR_INNER_TRAIL, RING_T0_B, Ring, period, steps, trail};
use crate::fxsim::color::{RAINBOW7, rainbow12, shr};
use crate::fxsim::{BLACK, Fx};

pub(crate) fn ring(fx: &Fx, r: &mut Ring, t: f64) {
    let (s, rev, qi) = (fx.shift, fx.reverse, r.q as i64);
    if r.outer {
        let n = steps(t, period(fx, 59, 10), RING_T0_B);
        let cyc = 12 * qi + 10;
        let (pos, cyc_no) = (n % cyc, n / cyc);
        r.paint(|f, x| {
            let xx = (12 * f + x) as i64;
            let d = pos - if rev { 47 - xx } else { xx };
            if !(0..7).contains(&d) {
                return BLACK;
            }
            // Quirk: step 0 of a later cycle still uses the old counter for the colours.
            let ci = if pos == 0 && cyc_no > 0 { (cyc + d) % 7 } else { (pos + d) % 7 };
            shr(RAINBOW7[ci as usize], s)
        });
    } else {
        let n = steps(t, period(fx, 53, 10), RING_T0_B);
        let (pos, c) = (n % 18, rainbow12(n / 18));
        r.paint(|_, x| {
            let idx = if rev { 7 - x as i64 } else { x as i64 };
            shr(trail(c, pos - idx, &METEOR_INNER_TRAIL), s)
        });
    }
}
