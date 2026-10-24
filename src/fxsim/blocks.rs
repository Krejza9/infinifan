//! Building blocks shared by several effects: the LED views of the three scopes (whole-fan
//! canvas, ring, merged strips), step counters, bouncing / ping-pong segments, stacking, the
//! whole-fan strip and its spill, and brightness helpers.

use super::color::{add_wrap, mul_div, rainbow12, scale, shr};
use super::{BLACK, Fx, GroupLeds, LEVEL, OUTER_BASE, Rgb8};

// ------------------------------------------------------------------------------- step counters

/// Start latency of every ring effect (ring.md §2.1): `n = floor((t − 5.5) / P)`.
pub(crate) const RING_T0: f64 = 5.5;
/// Some ring effects (meteor family, FwRunner, FwConverge, HeartBeatRunway) fit 6.0 ms better.
pub(crate) const RING_T0_B: f64 = 6.0;

/// Step counter `n = floor((t - t0) / period)`, never negative (state 0 is held until the first
/// step, also for times before the start offset).
pub(crate) fn steps(t: f64, period: f64, t0: f64) -> i64 {
    ((t - t0) / period).floor().max(0.0) as i64
}

/// Ring effect period `P = a − b·i` ms (ring.md §6).
pub(crate) fn period(fx: &Fx, a: i64, b: i64) -> f64 {
    (a - b * fx.speed) as f64
}

// -------------------------------------------------------------------------- brightness helpers

impl Fx<'_> {
    /// Palette colour with the ring "scale256" brightness rule R2: `(c · (255 >> s)) >> 8`.
    pub fn pal_s(&self, k: i64) -> Rgb8 {
        scale(self.pal(k), 255 >> self.shift)
    }
}

/// `floor(c · level / 255)`.
pub(crate) fn level(c: Rgb8, l: u32) -> Rgb8 {
    mul_div(c, l, 255)
}

/// "×254" colours: `floor(c · 254 / 255)`.
pub(crate) fn x254(c: Rgb8) -> Rgb8 {
    mul_div(c, 254, 255)
}

// ---------------------------------------------------------------------------- whole-fan canvas

/// The LEDs of one group seen as the strips of whole.md §0.1 (scope Whole). Effects draw at full
/// brightness on all 4 fan slots; missing fans and brightness are handled afterwards.
pub(crate) struct Canvas<'a> {
    pub leds: &'a mut GroupLeds,
    /// Installed fans.
    pub q: usize,
}

impl Canvas<'_> {
    /// Inner ring LED `i` of fan `f`.
    pub fn inner(&mut self, f: usize, i: usize, c: Rgb8) {
        self.leds[8 * f + i] = c;
    }
    /// Outer frame LED `j` (wire order) of fan `f`.
    pub fn outer(&mut self, f: usize, j: usize, c: Rgb8) {
        self.leds[OUTER_BASE + 12 * f + j] = c;
    }
    /// Every LED (inner and outer) of fan `f`.
    pub fn fan(&mut self, f: usize, c: Rgb8) {
        (0..8).for_each(|i| self.inner(f, i, c));
        (0..12).for_each(|j| self.outer(f, j, c));
    }
    /// Stacked outer strip position `J = 12f + j`.
    pub fn outer_j(&mut self, pos: usize, c: Rgb8) {
        self.leds[OUTER_BASE + pos] = c;
    }
    /// Stacked inner LED strip position `I = 8f + i`.
    pub fn inner_i(&mut self, pos: usize, c: Rgb8) {
        self.leds[pos] = c;
    }
    /// Stacked inner 5-position strip `P = 5f + k` (k = 0 top … 4 bottom, mirrored halves).
    pub fn inner_p(&mut self, pos: usize, c: Rgb8) {
        let (f, k) = (pos / 5, pos % 5);
        for i in 0..8 {
            if LEVEL[i] == k {
                self.leds[8 * f + i] = c;
            }
        }
    }
    /// Same colour on every inner (resp. outer) LED of all 4 fan slots.
    pub fn all_inner(&mut self, c: Rgb8) {
        self.leds[..OUTER_BASE].fill(c);
    }
    pub fn all_outer(&mut self, c: Rgb8) {
        self.leds[OUTER_BASE..].fill(c);
    }
}

// ------------------------------------------------------------------------------------- ring

