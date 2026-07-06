
#![no_std]

#[cfg(feature = "std")]
extern crate std;

mod pt2;
pub use pt2::Pt2Filter;
mod ramp;
pub use ramp::{LinearRamp, LinearRampConfig};
mod real;
