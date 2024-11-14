use defmt::info;
use embassy_stm32::gpio::Output;
use embassy_stm32::usart::BufferedUart;
use embedded_io_async::Read;
use embedded_io_async::Write;
use lin_bus::Frame;
use lin_bus::PID;

use crate::signals::Rgb;
use crate::signals::SIGNAL_LEDS;
use crate::signals::SIGNAL_PHOTORESISTOR;
use crate::signals::SIGNAL_RGB;

const LIN_FRAME_RGB: u8 = 0;
const LIN_FRAME_LEDS: u8 = 1;
const LIN_FRAME_PHOTORES: u8 = 2;

struct FrameResponses {
    photores: [u8; 2],
}

#[embassy_executor::task]
pub async fn lin_slave_task(mut uart: BufferedUart<'static>, _lin_sleep: Output<'static>) {
    let mut frames = FrameResponses { photores: [0; 2] };
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
        if let Some(data) = lin_slave_response(pid.get_id(), &mut frames) {
            uart.write_all(Frame::from_data(pid, data).get_data_with_checksum())
                .await
                .unwrap();
        } else if let Some(len) = lin_command_size(pid.get_id()) {
            let mut buf = [0u8; 9];

            uart.read_exact(&mut buf[..=len]).await.unwrap();

            let frame = Frame::from_data(pid, &buf[..len]);
            let valid = frame.get_checksum() == buf[len];

            info!("{} {:x} {}", pid.get_id(), buf[..=len], valid);
            if valid {
                lin_slave_process(frame.get_pid().get_id(), frame.get_data());
            }
        } else {
            info!("drop unknown");
        }
    }
}

fn lin_slave_process(frame_id: u8, data: &[u8]) {
    if frame_id == LIN_FRAME_RGB {
        let color = Rgb {
            r: data[0],
            g: data[1],
            b: data[2],
        };
        info!("got color: {}", color);
        SIGNAL_RGB.signal(color);
    } else if frame_id == LIN_FRAME_LEDS {
        let leds = [data[0] & 1, data[0] & 2, data[0] & 4, data[0] & 8];
        info!("got leds: {}", leds);
        SIGNAL_LEDS.signal(leds);
    }
}

fn lin_slave_response(frame_id: u8, state: &mut FrameResponses) -> Option<&[u8]> {
    match frame_id {
        LIN_FRAME_PHOTORES => {
            if let Some(millivolts) = SIGNAL_PHOTORESISTOR.try_take() {
                state.photores[0] = (millivolts & 0xFF) as u8;
                state.photores[1] = ((millivolts >> 8) & 0xFF) as u8;
            }
            Some(&state.photores)
        }
        _ => None,
    }
}

fn lin_command_size(frame_id: u8) -> Option<usize> {
    Some(match frame_id {
        LIN_FRAME_RGB => 3,
        LIN_FRAME_LEDS => 1,
        _ => return None,
    })
}
