use esp_idf_hal::gpio::Pull;
use esp_idf_hal::gpio::{Input, InputPin, PinDriver};
use std::time::Duration;
use std::time::Instant;

pub struct DigitalInput<'a> {
    pin_driver: PinDriver<'a, Input>,
    invert_state: bool,
    debounce_milis: u32,
    state: bool,
    previous_state: bool,
    time_state_changed: Instant,
}

impl<'a> DigitalInput<'a> {
    pub fn new<P: InputPin + 'a>(
        pin: P,
        pullup: bool,
        invert_state: bool,
        debounce_milis: u32,
    ) -> Self {
        let pin_number = pin.pin();
        let pull = if pullup { Pull::Up } else { Pull::Floating };
        let pin_driver = PinDriver::input(pin, pull)
            .unwrap_or_else(|_| panic!("Failed to initialize pin {}", pin_number));
        let state = if invert_state {
            !pin_driver.is_high()
        } else {
            pin_driver.is_high()
        };
        Self {
            pin_driver,
            invert_state,
            debounce_milis,
            state,
            previous_state: state,
            time_state_changed: Instant::now(),
        }
    }

    pub fn refresh_state(&mut self) {
        self.previous_state = self.state;
        let pin_state = self.get_state();
        if pin_state != self.state
            && self.time_state_changed.elapsed().as_millis() as u32 >= self.debounce_milis
        {
            self.state = pin_state;
            self.time_state_changed = Instant::now();
        }
    }

    fn get_state(&self) -> bool {
        if self.invert_state {
            return !self.pin_driver.is_high();
        }
        self.pin_driver.is_high()
    }

    pub fn true_at_least_for(&mut self, seconds: u64) -> bool {
        self.refresh_state();
        self.state && self.time_state_changed.elapsed() >= Duration::from_secs(seconds)
    }

    pub fn rising_edge(&self) -> bool {
        self.state && !self.previous_state
    }

    pub fn falling_edge(&self) -> bool {
        !self.state && self.previous_state
    }

    pub fn time_state_changed(&self) -> Instant {
        self.time_state_changed
    }

    pub fn state(&self) -> bool {
        self.state
    }
}
