use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, signal::Signal};

#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

pub static SIGNAL_RGB: Signal<CriticalSectionRawMutex, Rgb> = Signal::new();
pub static SIGNAL_LEDS: Signal<CriticalSectionRawMutex, [u8; 4]> = Signal::new();
pub static SIGNAL_PHOTORESISTOR: Signal<CriticalSectionRawMutex, u16> = Signal::new();
