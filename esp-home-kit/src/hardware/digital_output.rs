use std::time::Instant;
use esp_idf_hal::gpio::{Output, OutputPin, PinDriver};

pub struct DigitalOutput<'a> {
    pin_driver: PinDriver<'a, Output>,
    time_switched_on: Option<Instant>,
    time_switched_off: Option<Instant>,
}

impl<'a> DigitalOutput<'a> {
    pub fn new<P: OutputPin + 'a>(pin: P) -> Self {
        let pin_number = pin.pin();
        Self {
            pin_driver: PinDriver::output(pin).unwrap_or_else(|_| panic!("Failed to initialize pin {}", pin_number)),
            time_switched_on: None,
            time_switched_off: None,
        }
    }

    pub fn is_on(&self) -> bool {
        self.pin_driver.is_set_high()
    }

    pub fn switch_on(&mut self) {
        let switched = self.pin_driver.is_set_low();
        self.pin_driver.set_high().unwrap_or_else(|_| panic!("Failed to switch on pin {}", self.pin_driver.pin()));
        if switched {
            self.time_switched_on = Some(Instant::now());
        }
    }

    pub fn switch_off(&mut self) {
        let switched = self.pin_driver.is_set_high();
        self.pin_driver.set_low().unwrap_or_else(|_| panic!("Failed to switch off pin {}", self.pin_driver.pin()));
        if switched {
            self.time_switched_off = Some(Instant::now());
        }
    }

    pub fn toggle(&mut self) {
        if self.pin_driver.is_set_high() {
            self.switch_off();
        } else {
            self.switch_on();
        }
    }

    pub fn time_switched_on(&self) -> Option<Instant> {
        self.time_switched_on
    }

    pub fn time_switched_off(&self) -> Option<Instant> {
        self.time_switched_off
    }
}
