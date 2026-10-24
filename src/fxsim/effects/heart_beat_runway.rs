//! HeartBeatRunway: a heartbeat pulse in colour 1 that runs from fan to fan (and ring to ring).

use crate::fxsim::blocks::{Canvas, RING_T0_B, Ring, Strips, period, steps};
use crate::fxsim::color::scale;
use crate::fxsim::{Fx, OUTER_BASE};

/// Whole fan: extra ticks per cycle (`C = 64q + X`) for speed index 0..4 (measured table).
const HBR_X: [i64; 5] = [102, 108, 134, 159, 184];
/// Inner ring: extra ticks per cycle (`C = 32q + X`) for speed index 0..4 (measured table).
const HBR_EXTRA: [i64; 5] = [134, 140, 166, 191, 216];

/// Heartbeat-runway ramp: `min(4u, 255)` up to u = 64, then `255 − 4(u − 64)`, never below 16.
fn ramp_level(u: i64) -> u32 {
    let l = if u <= 64 { (4 * u).min(255) } else { 255 - 4 * (u - 64) };
    l.max(16) as u32
}

pub(crate) fn whole(fx: &Fx, cv: &mut Canvas, t: f64) {
    let (rev, qi) = (fx.reverse, cv.q as i64);
    let c = 64 * qi + HBR_X[fx.speed as usize];
    let u0 = steps(t, 4.0 + 2.0 * fx.d(), 6.0) % c;
    // slots: inner fan 0..q−1, then outer fan 0..q−1 (reverse: the reversed list)
    for slot in 0..2 * qi {
        let j = if rev { 2 * qi - 1 - slot } else { slot };
        let l = ramp_level(u0 - 32 * j);
        let col = fx.pal(0).map(|v| ((u32::from(v) * l) >> 8) as u8);
        let f = (slot % qi) as usize;
        if slot < qi {
            (0..8).for_each(|i| cv.inner(f, i, col));
        } else {
            (0..12).for_each(|jj| cv.outer(f, jj, col));
        }
    }
}

pub(crate) fn inner(fx: &Fx, r: &mut Ring, t: f64) {
    let (rev, qi) = (fx.reverse, r.q as i64);
    let tau = period(fx, 12, 2);
    let c = 32 * qi + HBR_EXTRA[fx.speed as usize];
    let u0 = steps(t, tau, RING_T0_B) % c;
    let col = fx.pal(0);
    r.paint(|f, _| {
        let slot = if rev { 3 - f as i64 } else { f as i64 }; // four-slot mirror
        let u = u0 - 32 * slot;
        let l = if u >= 0 { ramp_level(u) } else { 16 };
        scale(col, l >> fx.shift)
    });
}

pub(crate) fn merge(fx: &Fx, st: &mut Strips, tau: f64) {
    let ni = st.n as i64;
    let tick = steps(tau, (12 - 2 * fx.speed) as f64, 0.0) % (44 * ni + 128);
    let mb = st.mb as i64;
    let base = 15 * mb / 256;
    let peak = 255 * (mb - 1) / 256;
    let c0 = fx.pal(0);
    for k in 0..st.n {
        // unit positions: inner ring of fan k → k, outer frame of fan k → 4 + k
        for (outer, pos) in [(false, k as i64), (true, 4 + k as i64)] {
            let pos = if fx.reverse { ni + 3 - pos } else { pos };
            let m = tick - 4 - 32 * pos;
            let lv = if (0..=60).contains(&m) {
                peak.min(base + 4 * m * mb / 256)
            } else if 60 < m && m < 120 {
                base.max(peak - 4 * (m - 60) * mb / 256)
            } else {
                base
            };
            // the level is already at the final brightness
            let rgb = c0.map(|v| ((lv * i64::from(v) + 127) / 255) as u8);
            let (g, f) = st.fans[k];
            let leds = &mut st.out[g];
            if outer {
                leds[OUTER_BASE + 12 * f..OUTER_BASE + 12 * f + 12].fill(rgb);
            } else {
                leds[8 * f..8 * f + 8].fill(rgb);
            }
        }
    }
}
