use defmt::info;

use crate::lin_slave_driver::LinSlaveHandler;
use crate::signals::Rgb;
use crate::signals::SIGNAL_LEDS;
use crate::signals::SIGNAL_PHOTORESISTOR;
use crate::signals::SIGNAL_RGB;

const LIN_FRAME_RGB: u8 = 0;
const LIN_FRAME_LEDS: u8 = 1;
const LIN_FRAME_PHOTORES: u8 = 2;

pub struct LinHandler {
    photores: [u8; 2],
}

impl LinHandler {
    pub fn new() -> Self {
        Self { photores: [0; 2] }
    }
}

impl LinSlaveHandler for LinHandler {
    fn process_master_frame(&mut self, frame_id: u8, data: &[u8]) {
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

    fn make_slave_response(&mut self, frame_id: u8) -> Option<&[u8]> {
        match frame_id {
            LIN_FRAME_PHOTORES => {
                if let Some(millivolts) = SIGNAL_PHOTORESISTOR.try_take() {
                    self.photores[0] = (millivolts & 0xFF) as u8;
                    self.photores[1] = ((millivolts >> 8) & 0xFF) as u8;
                }
                Some(&self.photores)
            }
            _ => None,
        }
    }

    fn master_frame_size(&mut self, frame_id: u8) -> Option<usize> {
        Some(match frame_id {
            LIN_FRAME_RGB => 3,
            LIN_FRAME_LEDS => 1,
            _ => return None,
        })
    }
}
