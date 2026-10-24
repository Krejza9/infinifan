//! Colour tables and per-channel helpers shared by the effect modules.

use super::Rgb8;

/// 12-step colour wheel used by the "rainbow" palette effects (ring.md RAINBOW12, whole.md WHEEL12).
pub(crate) const RAINBOW12: [Rgb8; 12] = [
    [255, 0, 0],
    [178, 67, 0],
    [128, 128, 0],
    [67, 178, 0],
    [0, 255, 0],
    [0, 178, 67],
    [0, 128, 128],
    [0, 67, 178],
    [0, 0, 255],
    [67, 0, 178],
    [128, 0, 128],
    [178, 0, 67],
];

/// 7-colour rainbow of the outer meteor effects (ring.md RAINBOW7).
pub(crate) const RAINBOW7: [Rgb8; 7] = [
    [255, 0, 0],
    [255, 67, 0],
    [128, 128, 0],
    [0, 255, 0],
    [0, 128, 128],
    [0, 0, 255],
    [128, 0, 128],
];

/// `RAINBOW12` entry `k mod 12`.
pub(crate) fn rainbow12(k: i64) -> Rgb8 {
    RAINBOW12[k.rem_euclid(12) as usize]
}

/// `v >> s` per channel (any shift; 8 or more gives 0).
pub(crate) fn shr(c: Rgb8, s: u32) -> Rgb8 {
    c.map(|v| u32::from(v).checked_shr(s).unwrap_or(0) as u8)
}

/// `(c · k) >> 8` per channel ("scale256" with `k = 255 >> s`, level weights, …).
pub(crate) fn scale(c: Rgb8, k: u32) -> Rgb8 {
    c.map(|v| ((u32::from(v) * k) >> 8).min(255) as u8)
}

/// `floor(c · num / den)` per channel.
pub(crate) fn mul_div(c: Rgb8, num: u32, den: u32) -> Rgb8 {
    c.map(|v| (u32::from(v) * num / den).min(255) as u8)
}

/// Two dots on the same LED add with 8-bit wrap-around (ring.md §3.3).
pub(crate) fn add_wrap(a: Rgb8, b: Rgb8) -> Rgb8 {
    [a[0].wrapping_add(b[0]), a[1].wrapping_add(b[1]), a[2].wrapping_add(b[2])]
}

/// Puts `a` on the channel of `third` and `b` on the next primary (R→G→B→R).
fn primaries(third: usize, a: u8, b: u8) -> Rgb8 {
    let mut out = [0; 3];
    out[third] = a;
    out[(third + 1) % 3] = b;
    out
}

/// wheel128(n): 128-step fully saturated wheel with maximum 160. Thirds start at 0 (red),
/// 43 (green) and 86 (blue); at offset `j` in a third: `j ≤ 21` → (160, floor(7.5j + 0.5)),
/// else (floor(321 − 7.5j), 160) for (current, next primary).
pub(crate) fn wheel128(n: i64) -> Rgb8 {
    let n = n.rem_euclid(128);
    let third = if n < 43 { 0 } else if n < 86 { 1 } else { 2 };
    let j = n - [0, 43, 86][third];
    if j <= 21 {
        primaries(third, 160, ((15 * j + 1) / 2) as u8)
    } else {
        primaries(third, ((642 - 15 * j) / 2) as u8, 160)
    }
}

/// hsv8(h): 8-bit integer HSV with s = v = 255.
pub(crate) fn hsv8(h: i64) -> Rgb8 {
    let h = h.rem_euclid(256);
    let region = h / 43;
    let rem = (h - 43 * region) * 6;
    let q = ((255 * (255 - ((255 * rem) >> 8))) >> 8) as u8;
    let t = ((255 * (255 - ((255 * (255 - rem)) >> 8))) >> 8) as u8;
    match region {
        0 => [255, t, 0],
        1 => [q, 255, 0],
        2 => [0, 255, t],
        3 => [0, q, 255],
        4 => [t, 0, 255],
        _ => [255, 0, q],
    }
}

/// whole.md `wheel(h, M)`: piecewise linear hue wheel, `h` in turns, peak `M`, floored.
pub(crate) fn wheel(h: f64, m: f64) -> Rgb8 {
    let h = h.rem_euclid(1.0) * 6.0;
    let seg = (h.floor() as i64).rem_euclid(6);
    let f = h - h.floor();
    let (up, dn) = (m * f, m * (1.0 - f));
    let [r, g, b] = match seg {
        0 => [m, up, 0.0],
        1 => [dn, m, 0.0],
        2 => [0.0, m, up],
        3 => [0.0, dn, m],
        4 => [up, 0.0, m],
        _ => [m, 0.0, dn],
    };
    [r, g, b].map(|v| (v + 1e-9).floor().clamp(0.0, 255.0) as u8)
}

/// whole.md `SIXTH(k, a, b)`: 60° steps, `(a,0,0) (b,b,0) (0,a,0) (0,b,b) (0,0,a) (b,0,b)`.
pub(crate) fn sixth(k: i64, a: u8, b: u8) -> Rgb8 {
    match k.rem_euclid(6) {
        0 => [a, 0, 0],
        1 => [b, b, 0],
        2 => [0, a, 0],
        3 => [0, b, b],
        4 => [0, 0, a],
        _ => [b, 0, b],
    }
}
