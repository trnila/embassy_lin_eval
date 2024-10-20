#![no_std]
#![no_main]

mod fmt;

#[cfg(not(feature = "defmt"))]
use panic_halt as _;
#[cfg(feature = "defmt")]
use {defmt_rtt as _, panic_probe as _};

use embassy_executor::Spawner;
use embassy_stm32::gpio::{Input, Level, Output, Pull, Speed};
use embassy_time::{Duration, Timer};
use fmt::info;

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let p = embassy_stm32::init(Default::default());
    let mut led_r = Output::new(p.PA6, Level::Low, Speed::Low);
    let mut led_g = Output::new(p.PA7, Level::Low, Speed::Low);
    let mut led_b = Output::new(p.PB0, Level::Low, Speed::Low);

    let mut led_0 = Output::new(p.PA8, Level::Low, Speed::Low);
    let mut led_1 = Output::new(p.PB3, Level::Low, Speed::Low);
    let mut led_2 = Output::new(p.PA12, Level::Low, Speed::Low);
    let mut led_3 = Output::new(p.PA11, Level::Low, Speed::Low);

    let mut lin_sleep = Output::new(p.PA4, Level::High, Speed::Low);

    let p2 = Input::new(p.PB9, Pull::Up);
    let p1 = Input::new(p.PB8, Pull::Up);
    let p0 = Input::new(p.PB7, Pull::Up);

    loop {
        info!("{} {} {}", p2.get_level(), p1.get_level(), p0.get_level());
        led_r.set_high();
        led_g.set_high();
        led_b.set_high();
        led_0.set_high();
        led_1.set_high();
        led_2.set_high();
        led_3.set_high();
        Timer::after(Duration::from_millis(500)).await;
        led_r.set_low();
        led_g.set_low();
        led_b.set_low();
        led_0.set_low();
        led_1.set_low();
        led_2.set_low();
        led_3.set_low();
        Timer::after(Duration::from_millis(500)).await;
    }
}
