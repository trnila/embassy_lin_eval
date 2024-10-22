#![no_std]
#![no_main]

mod rgb;

use cortex_m::singleton;
use defmt::info;
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, signal::Signal};
use lin_bus::{Frame, PID};
#[cfg(not(feature = "defmt"))]
use panic_halt as _;
#[cfg(feature = "defmt")]
use {defmt_rtt as _, panic_probe as _};

use crate::rgb::RGBLed;
use embassy_executor::Spawner;
use embassy_stm32::timer::Channel as TimChannel;
use embassy_stm32::{
    adc::AdcChannel,
    gpio::OutputType,
    time::khz,
    timer::simple_pwm::{PwmPin, SimplePwm},
};
use embassy_stm32::{
    adc::{Adc, SampleTime},
    peripherals::*,
};
use embassy_stm32::{
    bind_interrupts,
    gpio::{Input, Level, Output, Pull, Speed},
    usart::{self, BufferedUart},
};
use embassy_time::{Duration, Timer};
use embedded_io_async::Read;
use embedded_io_async::Write;

bind_interrupts!(struct UARTIRqs {
    USART2 => usart::BufferedInterruptHandler<USART2>;
});

#[cfg_attr(feature = "defmt", derive(defmt::Format))]
struct Rgb {
    r: u8,
    g: u8,
    b: u8,
}

static SIGNAL_RGB: Signal<CriticalSectionRawMutex, Rgb> = Signal::new();
static SIGNAL_LEDS: Signal<CriticalSectionRawMutex, [u8; 4]> = Signal::new();

const LIN_FRAME_RGB: u8 = 0;
const LIN_FRAME_LEDS: u8 = 1;

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
            if valid {
                lin_slave_process(frame.get_pid().get_id(), frame.get_data());
            }
        } else {
            info!("drop unknown");
        }
    }
}

fn lin_slave_process(id: u8, data: &[u8]) {
    if id == LIN_FRAME_RGB {
        let color = Rgb {
            r: data[0],
            g: data[1],
            b: data[2],
        };
        info!("got color: {}", color);
        SIGNAL_RGB.signal(color);
    } else if id == LIN_FRAME_LEDS {
        let leds = [data[0] & 1, data[0] & 2, data[0] & 4, data[0] & 8];
        info!("got leds: {}", leds);
        SIGNAL_LEDS.signal(leds);
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
        LIN_FRAME_RGB => 3,
        LIN_FRAME_LEDS => 1,
        _ => return None,
    })
}

#[embassy_executor::task]
async fn rgb_task(mut led: RGBLed<TIM3>) {
    loop {
        let req = SIGNAL_RGB.wait().await;
        led.set(req.r, req.g, req.b);
    }
}

#[embassy_executor::task]
async fn leds_task(mut leds: [Output<'static>; 4]) {
    loop {
        let req = SIGNAL_LEDS.wait().await;
        for (led, state) in leds.iter_mut().zip(req) {
            led.set_level(match state {
                0 => Level::Low,
                _ => Level::High,
            });
        }
    }
}

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let p = embassy_stm32::init(Default::default());

    let leds = [
        Output::new(p.PA8, Level::Low, Speed::Low),
        Output::new(p.PB3, Level::Low, Speed::Low),
        Output::new(p.PA12, Level::Low, Speed::Low),
        Output::new(p.PA11, Level::Low, Speed::Low),
    ];

    let _lin_sleep = Output::new(p.PA4, Level::High, Speed::Low);

    let p2 = Input::new(p.PB9, Pull::Up);
    let p1 = Input::new(p.PB8, Pull::Up);
    let p0 = Input::new(p.PB7, Pull::Up);

    let rgb_blue_ch = PwmPin::new_ch1(p.PA6, OutputType::PushPull);
    let rgb_red_ch = PwmPin::new_ch2(p.PA7, OutputType::PushPull);
    let rgb_green_ch = PwmPin::new_ch3(p.PB0, OutputType::PushPull);

    let pwm = SimplePwm::new(
        p.TIM3,
        Some(rgb_blue_ch),
        Some(rgb_red_ch),
        Some(rgb_green_ch),
        None,
        khz(1),
        Default::default(),
    );
    let mut pwm_rgb = RGBLed::new(pwm, TimChannel::Ch2, TimChannel::Ch3, TimChannel::Ch1);
    pwm_rgb.set(0, 0, 255);

    let config = {
        let mut config = usart::Config::default();
        config.baudrate = 19200;
        //config.extended_feature = Some(ExtendedFeature::LIN);
        config
    };

    let tx_buf: &mut [u8; 32] = singleton!(TX_BUF: [u8; 32] = [0; 32]).unwrap();
    let rx_buf: &mut [u8; 32] = singleton!(RX_BUF: [u8; 32] = [0; 32]).unwrap();
    let uart = BufferedUart::new(p.USART2, UARTIRqs, p.PA3, p.PA2, tx_buf, rx_buf, config).unwrap();

    let read_buffer: &mut [u16; 2] = singleton!(ADC_BUF: [u16; 2] = [0; 2]).unwrap();
    let mut adc = Adc::new(p.ADC1);
    let mut dma = p.DMA1_CH1;
    let mut vrefint_channel = adc.enable_vrefint().degrade_adc();
    let mut pa0 = p.PA0.degrade_adc();

    spawner.spawn(lin_slave_task(uart)).unwrap();
    spawner.spawn(rgb_task(pwm_rgb)).unwrap();
    spawner.spawn(leds_task(leds)).unwrap();

    loop {
        adc.read(
            &mut dma,
            [
                (&mut vrefint_channel, SampleTime::CYCLES160_5),
                (&mut pa0, SampleTime::CYCLES160_5),
            ]
            .into_iter(),
            read_buffer,
        )
        .await;

        let vrefint = read_buffer[1];
        let measured = read_buffer[0];

        const VREFINT_MV: u32 = 1212; // mV
        let measured_mv: u16 = (u32::from(measured) * VREFINT_MV / u32::from(vrefint)) as u16;

        info!(
            "{} {} {} vrefint: {} PA0: {} {}mV",
            p2.get_level(),
            p1.get_level(),
            p0.get_level(),
            vrefint,
            measured,
            measured_mv
        );
        Timer::after(Duration::from_millis(500)).await;
    }
}
