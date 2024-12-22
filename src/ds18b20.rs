use crate::onewire::{OneWire, SetBaudrate};
use embassy_time::{Duration, Timer};

use {defmt_rtt as _, panic_probe as _};

/// DS18B20 temperature sensor driver
pub struct Ds18b20<TX, RX>
where
    TX: embedded_io_async::Write + SetBaudrate,
    RX: embedded_io_async::Read + SetBaudrate,
{
    bus: OneWire<TX, RX>,
}

impl<TX, RX> Ds18b20<TX, RX>
where
    TX: embedded_io_async::Write + SetBaudrate,
    RX: embedded_io_async::Read + SetBaudrate,
{
    /// Start a temperature conversion.
    const FN_CONVERT_T: u8 = 0x44;
    /// Read contents of the scratchpad containing the temperature.
    const FN_READ_SCRATCHPAD: u8 = 0xBE;

    pub fn new(bus: OneWire<TX, RX>) -> Self {
        Self { bus }
    }

    /// Start a new measurement. Allow at least 1000ms before getting `temperature`.
    async fn start(&mut self) {
        self.bus.reset().await;
        self.bus
            .write_read_byte(OneWire::<TX, RX>::COMMAND_SKIP_ROM)
            .await;
        self.bus.write_read_byte(Self::FN_CONVERT_T).await;
    }

    /// Calculate CRC8 of the data
    fn crc8(data: &[u8]) -> u8 {
        let mut temp;
        let mut data_byte;
        let mut crc = 0;
        for b in data {
            data_byte = *b;
            for _ in 0..8 {
                temp = (crc ^ data_byte) & 0x01;
                crc >>= 1;
                if temp != 0 {
                    crc ^= 0x8C;
                }
                data_byte >>= 1;
            }
        }
        crc
    }

    /// Read the temperature
    pub async fn raw_temperature(&mut self) -> Result<u16, ()> {
        // start measurement
        self.start().await;
        // wait for the conversion
        Timer::after(Duration::from_secs(1)).await;

        self.bus.reset().await;
        self.bus
            .write_read_byte(OneWire::<TX, RX>::COMMAND_SKIP_ROM)
            .await;
        self.bus.write_read_byte(Self::FN_READ_SCRATCHPAD).await;

        let mut data = [0; 9];
        for byte in data.iter_mut() {
            *byte = self.bus.read_byte().await;
        }

        match Self::crc8(&data) == 0 {
            true => Ok((data[1] as u16) << 8 | data[0] as u16),
            false => Err(()),
        }
    }
}
