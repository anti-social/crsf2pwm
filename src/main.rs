#![no_std]
#![no_main]

use embassy_executor::Spawner;
use embassy_rp::dma;
use embassy_rp::peripherals::{DMA_CH0, DMA_CH1, DMA_CH2, PIO0, UART0};
use embassy_rp::pio::{self, Pio};
use embassy_rp::pio_programs::ws2812::{Grb, PioWs2812, PioWs2812Program};
use embassy_rp::pwm::{Config, Pwm, PwmBatch, PwmOutput, SetDutyCycle};
use embassy_rp::uart::{self, Uart, UartRx};
use embassy_rp::watchdog::Watchdog;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::watch::Watch;
use embassy_time::{Duration, Instant, Timer, with_timeout};
use fixed::traits::ToFixed;
use {defmt_rtt as _, panic_probe as _};
use smart_leds::{brightness, colors, gamma, RGB8};

use pt_filter::Pt2Filter;

mod pwm_input;
use pwm_input::{PwmInput, PwmInputProgram};

const CRSF_RX_BAUDRATE: u32 = 420_000;
const CRSF_RESET_TIMEOUT: Duration = Duration::from_millis(10);
const NUM_PWM_CHANNELS: usize = 8;
const RX_LOSS_TIMEOUT: Duration = Duration::from_millis(100);
const EXT_PWM_SIGNAL_TIMEOUT: Duration = Duration::from_millis(50);
const EXT_PWM_POLL_INTERVAL: Duration = Duration::from_millis(20);
const LED_BRIGHTNESS: u8 = 64;

static LAST_RC_PACKET: Watch<CriticalSectionRawMutex, (crsf::RcChannelsPacked, Instant), 1> = Watch::new();
static CRSF_STATUS: Watch<CriticalSectionRawMutex, Status, 1> = Watch::new();
static EXT_PWM_STATUS: Watch<CriticalSectionRawMutex, bool, 1> = Watch::new();

const PWM_MIN_VALUE: u16 = 988;
const PWM_MAX_VALUE: u16 = 2012;
const PWM_MID_VALUE: u16 = 1500;
// TODO: Consider remembering values from the first crsf packet
const PWM_FAILSAFE_VALUES: [u16; NUM_PWM_CHANNELS] = [
    PWM_MID_VALUE,
    PWM_MID_VALUE,
    PWM_MIN_VALUE,
    PWM_MIN_VALUE,
    PWM_MIN_VALUE,
    PWM_MIN_VALUE,
    PWM_MIN_VALUE,
    PWM_MIN_VALUE,
];
const RC_TO_PWM_SCALE_FACTOR: u32 = (PWM_MAX_VALUE - PWM_MIN_VALUE) as u32 * 1_000_000 /
    (crsf::RcChannelsPacked::CHANNEL_VALUE_MAX as u32 - crsf::RcChannelsPacked::CHANNEL_VALUE_MIN as u32);
const RC_TO_PWM_OFFSET: u16 = 881;

const EXT_PWM_MIN_CENTRAL_VALUE: u16 = 1490;
const EXT_PWM_MAX_CENTRAL_VALUE: u16 = 1510;

const FILTER_SAMPLE_MIN_FREQ: u16 = 25;
const FILTER_SAMPLE_MAX_FREQ: u16 = 250;
const FILTER_SAMPLE_DEFAULT_FREQ: u16 = 50;
const FILTER_CUT_FREQ: u16 = 5;

fn is_ext_pwm_active(reading: &(u16, Instant)) -> bool {
    let (v, t) = reading;
    t.elapsed() < EXT_PWM_SIGNAL_TIMEOUT
        && !(EXT_PWM_MIN_CENTRAL_VALUE..=EXT_PWM_MAX_CENTRAL_VALUE).contains(v)
}

fn resolve_ext_pwm(reading: &Option<(u16, Instant)>, fallback: u16) -> u16 {
    reading
        .filter(is_ext_pwm_active)
        .map(|(v, _)| v)
        .unwrap_or(fallback)
}

fn map_rc_channel_to_pwm(rc_value: u16) -> u16 {
    // conversion from RC value to PWM
    // for 0x16 RC frame
    //       RC     PWM
    // min  172 ->  988us
    // mid  992 -> 1500us
    // max 1811 -> 2012us
    // scale factor = (2012-988) / (1811-172) = 0.62477120195241
    // offset = 988 - 172 * 0.62477120195241 = 880.53935326418548

    ((RC_TO_PWM_SCALE_FACTOR * (rc_value as u32)) / 1_000_000) as u16 + RC_TO_PWM_OFFSET
}

