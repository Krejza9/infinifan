//! One module per effect, named after the base of its mode id (`HeartBeatRunway_Inner` →
//! [`heart_beat_runway`]). Each holds all variants of the effect:
//!
//! * `whole(fx, canvas, t)` – scope Whole (`whole.md`);
//! * `ring(fx, ring, t)` – scopes Inner and Outer (`ring.md`), or `inner` / `outer` for an
//!   effect that exists on one ring only;
//! * `merge(fx, strips, τ)` – scope Merge (`merge.md`).

pub(crate) mod breathing;
pub(crate) mod breathing_rainbow;
pub(crate) mod color_cycle;
pub(crate) mod colorful_meteor;
pub(crate) mod disco;
pub(crate) mod door;
pub(crate) mod double_arc;
pub(crate) mod double_meteor;
pub(crate) mod electric_current;
pub(crate) mod fw_converge;
pub(crate) mod fw_conveyor;
pub(crate) mod fw_conveyor_wipe;
pub(crate) mod fw_pastel;
pub(crate) mod fw_runner;
pub(crate) mod fw_sparkle;
pub(crate) mod fw_strobe;
pub(crate) mod fw_strobe_rainbow;
pub(crate) mod fw_twinkle;
pub(crate) mod fw_wave;
pub(crate) mod heart_beat;
pub(crate) mod heart_beat_runway;
pub(crate) mod lottery;
pub(crate) mod meteor;
pub(crate) mod meteor_contest;
pub(crate) mod meteor_mix;
pub(crate) mod meteor_rainbow;
pub(crate) mod mixing;
pub(crate) mod mop_up;
pub(crate) mod rainbow;
pub(crate) mod rainbow_morph;
pub(crate) mod reflect;
pub(crate) mod return_arc;
pub(crate) mod runway;
pub(crate) mod scan;
pub(crate) mod stack;
pub(crate) mod static_color;
pub(crate) mod taichi;
pub(crate) mod tide;
pub(crate) mod voice;
pub(crate) mod warning;
