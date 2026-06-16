use embassy_rp::pio::{Common, Config, Direction, Instance, LoadedProgram, Pin, PioPin, ShiftDirection, StateMachine};
use embassy_rp::Peri;
use embassy_time::Instant;
use fixed::FixedU32;
use fixed::types::extra::U8;

pub struct PwmInputProgram<'a, PIO: Instance> {
    prg: LoadedProgram<'a, PIO>,
}

impl<'a, PIO: Instance> PwmInputProgram<'a, PIO> {
    pub fn new(common: &mut Common<'a, PIO>) -> Self {
        let prg = pio::pio_asm!(
            ".wrap_target",

            "wait 0 pin 0",       // sync: wait for pin to go low
            "wait 1 pin 0",       // wait for rising edge
            "mov x, ~null",       // x = 0xFFFFFFFF

            "counting:",
            "jmp x-- check_pin",  // decrement x (always)
            "check_pin:",
            "jmp pin counting",   // keep counting while pin is high

            "mov isr, x",
            "push noblock",       // pulse_us = 0xFFFFFFFF - x

            ".wrap"
        );
        Self { prg: common.load_program(&prg.program) }
    }
}

pub struct PwmInput<'a, PIO: Instance, const SM: usize> {
    sm: StateMachine<'a, PIO, SM>,
    _pin: Pin<'a, PIO>,
    last_pulse: Option<(u16, Instant)>,
}

impl<'a, PIO: Instance, const SM: usize> PwmInput<'a, PIO, SM> {
    pub fn new(
        common: &mut Common<'a, PIO>,
        mut sm: StateMachine<'a, PIO, SM>,
        pin: Peri<'a, impl PioPin>,
        program: &PwmInputProgram<'a, PIO>,
    ) -> Self {
        let pin = common.make_pio_pin(pin);
        sm.set_pin_dirs(Direction::In, &[&pin]);

        let mut cfg = Config::default();
        cfg.use_program(&program.prg, &[]);
        // 2 MHz PIO clock → 1 µs per 2-instruction loop iteration (125 MHz / 62.5)
        cfg.clock_divider = FixedU32::<U8>::from_num(62.5_f32);
        cfg.set_in_pins(&[&pin]);
        cfg.set_jmp_pin(&pin);
        cfg.shift_in.direction = ShiftDirection::Left;
        cfg.shift_in.auto_fill = false;

        sm.set_config(&cfg);
        sm.set_enable(true);

        Self { sm, _pin: pin, last_pulse: None }
    }

    /// Drains the FIFO and returns `(pulse_us, received_at)` for the most
    /// recent pulse, or `None` if no pulse has ever been received.
    /// Timeout and range checks are left to the caller.
    pub fn current_value(&mut self) -> Option<(u16, Instant)> {
        while let Some(raw) = self.sm.rx().try_pull() {
            self.last_pulse = Some(((0xFFFF_FFFFu32 - raw) as u16, Instant::now()));
        }
        self.last_pulse
    }
}