/// One ring of one group (scopes Inner and Outer): LED `x` of fan `f` is `leds[base + n·f + x]`.
pub(crate) struct Ring<'a> {
    pub leds: &'a mut GroupLeds,
    pub base: usize,
    /// LEDs per fan: 8 (inner) or 12 (outer).
    pub n: usize,
    /// Installed fans.
    pub q: usize,
    /// Outer frame (Outer scope) rather than inner ring.
    pub outer: bool,
}

impl Ring<'_> {
    /// Sets every LED of the installed fans to `col(fan, led)`.
    #[inline]
    pub fn paint(&mut self, mut col: impl FnMut(usize, usize) -> Rgb8) {
        for f in 0..self.q {
            for x in 0..self.n {
                self.leds[self.base + self.n * f + x] = col(f, x);
            }
        }
    }
}

// ---------------------------------------------------------------------------- merged strips

/// Constant start latency of the merged effects: everything is a function of `τ = t − 7 ms`.
pub(crate) const LATENCY: f64 = 7.0;
pub(crate) const MAX_FANS: usize = 16;

/// The merged strips over the installed fans of all groups, and the output brightness
/// (merge.md). Fans are numbered globally (group order, then fan order; empty groups are
/// skipped) and the effect draws on 1-D strips that run through the whole stack:
/// **O** outer frames (`12N`), **I8** inner LEDs (`8N`), **IR** inner rows (`5N`) and
/// **IP** inner left/right pairs (`4N`).
pub(crate) struct Strips<'a> {
    pub out: &'a mut [GroupLeds; 4],
    /// Global fan k → (group, fan in group).
    pub fans: [(usize, usize); MAX_FANS],
    /// Total fans `N`.
    pub n: usize,
    /// `Mb = 256 >> s` (256/128/64/32/16 for GUI 100/75/50/25/0).
    pub mb: u32,
}

impl Strips<'_> {
    /// Writes nominal colour `c / div` to a LED with the general brightness rule
    /// `floor(v · (Mb − d) / 256)`, `d = 1` for the "minus-one" colours (`dim`).
    fn put(&mut self, g: usize, led: usize, c: Rgb8, div: u32, dim: bool) {
        let m = self.mb - u32::from(dim);
        self.out[g][led] = c.map(|v| (u32::from(v) * m / (256 * div)) as u8);
    }

    /// Outer strip position `x = 12k + j`.
    pub fn outer(&mut self, x: usize, c: Rgb8, div: u32, dim: bool) {
        let (g, f) = self.fans[x / 12];
        self.put(g, OUTER_BASE + 12 * f + x % 12, c, div, dim);
    }

    /// I8 position `x = 8k + i` (ring LED i, clockwise from the top).
    pub fn inner8(&mut self, x: usize, c: Rgb8, div: u32, dim: bool) {
        let (g, f) = self.fans[x / 8];
        self.put(g, 8 * f + x % 8, c, div, dim);
    }

    /// IR row `r = 5k + ρ`: all ring LEDs i of fan k with `min(i, 8 − i) = ρ`.
    pub fn row(&mut self, r: usize, c: Rgb8, div: u32, dim: bool) {
        let (g, f) = self.fans[r / 5];
        let rho = r % 5;
        self.put(g, 8 * f + rho, c, div, dim);
        self.put(g, 8 * f + (8 - rho) % 8, c, div, dim);
    }

    /// IP position `4k + j`: ring LEDs j and 7 − j.
    pub fn pair(&mut self, p: usize, c: Rgb8, div: u32, dim: bool) {
        let (g, f) = self.fans[p / 4];
        let j = p % 4;
        self.put(g, 8 * f + j, c, div, dim);
        self.put(g, 8 * f + 7 - j, c, div, dim);
    }
}

/// `end(x, L) = min(x, L − 1 − x)`: 0 at both ends of a strip.
pub(crate) fn end(x: i64, len: i64) -> i64 {
    x.min(len - 1 - x)
}

// ------------------------------------------------------------------- strip shapes and motion

/// Shift trail: the LED `d` behind the head shows `c >> table[d]`, black outside the table.
pub(crate) fn trail(c: Rgb8, d: i64, table: &[u32]) -> Rgb8 {
    match usize::try_from(d).ok().and_then(|d| table.get(d)) {
        Some(&s) => shr(c, s),
        None => BLACK,
    }
}

/// Trail of the inner-ring meteors (Meteor, MeteorRainbow; ring.md).
pub(crate) const METEOR_INNER_TRAIL: [u32; 9] = [0, 1, 2, 3, 4, 5, 6, 6, 6];

