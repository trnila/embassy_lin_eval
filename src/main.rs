#![no_std]
#![no_main]

mod lin_slave;
mod rgb;
mod signals;

use cortex_m::singleton;
use defmt::info;
#[cfg(not(feature = "defmt"))]
use panic_halt as _;
use signals::{SIGNAL_LEDS, SIGNAL_PHOTORESISTOR, SIGNAL_RGB};
#[cfg(feature = "defmt")]
use {defmt_rtt as _, panic_probe as _};

use crate::rgb::RGBLed;
use embassy_executor::Spawner;
use embassy_stm32::{
    adc::AdcChannel,
    gpio::OutputType,
    time::khz,
    timer::simple_pwm::{PwmPin, SimplePwm},
};
use embassy_stm32::{adc::AnyAdcChannel, timer::Channel as TimChannel};
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
use lin_slave::lin_slave_task;

bind_interrupts!(struct UARTIRqs {
    USART2 => usart::BufferedInterruptHandler<USART2>;
});

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

#[embassy_executor::task]
async fn adc_task(
    mut adc: Adc<'static, ADC1>,
    mut dma: DMA1_CH1,
    mut channel: AnyAdcChannel<ADC1>,
) {
    let read_buffer: &mut [u16; 2] = singleton!(ADC_BUF: [u16; 2] = [0; 2]).unwrap();
    let mut vrefint_channel = adc.enable_vrefint().degrade_adc();

    loop {
        adc.read(
            &mut dma,
            [
                (&mut vrefint_channel, SampleTime::CYCLES160_5),
                (&mut channel, SampleTime::CYCLES160_5),
            ]
            .into_iter(),
            read_buffer,
        )
        .await;

        let vrefint = read_buffer[1];
        let measured = read_buffer[0];

        const VREFINT_MV: u32 = 1212; // mV
        let measured_mv: u16 = (u32::from(measured) * VREFINT_MV / u32::from(vrefint)) as u16;

        SIGNAL_PHOTORESISTOR.signal(measured_mv);

        info!("vrefint: {} PA0: {} {}mV", vrefint, measured, measured_mv);
        Timer::after(Duration::from_millis(500)).await;
    }
}

fn get_board_id(pins: &[Input]) -> u8 {
    return pins
        .iter()
        .enumerate()
        .map(|(pos, pin)| {
            if pin.is_low() {
                2u8.pow((pins.len() - 1 - pos) as u32)
            } else {
                0
            }
        })
        .sum();
}

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let p = embassy_stm32::init(Default::default());

    let board_id = get_board_id(&[
        Input::new(p.PB9, Pull::Up),
        Input::new(p.PB8, Pull::Up),
        Input::new(p.PB7, Pull::Up),
    ]);
    info!("Board id: {}", board_id);

    let leds = [
        Output::new(p.PA8, Level::Low, Speed::Low),
        Output::new(p.PB3, Level::Low, Speed::Low),
        Output::new(p.PA12, Level::Low, Speed::Low),
        Output::new(p.PA11, Level::Low, Speed::Low),
    ];
    let lin_sleep = Output::new(p.PA4, Level::High, Speed::Low);

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

    let adc = Adc::new(p.ADC1);
    let dma = p.DMA1_CH1;
    let pa0 = p.PA0.degrade_adc();

    spawner.spawn(lin_slave_task(uart, lin_sleep)).unwrap();
    spawner.spawn(rgb_task(pwm_rgb)).unwrap();
    spawner.spawn(leds_task(leds)).unwrap();
    spawner.spawn(adc_task(adc, dma, pa0)).unwrap();
}
