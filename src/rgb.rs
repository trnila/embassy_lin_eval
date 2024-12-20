use cortex_m::prelude::_embedded_hal_Pwm;
use embassy_stm32::timer::{simple_pwm::SimplePwm, Channel as TimChannel};

pub struct RGBLed<TIM: embassy_stm32::timer::GeneralInstance4Channel> {
    pwm: SimplePwm<'static, TIM>,
    chan_r: TimChannel,
    chan_g: TimChannel,
    chan_b: TimChannel,
}

impl<TIM: embassy_stm32::timer::GeneralInstance4Channel> RGBLed<TIM> {
    pub fn new(
        mut pwm: SimplePwm<'static, TIM>,
        chan_r: TimChannel,
        chan_g: TimChannel,
        chan_b: TimChannel,
    ) -> Self {
        pwm.enable(chan_r);
        pwm.enable(chan_g);
        pwm.enable(chan_b);
        RGBLed {
            pwm,
            chan_r,
            chan_g,
            chan_b,
        }
    }

    pub fn set(&mut self, r: u8, g: u8, b: u8) {
        self.pwm.set_duty(self.chan_r, self.value_to_duty(r));
        self.pwm.set_duty(self.chan_g, self.value_to_duty(g));
        self.pwm.set_duty(self.chan_b, self.value_to_duty(b));
    }

    fn value_to_duty(&self, val: u8) -> u32 {
        val as u32 * self.pwm.get_max_duty() / 255
    }
}
