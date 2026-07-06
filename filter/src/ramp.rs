use core::cmp::Ordering;

use crate::real::Real;

#[derive(Clone, Copy)]
pub struct LinearRampConfig<T: Real> {
    pub span: T,
    pub ramp_up_millis: u64,
    pub ramp_down_millis: u64,
    pub first_interval_millis: u64,
}

#[derive(Clone)]
pub struct LinearRamp<T: Real> {
    ramp_up_millis: u64,
    ramp_down_millis: u64,
    first_interval_millis: u64,
    span: T,
    current: T,
    last_updated_at: Option<u64>,
}

impl<T: Real> LinearRamp<T> {
    pub fn new(config: LinearRampConfig<T>, current: T) -> Self {
        Self {
            ramp_up_millis: config.ramp_up_millis,
            ramp_down_millis: config.ramp_down_millis,
            first_interval_millis: config.first_interval_millis,
            span: config.span,
            current,
            last_updated_at: None,
        }
    }

    pub fn value(&self) -> T {
        self.current
    }
    
    pub fn update(&mut self, target: T, dt_millis: u64) -> T {
        let delta = target - self.current;

        let ramp_millis = match delta.partial_cmp(&T::zero()) {
            Some(Ordering::Equal) | None => {
                return self.current;
            }
            Some(Ordering::Greater) => {
                self.ramp_up_millis
            }
            Some(Ordering::Less) => {
                self.ramp_down_millis
            }
        };

        let period_from_last_update = self.last_updated_at
            .map(|v| if dt_millis >= v { dt_millis - v } else { 0 })
            .unwrap_or(self.first_interval_millis);
        let max_step = self.span * T::from_u64(period_from_last_update) / T::from_u64(ramp_millis);
        
        if delta.abs() <= max_step {
            self.current = target;
        } else {
            self.current += max_step * delta.signum();
        }
        self.last_updated_at = Some(dt_millis);
        self.current
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_linear_ramp() {
        let mut ramp = LinearRamp::new(
            LinearRampConfig {
                span: 1024.0,
                ramp_up_millis: 1000,
                ramp_down_millis: 1000,
                first_interval_millis: 20,
            },
            0.0
        );

        assert_eq!(ramp.update(512.0, 100), 20.48);
        assert_eq!(ramp.update(512.0, 100), 20.48);
        assert_eq!(ramp.update(512.0, 120), 40.96);
        assert_eq!(ramp.update(512.0, 200), 122.88);
        assert_eq!(ramp.update(1024.0, 1000), 942.08);
        assert_eq!(ramp.update(1024.0, 1100), 1024.0);
        assert_eq!(ramp.update(512.0, 1100), 1024.0);
        assert_eq!(ramp.update(512.0, 1120), 1003.52);
    }
}
