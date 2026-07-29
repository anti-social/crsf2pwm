use embassy_rp::pio::Instance;
use embassy_rp::pio_programs::onewire::{PioOneWire, PioOneWireSearch};
use fixed::FixedI16;
use fixed::types::extra::U4;
use heapless::Vec;

pub struct Ds18b20<PIO: Instance + 'static, const SM: usize, const N: usize> {
    onewire: PioOneWire<'static, PIO, SM>,
    devices: Vec<u64, N>,
}

impl<PIO: Instance + 'static, const SM: usize, const N: usize> Ds18b20<PIO, SM, N> {
    pub async fn new(mut onewire: PioOneWire<'static, PIO, SM>) -> Self {
        let mut search = PioOneWireSearch::new();
        let mut devices = Vec::new();
        for _ in 0..N {
            if search.is_finished() {
                break;
            }
            if let Some(address) = search.next(&mut onewire).await {
                if crc8(&address.to_le_bytes()) == 0 {
                    defmt::info!("Found address: {:x}", address);
                    let _ = devices.push(address);
                } else {
                    defmt::warn!("Found invalid address: {:x}", address);
                }
            }
        }
        Self { onewire, devices }
    }

    pub async fn read_temperatures(&mut self) -> Vec<FixedI16<U4>, N> {
        let mut measurements = Vec::new();
        if self.devices.is_empty() {
            return measurements;
        }

        // Start conversion on all devices simultaneously
        self.onewire.reset().await;
        self.onewire.write_bytes(&[0xCC, 0x44]).await;
        // DS18B20 needs up to 750ms for a 12-bit conversion
        embassy_time::Timer::after_millis(750).await;

        for device in &self.devices {
            self.onewire.reset().await;
            self.onewire.write_bytes(&[0x55]).await; // Match ROM
            self.onewire.write_bytes(&device.to_le_bytes()).await;
            self.onewire.write_bytes(&[0xBE]).await; // Read scratchpad

            let mut data = [0; 9];
            self.onewire.read_bytes(&mut data).await;
            if crc8(&data) == 0 {
                let _ = measurements.push(
                    FixedI16::from_bits((data[1] as i16) << 8 | data[0] as i16)
                );
            } else {
                defmt::warn!("Reading device {:x} failed. {:02x}", device, data);
            }
        }
        measurements
    }
}

fn crc8(data: &[u8]) -> u8 {
    let mut crc = 0;
    for b in data {
        let mut data_byte = *b;
        for _ in 0..8 {
            let temp = (crc ^ data_byte) & 0x01;
            crc >>= 1;
            if temp != 0 {
                crc ^= 0x8C;
            }
            data_byte >>= 1;
        }
    }
    crc
}