/// Two dots at ring positions `a` and `b`; on the same LED they add with 8-bit wrap-around
/// (ring.md §3.3).
pub(crate) fn two_dots(x: i64, a: i64, ca: Rgb8, b: i64, cb: Rgb8) -> Rgb8 {
    let mut v = BLACK;
    if x == a {
        v = add_wrap(v, ca);
    }
    if x == b {
        v = add_wrap(v, cb);
    }
    v
}

/// Tide mask: positions `x < k` or `x > len − 1 − k` are lit (fill from both ends).
pub(crate) fn tide_mask(k: i64, len: i64, x: i64) -> bool {
    x < k || x > len - 1 - k
}

/// Ping-pong window `(L, w, half, Lv)` (ring.md §3.4) at cycle tick `m`:
/// returns the distance of position `x` behind the head (0 = head), or `None` if dark.
pub(crate) fn pingpong(m: i64, len: i64, w: i64, half: i64, lv: i64, x: i64) -> Option<i64> {
    let extra = half - 1 - lv;
    let fwd = m < half;
    let k = if fwd { m } else { m - half + 1 };
    let head = (k * (len + extra)).div_euclid(lv + extra);
    let xf = if fwd { x } else { len - 1 - x };
    let d = head - xf;
    ((0..w).contains(&d) && (fwd || head >= 1)).then_some(d)
}

/// Bouncing segment clock of the whole-fan Runway/MopUp/Scan in steps of the outer strip
/// (length `no`): returns (forward pass?, head, pass number).
pub(crate) fn bounce(t: f64, period: f64, t0: f64, no: i64, w: i64, g: i64) -> (bool, i64, i64) {
    let pp = no + w - 1 + g;
    let total = steps(t, period, t0);
    let p = total % (2 * pp);
    if p < pp { (true, p, total / pp) } else { (false, p - pp + 1, total / pp) }
}

/// LED `x` of a strip of `len` lit by a `w`-long segment with head `h` (forward: 0 → len−1).
pub(crate) fn seg_lit(fwd: bool, h: i64, len: i64, w: i64, x: i64) -> bool {
    let pos = if fwd { x } else { len - 1 - x };
    pos <= h && pos > h - w
}

/// Merged ping-pong segment of `seg` LEDs on a strip of `len`, slave of the outer master strip
/// `lo`, with `gap` dark ticks at each turn. Returns (backward pass?, first lit, last lit).
pub(crate) fn bounce_segment(u: i64, len: i64, lo: i64, seg: i64, gap: i64) -> (bool, i64, i64) {
    let ho = lo + seg - 1 + gap;
    let h = len + seg - 1 + gap;
    let c = u.rem_euclid(2 * ho) * (h - 1) / (ho - 1);
    if c < h {
        (false, c - seg + 1, c)
    } else {
        let m = c - h + 1;
        (true, len - 1 - m, len - 2 + seg - m)
    }
}

/// Stacking sequence state `s` on a strip of `len` positions (ring.md §3.4, whole.md, merge.md):
/// (pile height = dots already stacked, falling dot position).
pub(crate) fn stack_state(mut s: i64, len: i64) -> (i64, i64) {
    let mut h = 0;
    while h < len && s >= len - h {
        s -= len - h;
        h += 1;
    }
    (h, s)
}

// ------------------------------------------------------------------------- shared effect cores

/// Breathing / BreathingRainbow on the whole fan: step `n` and the breathing envelope,
/// 192 steps: 64 down (255 … 3), 32 off, 64 up (3 … 255), 32 full.
pub(crate) fn whole_breath(fx: &Fx, t: f64) -> (i64, u32) {
    let n = steps(t, 10.78 + 9.8 * fx.d(), 10.0);
    let m = n % 192;
    let l = match m {
        0..64 => 255 - 4 * m,
        64..96 => 0,
        96..160 => 3 + 4 * (m - 96),
        _ => 255,
    };
    (n, l as u32)
}

/// Breathing / BreathingRainbow on a ring: step `n` and the weight `(level + 1) >> s` of the
/// 188-tick breath envelope (ring.md §3.2).
pub(crate) fn ring_breath(fx: &Fx, t: f64) -> (i64, u32) {
    let n = steps(t, period(fx, 51, 10), RING_T0);
    let m = n.rem_euclid(188);
    let l = match m {
        0 => 255,
        1..=63 => 255 - 4 * m,
        64..=94 => 0,
        95..=157 => 3 + 4 * (m - 95),
        _ => 255,
    } as u32;
    (n, (l + 1) >> fx.shift)
}

