use core::cmp::PartialOrd;
use core::ops::{Add, AddAssign, Div, Mul, Sub};

use fixed::traits::ToFixed;

pub trait Real:
    Copy
    + Add<Output = Self>
    + AddAssign
    + Sub<Output = Self>
    + Mul<Output = Self>
    + Div<Output = Self>
    + PartialOrd
{
    fn from_f32(v: f32) -> Self;
    fn from_u64(v: u64) -> Self;
    fn zero() -> Self;
    fn one() -> Self;
    fn tau() -> Self;
    fn abs(self) -> Self;
    fn signum(self) -> Self;
}

impl Real for f32 {
    fn from_f32(v: f32) -> Self { v }

    fn from_u64(v: u64) -> Self { v as f32 }

    fn zero() -> Self { 0.0 }

    fn one() -> Self { 1.0 }

    fn tau() -> Self { core::f32::consts::TAU }

    fn abs(self) -> Self { f32::abs(self) }

    fn signum(self) -> Self { f32::signum(self) }
}

impl Real for f64 {
    fn from_f32(v: f32) -> Self { v as f64 }

    fn from_u64(v: u64) -> Self { v as f64 }

    fn zero() -> Self { 0.0 }

    fn one() -> Self { 1.0 }

    fn tau() -> Self { core::f64::consts::TAU }

    fn abs(self) -> Self { f64::abs(self) }

    fn signum(self) -> Self { f64::signum(self) }
}

macro_rules! impl_real_for_fixed {
    ($($t:ty),+ $(,)?) => {
        $(
            impl Real for $t {
                fn from_f32(v: f32) -> Self {
                    v.to_fixed()
                }

                fn from_u64(v: u64) -> Self {
                    v.to_fixed()
                }

                fn zero() -> Self {
                    Self::from_num(0)
                }

                fn one() -> Self {
                    Self::from_num(1)
                }

                fn tau() -> Self {
                    Self::from_num(core::f64::consts::TAU)
                }

                fn abs(self) -> Self {
                    <$t>::abs(self)
                }

                fn signum(self) -> Self {
                    <$t>::signum(self)
                }
            }
        )+
    };
}

use fixed::types::{
    I8F8, I12F4,
    I8F24, I12F20, I16F16, I24F8, I28F4,
    I8F56, I12F52, I16F48, I20F44, I24F40, I28F36, I32F32,
    I36F28, I40F24, I44F20, I48F16, I52F12, I56F8, I60F4,
};
impl_real_for_fixed!(
    // 16-bit
    I8F8, I12F4,
    // 32-bit
    I8F24, I12F20, I16F16, I24F8, I28F4,
    // 64-bit
    I8F56, I12F52, I16F48, I20F44, I24F40, I28F36, I32F32,
    I36F28, I40F24, I44F20, I48F16, I52F12, I56F8, I60F4,
);
