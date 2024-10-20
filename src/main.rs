#![no_std]
#![no_main]

mod fmt;

use cortex_m::singleton;
use lin_bus::{Frame, PID};
#[cfg(not(feature = "defmt"))]
use panic_halt as _;
#[cfg(feature = "defmt")]
use {defmt_rtt as _, panic_probe as _};

use embassy_executor::Spawner;
use embassy_stm32::peripherals::*;
use embassy_stm32::{
    bind_interrupts,
    gpio::{Input, Level, Output, Pull, Speed},
    usart::{self, BufferedUart},
};
use embassy_time::{Duration, Timer};
use embedded_io_async::Read;
use embedded_io_async::Write;
use fmt::info;

bind_interrupts!(struct UARTIRqs {
    USART2 => usart::BufferedInterruptHandler<USART2>;
});

#[embassy_executor::task]
async fn lin_slave_task(mut uart: BufferedUart<'static>) {
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
        if let Some(frame) = lin_slave_response(pid) {
            uart.write_all(frame.get_data_with_checksum())
                .await
                .unwrap();
        } else if let Some(len) = lin_command_size(pid) {
            let mut buf = [0u8; 9];

            uart.read_exact(&mut buf[..=len]).await.unwrap();

            let frame = Frame::from_data(pid, &buf[..len]);
            let valid = frame.get_checksum() == buf[len];

            info!("{} {:x} {}", pid.get_id(), buf[..=len], valid);
        } else {
            info!("drop unknown");
        }
    }
}

fn lin_slave_response(pid: PID) -> Option<Frame> {
    match pid.get_id() {
        8 => Some(Frame::from_data(pid, &[0xaa, 0xbb])),
        _ => None,
    }
}

fn lin_command_size(pid: PID) -> Option<usize> {
    Some(match pid.get_id() {
        12 => 8,
        _ => return None,
    })
}

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let p = embassy_stm32::init(Default::default());
    let mut led_r = Output::new(p.PA6, Level::Low, Speed::Low);
    let mut led_g = Output::new(p.PA7, Level::Low, Speed::Low);
    let mut led_b = Output::new(p.PB0, Level::Low, Speed::Low);

    let mut led_0 = Output::new(p.PA8, Level::Low, Speed::Low);
    let mut led_1 = Output::new(p.PB3, Level::Low, Speed::Low);
    let mut led_2 = Output::new(p.PA12, Level::Low, Speed::Low);
    let mut led_3 = Output::new(p.PA11, Level::Low, Speed::Low);

    let _lin_sleep = Output::new(p.PA4, Level::High, Speed::Low);

    let p2 = Input::new(p.PB9, Pull::Up);
    let p1 = Input::new(p.PB8, Pull::Up);
    let p0 = Input::new(p.PB7, Pull::Up);

    let config = {
        let mut config = usart::Config::default();
        config.baudrate = 19200;
        //config.extended_feature = Some(ExtendedFeature::LIN);
        config
    };

    let tx_buf: &mut [u8; 32] = singleton!(TX_BUF: [u8; 32] = [0; 32]).unwrap();
    let rx_buf: &mut [u8; 32] = singleton!(RX_BUF: [u8; 32] = [0; 32]).unwrap();
    let uart = BufferedUart::new(p.USART2, UARTIRqs, p.PA3, p.PA2, tx_buf, rx_buf, config).unwrap();

    spawner.spawn(lin_slave_task(uart)).unwrap();

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
