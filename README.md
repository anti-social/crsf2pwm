# crsf2pwm

CRSF (CrossFire System) to PWM signal converter for RC models, running on Raspberry Pi Pico (RP2040/RP2350).

## What it does

This firmware reads CRSF RC channel packets from an ExpressLRS receiver over UART and outputs them as standard PWM servo signals on 8 channels. It supports:

- **8 PWM channels** (4 PWM slices, dual-output each)
- **External PWM passthrough** — channels 0 and 1 can proxy external PWM signals on GPIO 14/15, ignoring central-range values (1490–1510 µs)
- **Soft start ramps** with configurable up/down rates to avoid servo/ESC glitches on boot
- **2nd-order low-pass filter** (PT2) with auto-detection of CRSF packet rate as sample frequency
- **RX loss detection** with configurable failsafe values (ch1–2 mid, ch3–8 minimum)
- **WS2812 status LED** on GPIO 16: solid green = connected, red = RX loss, blue = booting, blinking = external PWM active

## Hardware

| Pin | Function |
|-----|----------|
| GPIO 12 | CRSF RX (UART0 TX) |
| GPIO 13 | (UART0 RX, unused) |
| GPIO 0–7 | PWM outputs (8 channels) |
| GPIO 14 | External PWM input ch0 |
| GPIO 15 | External PWM input ch1 |
| GPIO 16 | WS2812 LED |

## Building

### Prerequisites

- [Rust toolchain](https://www.rust-lang.org/tools/install) with `thumbv6m-none-eabi` target (RP2040) or `thumbv8m.main-none-eabihf` (RP2350)
- [probe-rs](https://probe.rs/) for flashing
- [picotool](https://github.com/raspberrypi/picotool) for UF2 conversion (optional)

```bash
# RP2040 (default)
make build

# Flash via probe-rs
make run
```

### Chip selection

Toggle between RP2040 and RP2350 via Cargo features:

```bash
cargo build --profile debug-release --target thumbv6m-none-eabihf --features rp2350
```

## Configuration

Values in `src/main.rs`:

| Constant | Default | Description |
|----------|---------|-------------|
| `PWM_MIN_VALUE` | 988 µs | PWM minimum |
| `PWM_MAX_VALUE` | 2012 µs | PWM maximum |
| `PWM_MID_VALUE` | 1500 µs | PWM center |
| `CRSF_RX_BAUDRATE` | 420000 | CRSF serial speed |
| `RX_LOSS_TIMEOUT` | 100 ms | RX signal lost threshold |
| `FILTER_CUT_FREQ` | 5 Hz | Filter cutoff |
| `FILTER_SAMPLE_DEFAULT_FREQ` | 50 Hz | Filter sample rate default |

## License

See [LICENSE](LICENSE).
