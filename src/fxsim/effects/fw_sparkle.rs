//! FwSparkle: a sweep along each ring and frame, then a swell in a half-rainbow gradient.

use crate::fxsim::blocks::{Canvas, RING_T0, Ring, steps};
use crate::fxsim::color::{shr, wheel128};
use crate::fxsim::{BLACK, Fx, Rgb8};

/// FwSparkle on a strip of `N ≤ 12` LEDs at step `n`: a sweep, then a swell in a half-rainbow
/// gradient (identical in whole.md §1.23 and ring.md §4.39). Full-brightness colours.
fn sparkle(n: i64, out: &mut [Rgb8]) {
    let len = out.len() as i64;
    let k = len + 50;
    let (c, m) = (n.div_euclid(k), n.rem_euclid(k));
    let j = m - len;
    let jj = j.min(50 - j).clamp(0, 25);
    let a0 = if j >= 0 {
        (255.0 * (std::f64::consts::FRAC_PI_2 * jj as f64 / 25.0).sin()).round() as i64
    } else {
        0
    };
    let mut a = a0;
    for (x, led) in out.iter_mut().enumerate() {
        let x = x as i64;
        if x > 0 {
            a = (a * a0) >> 8;
        }
        let env = if j < 0 && x == m { 229.0 } else { 15.0 + a as f64 * 214.0 / 255.0 };
        let hue = wheel128(8 * (len * (c + 1) + x));
        *led = hue.map(|v| (f64::from(v) * env / 160.0 + 1e-9).floor() as u8);
    }
}

pub(crate) fn whole(fx: &Fx, cv: &mut Canvas, t: f64) {
    let n = steps(t, 8.0 + 4.0 * fx.d(), 5.5);
    let (mut si, mut so) = ([BLACK; 8], [BLACK; 12]);
    sparkle(n, &mut si);
    sparkle(n, &mut so);
    for f in 0..4 {
        (0..8).for_each(|i| cv.inner(f, i, si[i]));
        (0..12).for_each(|j| cv.outer(f, j, so[j]));
    }
}

pub(crate) fn ring(fx: &Fx, r: &mut Ring, t: f64) {
    let n = steps(t, 8.0 + 4.0 * fx.d(), RING_T0);
    let mut strip = [BLACK; 12];
    sparkle(n, &mut strip[..r.n]);
    r.paint(|_, x| shr(strip[x], fx.shift));
}