type Ws2812 = PioWs2812<'static, PIO0, 0, 1, Grb>;

type FilteredPwmValue = fixed::FixedI32<fixed::types::extra::U16>;

#[derive(Clone, PartialEq)]
enum Status {
    Connected,
    RxLoss,
}

embassy_rp::bind_interrupts!(struct Irqs {
    PIO0_IRQ_0 => pio::InterruptHandler<PIO0>;
    UART0_IRQ => uart::InterruptHandler<UART0>;
    // UART1_IRQ => uart::InterruptHandler<UART1>;
    DMA_IRQ_0 => dma::InterruptHandler<DMA_CH0>, dma::InterruptHandler<DMA_CH1>, dma::InterruptHandler<DMA_CH2>;
});

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let p = embassy_rp::init(Default::default());

    defmt::info!("Starting crsf to pwm converter...");

    let mut watchdog = Watchdog::new(p.WATCHDOG);
    watchdog.start(Duration::from_millis(1000));

    let Pio { mut common, sm0, sm1, sm2, .. } = Pio::new(p.PIO0, Irqs);
    let program = PioWs2812Program::new(&mut common);
    let mut ws2812: Ws2812 = PioWs2812::with_color_order(
        &mut common, sm0, p.DMA_CH2, Irqs, p.PIN_16, &program
    );
    set_led_color(&mut ws2812, colors::BLUE).await;

    let pwm_input_program = PwmInputProgram::new(&mut common);
    let ext_pwm_0 = PwmInput::new(&mut common, sm1, p.PIN_14, &pwm_input_program);
    let ext_pwm_1 = PwmInput::new(&mut common, sm2, p.PIN_15, &pwm_input_program);

    let crsf_uart_config = {
        let mut config = uart::Config::default();
        config.baudrate = CRSF_RX_BAUDRATE;
        config
    };
    let input_uart = Uart::new(
        p.UART0,
        p.PIN_12,
        p.PIN_13,
        Irqs,
        p.DMA_CH0,
        p.DMA_CH1,
        crsf_uart_config,
    );
    let (mut _in_uart_tx, in_uart_rx) = input_uart.split();

    // let output_uart = Uart::new(
    //     p.UART1,
    //     p.PIN_8,
    //     p.PIN_9,
    //     Irqs,
    //     p.DMA_CH3,
    //     p.DMA_CH4,
    //     crsf_uart_config,
    // );
    // let (mut out_uart_tx, out_uart_rx) = output_uart.split();

    let mut pwm_config = Config::default();
    // 125 MHz / 125 = 1 MHz (1 us per tick)
    pwm_config.divider = 125.into();
    // 20_000 us = 20 ms (pwm period)
    pwm_config.top = 20_000;
    pwm_config.compare_a = 0;
    pwm_config.compare_b = 0;
    // Disable PWMs, we will start them in a batch
    pwm_config.enable = false;

    let pwm_slice_0 = Pwm::new_output_ab(
        p.PWM_SLICE0, p.PIN_0, p.PIN_1, pwm_config.clone()
    );
    let pwm_slice_1 = Pwm::new_output_ab(
        p.PWM_SLICE1, p.PIN_2, p.PIN_3, pwm_config.clone()
    );
    let pwm_slice_2 = Pwm::new_output_ab(
        p.PWM_SLICE2, p.PIN_4, p.PIN_5, pwm_config.clone()
    );
    let pwm_slice_3 = Pwm::new_output_ab(
        p.PWM_SLICE3, p.PIN_6, p.PIN_7, pwm_config.clone()
    );
    PwmBatch::set_enabled(true, |batch| {
        batch.enable(&pwm_slice_0);
        batch.enable(&pwm_slice_1);
        batch.enable(&pwm_slice_2);
        batch.enable(&pwm_slice_3);
    });
    let pwm_slices = [pwm_slice_0, pwm_slice_1, pwm_slice_2, pwm_slice_3];

    spawner.spawn(
        read_rx_to_fc_packets(in_uart_rx).unwrap()
    );
    spawner.spawn(
        control_pwms(pwm_slices, ext_pwm_0, ext_pwm_1).unwrap()
    );
    spawner.spawn(
        status_led(ws2812).unwrap()
    );

    loop {
        watchdog.feed(Duration::from_millis(500));
        Timer::after_millis(100).await;
    }
}

