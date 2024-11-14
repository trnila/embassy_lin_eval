use defmt::info;
use embassy_stm32::usart::BufferedUart;
use embedded_io_async::Read;
use embedded_io_async::Write;
use lin_bus::Frame;
use lin_bus::PID;

pub trait LinSlaveHandler {
    /// expected size of master frame
    fn master_frame_size(&mut self, frame_id: u8) -> Option<usize>;

    /// process data received from master
    fn process_master_frame(&mut self, frame_id: u8, data: &[u8]);

    /// prepare a data for master request
    fn make_slave_response(&mut self, frame_id: u8) -> Option<&[u8]>;
}

pub async fn lin_slave_driver<T: LinSlaveHandler>(mut uart: BufferedUart<'static>, mut handler: T) {
    loop {
        let mut buf = [0u8; 1];
        uart.read_exact(&mut buf).await.unwrap();
        if buf[0] != 0 {
            continue;
        }

        uart.read_exact(&mut buf).await.unwrap();
        if buf[0] != 0x55 {
            continue;
        }

        uart.read_exact(&mut buf).await.unwrap();
        let pid = PID::new(buf[0]);
        if pid.is_none() {
            info!("bad PID");
            continue;
        }
        let pid = pid.unwrap();
        if let Some(data) = handler.make_slave_response(pid.get_id()) {
            uart.write_all(Frame::from_data(pid, data).get_data_with_checksum())
                .await
                .unwrap();
        } else if let Some(len) = handler.master_frame_size(pid.get_id()) {
            let mut buf = [0u8; 9];

            uart.read_exact(&mut buf[..=len]).await.unwrap();

            let frame = Frame::from_data(pid, &buf[..len]);
            let valid = frame.get_checksum() == buf[len];

            info!("{} {:x} {}", pid.get_id(), buf[..=len], valid);
            if valid {
                handler.process_master_frame(frame.get_pid().get_id(), frame.get_data());
            }
        } else {
            info!("drop unknown");
        }
    }
}
