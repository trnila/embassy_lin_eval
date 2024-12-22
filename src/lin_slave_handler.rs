use defmt::info;

use crate::lin_slave_driver::LinSlaveHandler;
use crate::signals::Rgb;
use crate::signals::SIGNAL_LEDS;
use crate::signals::SIGNAL_PHOTORESISTOR;
use crate::signals::SIGNAL_RGB;
use crate::signals::SIGNAL_TEMPERATURE;

const LIN_FRAME_ID_OFFSET: u8 = 5;
const LIN_FRAME_RGB: u8 = 0;
const LIN_FRAME_LEDS: u8 = 1;
const LIN_FRAME_PHOTORES: u8 = 2;
const LIN_FRAME_TEMP: u8 = 3;

enum LocalFrameId {
    Rgb,
    Leds,
    Photores,
    Temp,
}

impl LocalFrameId {
    fn from_frame_id(board_id: u8, frame_id: u8) -> Option<Self> {
        let shifted_frame_id = frame_id.checked_sub(LIN_FRAME_ID_OFFSET * board_id)?;
        match shifted_frame_id {
            LIN_FRAME_RGB => Some(LocalFrameId::Rgb),
            LIN_FRAME_LEDS => Some(LocalFrameId::Leds),
            LIN_FRAME_PHOTORES => Some(LocalFrameId::Photores),
            LIN_FRAME_TEMP => Some(LocalFrameId::Temp),
            _ => None,
        }
    }
}

pub struct LinHandler {
    board_id: u8,
    photores: [u8; 2],
    temp: [u8; 2],
}

impl LinHandler {
    pub fn new(board_id: u8) -> Self {
        Self {
            board_id,
            photores: [0; 2],
            temp: [0; 2],
        }
    }
}

impl LinSlaveHandler for LinHandler {
    fn process_master_frame(&mut self, frame_id: u8, data: &[u8]) {
        match LocalFrameId::from_frame_id(self.board_id, frame_id) {
            Some(LocalFrameId::Rgb) => {
                let color = Rgb {
                    r: data[0],
                    g: data[1],
                    b: data[2],
                };
                info!("got color: {}", color);
                SIGNAL_RGB.signal(color);
            }
            Some(LocalFrameId::Leds) => {
                let leds = [data[0] & 1, data[0] & 2, data[0] & 4, data[0] & 8];
                info!("got leds: {}", leds);
                SIGNAL_LEDS.signal(leds);
            }
            _ => {}
        }
    }

    fn make_slave_response(&mut self, frame_id: u8) -> Option<&[u8]> {
        match LocalFrameId::from_frame_id(self.board_id, frame_id) {
            Some(LocalFrameId::Photores) => {
                if let Some(millivolts) = SIGNAL_PHOTORESISTOR.try_take() {
                    self.photores[0] = (millivolts & 0xFF) as u8;
                    self.photores[1] = ((millivolts >> 8) & 0xFF) as u8;
                }
                Some(&self.photores)
            }
            Some(LocalFrameId::Temp) => {
                if let Some(value) = SIGNAL_TEMPERATURE.try_take() {
                    if let Some(temperature) = value {
                        self.temp[0] = (temperature & 0xFF) as u8;
                        self.temp[1] = ((temperature >> 8) & 0xFF) as u8;
                    } else {
                        self.temp[0] = 0xFF;
                        self.temp[1] = 0xFF;
                    }
                }
                Some(&self.temp)
            }
            _ => None,
        }
    }

    fn master_frame_size(&mut self, frame_id: u8) -> Option<usize> {
        Some(match LocalFrameId::from_frame_id(self.board_id, frame_id) {
            Some(LocalFrameId::Rgb) => 3,
            Some(LocalFrameId::Leds) => 1,
            _ => return None,
        })
    }
}