#[embassy_executor::task]
async fn read_rx_to_fc_packets(
    mut air_uart: UartRx<'static, uart::Async>,
) {
    defmt::info!("Reading CRSF packets...");

    let rc_packets_sender = LAST_RC_PACKET.sender();
    let mut parser = crsf::Parser::new({
        let mut cfg = crsf::ParserConfig::default();
        cfg.sync = &[crsf::PacketAddress::FlightController as u8];
        cfg
    });
    let mut buf = [0; 1];
    loop {
        match with_timeout(CRSF_RESET_TIMEOUT, air_uart.read(&mut buf)).await {
            Ok(Ok(())) => {
                let Some((Ok(raw_packet), _)) = parser.push_bytes_raw(&buf) else {
                    continue;
                };
                if let Ok(crsf::Packet::RcChannelsPacked(rc_channels)) =
                    crsf::Packet::parse(&raw_packet)
                {
                    rc_packets_sender.send((rc_channels, Instant::now()));
                    continue;
                }
            }
            Ok(Err(_)) | Err(_) => {
                parser.reset();
            }
        }
    }
}

#[embassy_executor::task]
async fn control_pwms(
    mut pwm_slices: [Pwm<'static>; NUM_PWM_CHANNELS / 2],
    mut ext_pwm_0: PwmInput<'static, PIO0, 1>,
    mut ext_pwm_1: PwmInput<'static, PIO0, 2>,
) {
    let mut rc_packets_receiver = LAST_RC_PACKET.receiver().unwrap();
    let crsf_status_sender = CRSF_STATUS.sender();
    let ext_pwm_status_sender = EXT_PWM_STATUS.sender();
    let mut is_rx_loss = true;
    let mut is_ext_pwm = false;

    let mut filter_sample_freq = FILTER_SAMPLE_DEFAULT_FREQ;
    let mut filtered_pwm_values: [Pt2Filter<FilteredPwmValue>; NUM_PWM_CHANNELS] =
        PWM_FAILSAFE_VALUES
        .map(|v| Pt2Filter::with_initial_state(
            FILTER_CUT_FREQ.to_fixed(), filter_sample_freq.to_fixed(), v.to_fixed()
        ));

    let mut freq_measurement_start_at: Option<Instant> = None;
    let mut freq_measurement_received_packets = 0;
    let mut last_crsf_at: Option<Instant> = None;

    let mut values = PWM_FAILSAFE_VALUES.clone();

    loop {
        let result = with_timeout(EXT_PWM_POLL_INTERVAL, rc_packets_receiver.changed()).await;
        let ext_pwm_readings = [ext_pwm_0.current_value(), ext_pwm_1.current_value()];
        if ext_pwm_readings.iter().any(|r| r.as_ref().map_or(false, is_ext_pwm_active)) {
            if !is_ext_pwm {
                is_ext_pwm = true;
                ext_pwm_status_sender.send(true);
            }
        } else {
            if is_ext_pwm {
                is_ext_pwm = false;
                ext_pwm_status_sender.send(false);
            }
        }

        match result {
            Ok((rc_channels, rc_timestamp)) => {
                last_crsf_at = Some(rc_timestamp);

                for i in 0..NUM_PWM_CHANNELS {
                    let crsf_pwm = map_rc_channel_to_pwm(rc_channels.0[i]);
                    if (PWM_MIN_VALUE..=PWM_MAX_VALUE).contains(&crsf_pwm) {
                        // Always feed CRSF through the filter so it stays warm
                        // even while external PWM is overriding the output.
                        let filtered = filtered_pwm_values[i]
                            .update(crsf_pwm.to_fixed())
                            .to_num::<i32>()
                            .clamp(PWM_MIN_VALUE as i32, PWM_MAX_VALUE as i32) as u16;
                        let ext = ext_pwm_readings.get(i).copied().flatten();
                        values[i] = resolve_ext_pwm(&ext, filtered);
                    }
                }

                // Calculate sample frequency for the filters
                freq_measurement_received_packets += 1;
                if let Some(ref mut freq_measurement_start_at) = freq_measurement_start_at {
                    let freq_measurement_duration = freq_measurement_start_at.elapsed();
                    if freq_measurement_received_packets > 3
                        && freq_measurement_duration >= Duration::from_millis(1000)
                    {
                        let avg_packet_interval =
                            freq_measurement_duration / (freq_measurement_received_packets - 1);
                        // Packet frequency that is round to 5 Hz
                        let avg_packet_freq = ((1_000_000 / avg_packet_interval.as_micros() + 2) / 5 * 5) as u16;
                        if (FILTER_SAMPLE_MIN_FREQ..=FILTER_SAMPLE_MAX_FREQ).contains(&avg_packet_freq)
                            && avg_packet_freq != filter_sample_freq
                        {
                            filter_sample_freq = avg_packet_freq;
                            for filter in &mut filtered_pwm_values {
                                filter.update_gain(FILTER_CUT_FREQ.to_fixed(), filter_sample_freq.to_fixed());
                            }
                        }
                        *freq_measurement_start_at = rc_timestamp;
                        freq_measurement_received_packets = 0;
                    }
                } else {
                    freq_measurement_start_at = Some(rc_timestamp);
                }

                if is_rx_loss {
                    crsf_status_sender.send(Status::Connected);
                    is_rx_loss = false;
                }
            }
            Err(_) => {
                let rx_loss_now = last_crsf_at
                    .map_or(true, |t| t.elapsed() >= RX_LOSS_TIMEOUT);

                if rx_loss_now {
                    for i in 0..NUM_PWM_CHANNELS {
                        let ext = ext_pwm_readings.get(i).copied().flatten();
                        values[i] = resolve_ext_pwm(&ext, PWM_FAILSAFE_VALUES[i]);
                    }

                    if !is_rx_loss {
                        freq_measurement_start_at = None;
                        freq_measurement_received_packets = 0;
                        crsf_status_sender.send(Status::RxLoss);
                        is_rx_loss = true;
                        for filtered_pwm_value in &mut filtered_pwm_values {
                            filtered_pwm_value.reset();
                        }
                    }
                } else {
                    // CRSF is active but no new packet in EXT_PWM_POLL_INTERVAL;
                    // keep ext PWM channels updated at poll rate using last filter state.
                    for i in 0..2 {
                        let ext = ext_pwm_readings.get(i).copied().flatten();
                        let last_filtered = filtered_pwm_values[i].state()
                            .to_num::<i32>()
                            .clamp(PWM_MIN_VALUE as i32, PWM_MAX_VALUE as i32) as u16;
                        values[i] = resolve_ext_pwm(&ext, last_filtered);
                    }
                }
            }
        }

        update_pwms(&mut pwm_slices, &values).await;
    }
}

#[embassy_executor::task]
async fn status_led(mut ws2812: Ws2812) {
    let mut crsf_status_receiver = CRSF_STATUS.receiver().unwrap();
    let mut ext_pwm_status_receiver = EXT_PWM_STATUS.receiver().unwrap();
    loop {
        let color = match crsf_status_receiver.try_get() {
            Some(Status::Connected) => colors::GREEN,
            Some(Status::RxLoss) => colors::RED,
            None => colors::BLUE,
        };
        let is_blinking = ext_pwm_status_receiver.try_get().unwrap_or(false);
        set_led_color(&mut ws2812, color).await;
        if is_blinking {
            Timer::after_millis(250).await;
            set_led_color(&mut ws2812, colors::BLACK).await;
            Timer::after_millis(250).await;
        } else {
            Timer::after_millis(500).await;
        }
    }
}

async fn update_pwms(
    pwm_slices: &mut [Pwm<'static>; NUM_PWM_CHANNELS / 2],
    values: &[u16; NUM_PWM_CHANNELS],
) {
    // Synchronize update to prevent double-pulse glitches
    if pwm_slices[0].counter() < PWM_MAX_VALUE {
        Timer::after_millis(4).await;
    }

    for (pwm_slice_ix, pwm_slice) in pwm_slices.iter_mut().enumerate() {
        let (mut pwm_0, mut pwm_1) = pwm_slice.split_by_ref();
        set_pwm_duty_cycle(&mut pwm_0, values[pwm_slice_ix]);
        set_pwm_duty_cycle(&mut pwm_1, values[pwm_slice_ix + 1]);
    }
}

fn set_pwm_duty_cycle(pwm: &mut Option<PwmOutput<'_>>, value: u16) {
    let Some(pwm) = pwm else {
        return;
    };
    let _ = pwm.set_duty_cycle(value);
}

async fn set_led_color(ws2812: &mut Ws2812, color: RGB8) {
    let rgb_data = brightness(
        gamma([color].iter().cloned()),
        LED_BRIGHTNESS,
    ).next().unwrap();
    ws2812.write(&[rgb_data]).await;
}
