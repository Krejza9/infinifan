//! FwStrobe: a double flash at LED 0 followed by single flashes running around the ring, in
//! colour 1 over the same colour at 1/16.

use crate::fxsim::Fx;
use crate::fxsim::blocks::{Canvas, Ring, ring_strobe, whole_strobe};

pub(crate) fn whole(fx: &Fx, cv: &mut Canvas, t: f64) {
    whole_strobe(fx, cv, t, |_, _| fx.pal(0));
}

pub(crate) fn ring(fx: &Fx, r: &mut Ring, t: f64) {
    ring_strobe(fx, r, t, |_, _| fx.pal(0));
}
