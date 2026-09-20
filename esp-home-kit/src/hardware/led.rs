use esp_idf_hal::gpio::OutputPin;
use esp_idf_hal::ledc::{LedcChannel, LedcDriver, LedcTimerDriver};
use std::borrow::Borrow;
use std::time::Instant;

const LED_LUT: [u8; 51] = [
    0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 15, 18, 21, 24, 27, 30, 34, 38, 42, 46, 51, 55,
    60, 66, 71, 77, 83, 89, 96, 102, 109, 116, 124, 131, 139, 148, 156, 165, 174, 183, 192, 202,
    212, 223, 233, 244, 255,
];

pub struct Led<'d> {
    driver: LedcDriver<'d>,
    value: u8,
    current_percentage: u8,
    time_last_change: Instant,
    fade_animation_step: bool,
}

impl<'d> Led<'d> {
    pub fn new<C: LedcChannel + 'd>(
        channel: C,
        timer_driver: impl Borrow<LedcTimerDriver<'d, C::SpeedMode>>,
        pin: impl OutputPin + 'd,
    ) -> Self {
        let mut driver =
            LedcDriver::new(channel, timer_driver, pin).expect("Failed to initialize Led");
        driver.set_duty(0).expect("Failed to set initial duty");
        Self {
            driver,
            value: 0,
            current_percentage: 0,
            time_last_change: Instant::now(),
            fade_animation_step: false,
        }
    }

    pub fn value(&self) -> u8 {
        self.value
    }

    pub fn current_percentage(&self) -> u8 {
        self.current_percentage
    }

    pub fn time_last_change(&self) -> Instant {
        self.time_last_change
    }

    pub fn set_value(&mut self, value: u8) {
        self.driver.set_duty(value as u32).ok();
        self.time_last_change = Instant::now();
        self.value = value;
    }

    pub fn set_level_logarithmic_percent(&mut self, percent: u8) {
        let percent = percent.min(100);
        self.current_percentage = percent;
        let value = LED_LUT[(percent / 2) as usize];
        self.set_value(value);
    }

    pub fn fade_logarithmic_to_percent(&mut self, target_percentage: u8, interval_ms: u32) -> bool {
        if self.time_last_change.elapsed().as_millis() as u32 >= interval_ms {
            if self.current_percentage < target_percentage {
                let new_percentage = (self.current_percentage + 2).min(target_percentage);
                self.set_level_logarithmic_percent(new_percentage);
            } else if self.current_percentage > target_percentage {
                let new_percentage = self
                    .current_percentage
                    .saturating_sub(2)
                    .max(target_percentage);
                self.set_level_logarithmic_percent(new_percentage);
            }
        }
        self.current_percentage == target_percentage
    }

    pub fn fade_logarithmic_between_percentages(
        &mut self,
        percent_1: u8,
        percent_2: u8,
        interval_1_ms: u32,
        interval_2_ms: u32,
        wait_time_1_ms: u64,
        wait_time_2_ms: u64,
    ) {
        if !self.fade_animation_step {
            if self.current_percentage != percent_1 {
                self.fade_logarithmic_to_percent(percent_1, interval_1_ms);
            } else if self.time_last_change.elapsed().as_millis() as u64 >= wait_time_1_ms {
                self.fade_animation_step = true;
            }
        } else {
            if self.current_percentage != percent_2 {
                self.fade_logarithmic_to_percent(percent_2, interval_2_ms);
            } else if self.time_last_change.elapsed().as_millis() as u64 >= wait_time_2_ms {
                self.fade_animation_step = false;
            }
        }
    }
}
