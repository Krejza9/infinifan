//! Meteor: a meteor with a fading trail per inner ring, and one down the stacked outer frames,
//! in the fans' palette colours.

use crate::fxsim::blocks::{Canvas, METEOR_INNER_TRAIL, RING_T0_B, Ring, level, period, steps, trail};
use crate::fxsim::color::shr;
use crate::fxsim::{BLACK, Fx};

const METEOR_IN: [u32; 9] = [255, 127, 63, 31, 15, 7, 3, 3, 3];
const METEOR_OUT: [u32; 9] = [255, 255, 255, 255, 255, 7, 3, 3, 3];
const METEOR_OUTER_TRAIL: [u32; 9] = [0, 0, 0, 0, 0, 5, 6, 6, 6];

pub(crate) fn whole(fx: &Fx, cv: &mut Canvas, t: f64) {
    let rev = fx.reverse;
    let no = 12 * cv.q as i64;
    let n = steps(t, 8.0 + 10.0 * fx.d(), 5.0);
    // inner: a meteor per fan, cycle 18 steps, colour P[f]
    let head = n % 18;
    for f in 0..4 {
        for i in 0..8 {
            let pos = if rev { 7 - i as i64 } else { i as i64 };
            let lv = usize::try_from(head - pos).ok().and_then(|a| METEOR_IN.get(a));
            cv.inner(f, i, lv.map_or(BLACK, |&l| level(fx.pal(f as i64), l)));
        }
    }
    // outer: one meteor down the stacked frame; reverse mirrors in a fixed 48-LED space
    let head = n % (no + 10);
    for x in 0..no {
        let pos = if rev { 47 - x } else { x };
        let lv = usize::try_from(head - pos).ok().and_then(|a| METEOR_OUT.get(a));
        cv.outer_j(x as usize, lv.map_or(BLACK, |&l| level(fx.pal(x / 12), l)));
    }
}

pub(crate) fn ring(fx: &Fx, r: &mut Ring, t: f64) {
    let (s, rev, qi) = (fx.shift, fx.reverse, r.q as i64);
    let n = steps(t, period(fx, 48, 10), RING_T0_B);
    if r.outer {
        let pos = n % (12 * qi + 10);
        r.paint(|f, x| {
            let xx = (12 * f + x) as i64;
            let j = if rev { 47 - xx } else { xx }; // reverse: four-slot mirror
            shr(trail(fx.pal(f as i64), pos - j, &METEOR_OUTER_TRAIL), s)
        });
    } else {
        let pos = n % 18;
        r.paint(|f, x| {
            let idx = if rev { 7 - x as i64 } else { x as i64 };
            shr(trail(fx.pal(f as i64), pos - idx, &METEOR_INNER_TRAIL), s)
        });
    }
}
