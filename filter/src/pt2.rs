use crate::real::Real;

// PTn cutoff correction = 1 / sqrt(2^(1/n) - 1)
const CUTOFF_CORRECTION_PT2: f32 = 1.553773974;

fn pt1_filter_gain<T: Real>(f_cut: T, f_sample: T) -> T {
    let omega = T::tau() * f_cut / f_sample;
    omega / (omega + T::one())
}

fn pt2_filter_gain<T: Real>(f_cut: T, f_sample: T) -> T {
    pt1_filter_gain(f_cut * T::from_f32(CUTOFF_CORRECTION_PT2), f_sample)
}

#[derive(Clone, Copy)]
pub struct Pt2Filter<T: Real> {
    state: T,
    state1: T,
    k: T,
    initial_state: T,
}

impl<T: Real> Pt2Filter<T> {
    pub fn new(f_cut: T, f_sample: T) -> Pt2Filter<T> {
        Self::with_initial_state(f_cut, f_sample, T::zero())
    }

    pub fn with_initial_state(f_cut: T, f_sample: T, initial_state: T) -> Pt2Filter<T> {
        let k = pt2_filter_gain(f_cut, f_sample);
        Self {
            state: initial_state,
            state1: initial_state,
            k,
            initial_state,
        }
    }

    pub fn update_gain(&mut self, f_cut: T, f_sample: T) {
        self.k = pt2_filter_gain(f_cut, f_sample);
    }

    pub fn reset(&mut self) {
        self.state = self.initial_state;
        self.state1 = self.initial_state;
    }

    pub fn update(&mut self, input: T) -> T {
        self.state1 += self.k * (input - self.state1);
        self.state += self.k * (self.state1 - self.state);
        self.state
    }

    pub fn state(&self) -> T {
        self.state
    }
}

#[cfg(test)]
mod tests {
    use fixed::traits::ToFixed;

    use super::*;

    #[test]
    fn test_pt2_filter_f32() {
        let mut f: Pt2Filter<f32> = Pt2Filter::with_initial_state(
            5.0, 50.0, 988.0
        );
        assert_eq!(f.update(1500.0), 1112.9438);
        assert_eq!(f.update(1500.0), 1239.3883);
        assert_eq!(f.update(1500.0), 1335.3606);
        assert_eq!(f.update(1500.0), 1400.1106);
        assert_eq!(f.update(1500.0), 1441.0654);
        assert_eq!(f.update(1500.0), 1465.9335);
        assert_eq!(f.update(1500.0), 1480.614);
    }

    #[test]
    fn test_pt2_filter_f64() {
        let mut f: Pt2Filter<f64> = Pt2Filter::with_initial_state(
            5.0, 50.0, 988.0
        );
        assert_eq!(f.update(1500.0), 1112.943894907884);
        assert_eq!(f.update(1500.0), 1239.388369859081);
        assert_eq!(f.update(1500.0), 1335.3606776145216);
        assert_eq!(f.update(1500.0), 1400.110636375015);
        assert_eq!(f.update(1500.0), 1441.0653912302762);
        assert_eq!(f.update(1500.0), 1465.933364867146);
        assert_eq!(f.update(1500.0), 1480.6139041494157);
    }

    #[test]
    fn test_pt2_filter_i12f4() {
        let mut f: Pt2Filter<fixed::types::I12F4> = Pt2Filter::with_initial_state(
            5.to_fixed(), 50.to_fixed(), 988.to_fixed()
        );
        assert_eq!(f.update(1500.to_fixed()).to_num::<u16>(), 1086);
        assert_eq!(f.update(1500.to_fixed()).to_num::<u16>(), 1196);
        assert_eq!(f.update(1500.to_fixed()).to_num::<u16>(), 1289);
        assert_eq!(f.update(1500.to_fixed()).to_num::<u16>(), 1359);
        assert_eq!(f.update(1500.to_fixed()).to_num::<u16>(), 1408);
        assert_eq!(f.update(1500.to_fixed()).to_num::<u16>(), 1441);
        assert_eq!(f.update(1500.to_fixed()).to_num::<u16>(), 1462);
    }

    #[test]
    fn test_pt2_filter_i16f16() {
        let mut f: Pt2Filter<fixed::types::I16F16> = Pt2Filter::with_initial_state(
            5.to_fixed(), 50.to_fixed(), 988.to_fixed()
        );
        assert_eq!(f.update(1500.to_fixed()).to_num::<u16>(), 1112);
        assert_eq!(f.update(1500.to_fixed()).to_num::<u16>(), 1239);
        assert_eq!(f.update(1500.to_fixed()).to_num::<u16>(), 1335);
        assert_eq!(f.update(1500.to_fixed()).to_num::<u16>(), 1400);
        assert_eq!(f.update(1500.to_fixed()).to_num::<u16>(), 1441);
        assert_eq!(f.update(1500.to_fixed()).to_num::<u16>(), 1465);
        assert_eq!(f.update(1500.to_fixed()).to_num::<u16>(), 1480);
    }
}
