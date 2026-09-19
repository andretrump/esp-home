use crate::hardware;
use crate::mqtt;
use crate::utils::Timer;
use esp_idf_svc::mqtt::client::EspMqttClient;
use std::cell::RefCell;
use std::rc::Rc;

pub const PUMP_ON_SECS: u64 = 60;
pub const PUMP_OFF_SECS: u64 = 40 * 60;

pub struct PumpController {
    enable_switch: Rc<RefCell<mqtt::Switch>>,
    pump_switch: Rc<RefCell<mqtt::Switch>>,
    output: Rc<RefCell<hardware::DigitalOutput<'static>>>,
    on_timer: Timer,
    off_timer: Timer,
    prev_enabled: bool,
    current_off_secs: u64,
}

impl PumpController {
    pub fn new(
        enable_switch: Rc<RefCell<mqtt::Switch>>,
        pump_switch: Rc<RefCell<mqtt::Switch>>,
        output: Rc<RefCell<hardware::DigitalOutput<'static>>>,
    ) -> Self {
        Self {
            enable_switch,
            pump_switch,
            output,
            on_timer: Timer::new(PUMP_ON_SECS),
            off_timer: Timer::new(PUMP_OFF_SECS),
            prev_enabled: true,
            current_off_secs: PUMP_OFF_SECS,
        }
    }

    pub fn toggle_enabled(&mut self, mqtt_client: Option<&mut EspMqttClient>) {
        self.enable_switch
            .borrow_mut()
            .toggle(mqtt_client)
            .unwrap_or_else(|e| log::warn!("Failed to toggle enable pump switch: {}", e));
    }

    pub fn is_on(&self) -> bool {
        self.pump_switch.borrow().is_on()
    }

    pub fn is_enabled(&self) -> bool {
        self.enable_switch.borrow().is_on()
    }

    pub fn countdown_secs(&self) -> u64 {
        let pump_enabled = self.enable_switch.borrow().is_on();
        let pump_is_on = self.pump_switch.borrow().is_on();
        if !pump_enabled {
            0
        } else if pump_is_on {
            PUMP_ON_SECS.saturating_sub(self.on_timer.elapsed_secs())
        } else {
            self.current_off_secs
                .saturating_sub(self.off_timer.elapsed_secs())
        }
    }

    pub fn tick(&mut self, mqtt_client: Option<&mut EspMqttClient>, temperature: Option<f32>) {
        self.current_off_secs = off_duration_secs(temperature);
        self.advance_cycle(mqtt_client);
        self.drive_output();
    }

    fn advance_cycle(&mut self, mqtt_client: Option<&mut EspMqttClient>) {
        let pump_enabled = self.enable_switch.borrow().is_on();
        let just_disabled = !pump_enabled && self.prev_enabled;
        self.prev_enabled = pump_enabled;

        if just_disabled {
            self.stop_pump(mqtt_client);
        } else if pump_enabled {
            let pump_is_on = self.pump_switch.borrow().is_on();
            if pump_is_on && self.on_timer.has_elapsed() {
                self.stop_pump(mqtt_client);
            } else if !pump_is_on && self.off_timer.elapsed_secs() >= self.current_off_secs {
                self.start_pump(mqtt_client);
            }
        }
    }

    fn stop_pump(&mut self, mqtt_client: Option<&mut EspMqttClient>) {
        self.pump_switch
            .borrow_mut()
            .switch_off(mqtt_client)
            .unwrap_or_else(|e| log::warn!("Failed to stop pump: {}", e));
        self.on_timer.reset();
        self.off_timer.reset();
    }

    fn start_pump(&mut self, mqtt_client: Option<&mut EspMqttClient>) {
        self.pump_switch
            .borrow_mut()
            .switch_on(mqtt_client)
            .unwrap_or_else(|e| log::warn!("Failed to start pump: {}", e));
        self.off_timer.reset();
        self.on_timer.reset();
    }

    fn drive_output(&mut self) {
        let pump_on = self.pump_switch.borrow().is_on();
        let pump_enabled = self.enable_switch.borrow().is_on();
        if pump_on && pump_enabled {
            self.output.borrow_mut().switch_on();
        } else {
            self.output.borrow_mut().switch_off();
        }
    }
}

fn off_duration_secs(temperature: Option<f32>) -> u64 {
    let Some(temp) = temperature else {
        return PUMP_OFF_SECS;
    };
    if temp <= 20.0 {
        return PUMP_OFF_SECS;
    }
    let kelvin_over_20 = (temp as u8).saturating_sub(20) as f32;
    let factor = (1.0 - 0.9 * (kelvin_over_20 / 15.0).powi(2) - kelvin_over_20 * 0.01).max(0.15);
    (PUMP_OFF_SECS as f32 * factor) as u64
}