/// Runway / MopUp on the whole fan: a 5-LED block bouncing along the stacked outer strip and the
/// inner LED strip. `colours(pass)` gives (block, background).
pub(crate) fn whole_runway(fx: &Fx, cv: &mut Canvas, t: f64, colours: impl Fn(i64) -> (Rgb8, Rgb8)) {
    let qi = cv.q as i64;
    let no = 12 * qi;
    let (w, g) = (5, 3);
    let (fwd, h, pass) = bounce(t, 24.0 + 7.0 * fx.d(), 5.0, no, w, g);
    let ni = 8 * qi;
    // inner block moves slower so both strips reach the end together
    let hi = (h as f64 * (ni + w) as f64 / (no + w) as f64 + 0.2).floor() as i64;
    let (fg, bg) = colours(pass);
    for x in 0..no {
        cv.outer_j(x as usize, if seg_lit(fwd, h, no, w, x) { fg } else { bg });
    }
    for x in 0..ni {
        cv.inner_i(x as usize, if seg_lit(fwd, hi, ni, w, x) { fg } else { bg });
    }
}

/// Runway / MopUp merged: a 5-LED segment bouncing along the I8 and the O strip.
/// `colours(cycle, backward pass?)` gives (segment, background); `dim_bg` = the background takes
/// the minus-one brightness path.
pub(crate) fn merge_runway(
    fx: &Fx,
    st: &mut Strips,
    tau: f64,
    colours: impl Fn(i64, bool) -> (Rgb8, Rgb8),
    dim_bg: bool,
) {
    let ni = st.n as i64;
    let lo = 12 * ni;
    let u = steps(tau, (37 - 7 * fx.speed) as f64, 0.0);
    let cycle = u / (2 * (lo + 7));
    for (outer, len) in [(false, 8 * ni), (true, lo)] {
        let (back, a, b) = bounce_segment(u, len, lo, 5, 3);
        let (fg, bg) = colours(cycle, back);
        for x in 0..len {
            let lit = (a..=b).contains(&x);
            let mut col = if lit { fg } else { bg };
            if outer && x == 79 {
                col = BLACK; // observed quirk: outer strip position 79 stays black
            }
            let dim = !lit && dim_bg;
            if outer {
                st.outer(x as usize, col, 1, dim);
            } else {
                st.inner8(x as usize, col, 1, dim);
            }
        }
    }
}

/// FwStrobe lap: which LED flashes at tick `n` on a strip of `n_leds` (lap `3N + 8` ticks).
/// Ticks 0 and 2 → LED 0 (double flash), tick `5 + 3k` → LED `k + 1`, otherwise none.
fn strobe_led(n: i64, n_leds: i64) -> Option<i64> {
    let m = n.rem_euclid(3 * n_leds + 8);
    if m == 0 || m == 2 {
        return Some(0);
    }
    if m >= 5 && (m - 5) % 3 == 0 && (m - 5) / 3 <= n_leds - 2 {
        return Some((m - 5) / 3 + 1);
    }
    None
}

/// FwStrobe / FwStrobeRainbow on the whole fan: the strobe lap on every inner ring and outer
/// frame, the other LEDs at 1/16. `colour(n, len)` gives the colour on a strip of `len` LEDs.
pub(crate) fn whole_strobe(fx: &Fx, cv: &mut Canvas, t: f64, colour: impl Fn(i64, i64) -> Rgb8) {
    let rev = fx.reverse;
    let n = steps(t, 8.0 + 7.0 * fx.d(), 5.5) + 2;
    for (len, outer) in [(8i64, false), (12, true)] {
        let led = strobe_led(n, len).map(|l| if rev { len - 1 - l } else { l });
        let c = colour(n, len);
        for f in 0..4 {
            for x in 0..len {
                let v = if led == Some(x) { c } else { shr(c, 4) };
                if outer { cv.outer(f, x as usize, v) } else { cv.inner(f, x as usize, v) }
            }
        }
    }
}

