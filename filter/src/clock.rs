pub trait Duration {
    fn as_millis(&self) -> u64;
}

pub trait Clock {
    type Mark;
    type Duration: Duration;
    
    fn now(&self) -> Self::Mark;

    fn duration_since(&self, from: Self::Mark) -> Self::Duration;
}

pub struct FakeMark(u64);

pub struct FakeDuration(u64);

impl Duration for FakeDuration {
    fn as_millis(&self) -> u64 {
        self.0
    }
}

pub struct FakeClock {
    current: u64,
}

impl FakeClock {
    pub fn new() -> Self {
        Self {
            current: 0,
        }
    }

    pub fn advance_millis(&mut self, advance: u64) {
        self.current += advance;
    }
}

impl Clock for FakeClock {
    type Mark = FakeMark;
    type Duration = FakeDuration;

    fn now(&self) -> Self::Mark {
        FakeMark(self.current)
    }

    fn duration_since(&self, from: Self::Mark) -> Self::Duration {
        FakeDuration(self.current - from.0)
    }
}

cfg_select! {
    feature = "std" => {
        pub struct StdMark(std::time::Duration);

        pub struct StdDuration(std::time::Duration);

        impl Duration for StdDuration {
            fn as_millis(&self) -> u64 {
                self.0.as_millis() as u64
            }
        }

        pub struct StdClock {
            start: std::time::Instant,
        }

        impl StdClock {
            fn new() -> Self {
                Self { start: std::time::Instant::now() }
            }
        }
        
        impl Clock for StdClock {
            type Mark = StdMark;
            type Duration = StdDuration;

            fn now(&self) -> Self::Mark {
                StdMark(self.start.elapsed())
            }

            fn duration_since(&self, from: Self::Mark) -> Self::Duration {
                StdDuration(
                    std::time::Instant::now().duration_since(self.start) - from.0
                )
            }
        }
    }
    feature = "embassy" => {

    }
    _ => {}
}

