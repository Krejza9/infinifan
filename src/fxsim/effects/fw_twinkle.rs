//! FwTwinkle: single LEDs sparkle at half level over a dim background in a slowly changing hue.

use crate::fxsim::blocks::{Canvas, RING_T0, Ring, steps};
use crate::fxsim::color::{hsv8, mul_div, shr};
use crate::fxsim::{Fx, Rgb8};

/// Draw indices (mod 512) at which FwTwinkle lights an LED.
const TWINKLE_FIRE: [i64; 10] = [18, 54, 197, 227, 241, 407, 424, 492, 495, 496];

/// whole.md FwTwinkle hue: 6-sector wheel with maximum 255 advancing 6 units per step.
fn hue6(k: i64) -> Rgb8 {
    let x = (6 * k).rem_euclid(6 * 255);
    let (sec, f) = (x / 255, (x % 255) as u8);
    let (up, dn) = (f, 255 - f);
    match sec {
        0 => [255, up, 0],
        1 => [dn, 255, 0],
        2 => [0, 255, up],
        3 => [0, dn, 255],
        4 => [up, 0, 255],
        _ => [255, 0, dn],
    }
}

pub(crate) fn whole(fx: &Fx, cv: &mut Canvas, t: f64) {
    let n = steps(t, 19.0 + 7.0 * fx.d(), 5.5);
    let c = hue6((n + 1) / 26);
    let bg = mul_div(c, 16, 255);
    let sp = mul_div(c, 128, 255);
    // one draw per LED per tick over the fan's 20 LEDs (inner x = i, outer x = 8 + j)
    let fire = |x: i64| TWINKLE_FIRE.contains(&(20 * n + x).rem_euclid(512));
    for f in 0..4 {
        (0..8).for_each(|i| cv.inner(f, i, if fire(i as i64) { sp } else { bg }));
        (0..12).for_each(|j| cv.outer(f, j, if fire(8 + j as i64) { sp } else { bg }));
    }
}

pub(crate) fn ring(fx: &Fx, r: &mut Ring, t: f64) {
    let (s, nl) = (fx.shift, r.n as i64);
    let n = steps(t, 19.0 + 7.0 * fx.d(), RING_T0);
    let c = hsv8((n + 1) / 26);
    let bg = shr(c.map(|v| ((u32::from(v) + 1) * 16 >> 8) as u8), s);
    let sp = shr(c.map(|v| ((u32::from(v) + 1) * 128 >> 8) as u8), s);
    r.paint(|_, x| {
        let d = (n * nl + x as i64).rem_euclid(512);
        if TWINKLE_FIRE.contains(&d) { sp } else { bg }
    });
}