/// FwStrobe / FwStrobeRainbow on a ring; `colour(n, len)` as for [`whole_strobe`].
pub(crate) fn ring_strobe(fx: &Fx, r: &mut Ring, t: f64, colour: impl Fn(i64, i64) -> Rgb8) {
    let (s, nl) = (fx.shift, r.n as i64);
    let p = 8.0 + 7.0 * fx.d();
    let n = steps(t, p, RING_T0) + 2;
    let led = strobe_led(n, nl).map(|l| if fx.reverse { nl - 1 - l } else { l });
    let c = colour(n, nl);
    let (hit, dim) = (shr(c, s), shr(c, 4 + s));
    r.paint(|_, x| if led == Some(x as i64) { hit } else { dim });
}

/// FwRunner / FwConverge background on a ring: `rainbow12(2r)` at quarter level.
pub(crate) fn ring_rainbow_bg(rr: i64, s: u32) -> Rgb8 {
    shr(shr(rainbow12(2 * rr), 2), s)
}

// ------------------------------------------------------------------ whole-fan strip and spill

/// SINE25 = round(127·sin(3.6°·k)), the rising quarter of the conveyor's P75 bump.
const SINE25: [u8; 25] = [
    0, 8, 16, 24, 31, 39, 47, 54, 61, 68, 75, 81, 87, 93, 98, 103, 107, 111, 115, 118, 121, 123, 125,
    126, 127,
];

/// Conveyor channel values at strip position `s` for the per-channel phases `phase`:
/// P75 bump = SINE25 up, SINE25 down, 25 steps of zero.
pub(crate) fn p75_bump(phase: [i64; 3], s: usize) -> [i16; 3] {
    let s = s as i64;
    phase.map(|ph| {
        let u = (s - ph).rem_euclid(75) as usize;
        i16::from(match u {
            0..25 => SINE25[u],
            25..50 => SINE25[49 - u],
            _ => 0,
        })
    })
}

/// Position `s` on the whole-fan strip (G4) of LED `x` of fan `f` in ring `outer`:
/// per fan 8 inner LEDs, then 12 outer wires.
fn strip_pos(f: usize, outer: bool, x: usize) -> usize {
    20 * f + if outer { 8 + x } else { x }
}

/// A whole-fan strip effect (FwConveyor, FwConveyorWipe, FwPastel; ring.md §4.7–4.10) at one
/// moment. It draws its own ring and also overwrites part of the other ring ("spill").
pub(crate) struct WholeFanStrip {
    /// Strip length `L`: 32 for an Inner effect, 48 for an Outer one.
    pub len: i64,
    /// Step counter.
    pub n: i64,
    pub reverse: bool,
    /// Last strip position written.
    last: usize,
    /// Full-brightness channel values at position `s`; −1 = channel not written.
    value: fn(&WholeFanStrip, usize) -> [i16; 3],
}

impl WholeFanStrip {
    /// Strip of an Inner (`outer` = false) or Outer effect at step `n`; `extra` = positions
    /// written beyond the strip end (Pastel writes one more than the conveyors).
    pub fn new(fx: &Fx, outer: bool, n: i64, extra: usize, value: fn(&Self, usize) -> [i16; 3]) -> Self {
        let len = if outer { 48 } else { 32 };
        Self { len, n, reverse: fx.reverse, last: (len - 1) as usize + extra, value }
    }

    fn value(&self, s: usize) -> [i16; 3] {
        (self.value)(self, s)
    }

    /// Draws the effect's own ring.
    pub fn paint(&self, r: &mut Ring, s: u32) {
        let outer = r.outer;
        r.paint(|f, x| {
            let pos = strip_pos(f, outer, x);
            if pos <= self.last { shr(self.value(pos).map(|v| v.max(0) as u8), s) } else { BLACK }
        });
    }

    /// Spill into the other ring (ring.md §4.9), drawn after both rings have been rendered.
    /// An Inner effect spills from step n = 1 on; an Outer effect from the start.
    pub fn spill(&self, outer: bool, q: usize, shift: u32, leds: &mut GroupLeds) {
        if !outer && self.n < 1 {
            return;
        }
        for s in 0..=self.last {
            let (f, r) = (s / 20, s % 20);
            let other_is_outer = r >= 8;
            if f >= q || other_is_outer == outer {
                continue; // missing fan, or the effect's own ring
            }
            let led = if other_is_outer { OUTER_BASE + 12 * f + (r - 8) } else { 8 * f + r };
            for (ch, v) in self.value(s).into_iter().enumerate() {
                if v >= 0 {
                    leds[led][ch] = (v as u8) >> shift.min(7);
                }
            }
        }
    }
}
