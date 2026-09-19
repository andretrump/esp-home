use crate::hardware::Led;

const STARTUP_MAX_PERCENTAGE: u8 = 50;
const STARTUP_SWITCH_TO_NEXT_UP: u8 = 16;
const STARTUP_SWITCH_TO_NEXT_DOWN: u8 = STARTUP_MAX_PERCENTAGE - STARTUP_SWITCH_TO_NEXT_UP;
const STARTUP_INTERVAL_MS: u32 = 12;
const STARTUP_LOOPS: u8 = 3;

pub enum ConnectionState {
    WifiMqttConnected,
    WifiConnected,
    PortalOpen,
    Disconnected,
    Deactivated,
}

pub struct LedGroup<'d> {
    green: Led<'d>,
    red: Led<'d>,
    blue: Led<'d>,
    startup_animation_step: u8,
    startup_animation_finished: bool,
}

impl<'d> LedGroup<'d> {
    pub fn new(green: Led<'d>, red: Led<'d>, blue: Led<'d>) -> Self {
        Self {
            green,
            red,
            blue,
            startup_animation_step: 0,
            startup_animation_finished: false,
        }
    }

    pub fn run_startup_animation(&mut self) -> bool {
        if !self.startup_animation_finished {
            match self.startup_animation_step % 2 {
                0 => self.startup_fade_up(),
                _ => self.startup_fade_down(),
            }
            self.startup_animation_finished = self.startup_animation_step >= STARTUP_LOOPS * 2;
        }
        self.startup_animation_finished
    }

    fn startup_fade_up(&mut self) {
        self.green
            .fade_logarithmic_to_percent(STARTUP_MAX_PERCENTAGE, STARTUP_INTERVAL_MS);
        if self.green.current_percentage() > STARTUP_SWITCH_TO_NEXT_UP {
            self.red
                .fade_logarithmic_to_percent(STARTUP_MAX_PERCENTAGE, STARTUP_INTERVAL_MS);
        }
        if self.red.current_percentage() > STARTUP_SWITCH_TO_NEXT_UP {
            self.blue
                .fade_logarithmic_to_percent(STARTUP_MAX_PERCENTAGE, STARTUP_INTERVAL_MS);
        }
        if self.blue.current_percentage() >= STARTUP_MAX_PERCENTAGE {
            self.startup_animation_step += 1;
            log::info!(
                "Startup animation {}%",
                self.startup_animation_step as u32 * 100 / (STARTUP_LOOPS as u32 * 2)
            );
        }
    }

    fn startup_fade_down(&mut self) {
        self.green.fade_logarithmic_to_percent(0, STARTUP_INTERVAL_MS);
        if self.green.current_percentage() < STARTUP_SWITCH_TO_NEXT_DOWN {
            self.red.fade_logarithmic_to_percent(0, STARTUP_INTERVAL_MS);
        }
        if self.red.current_percentage() < STARTUP_SWITCH_TO_NEXT_DOWN {
            self.blue.fade_logarithmic_to_percent(0, STARTUP_INTERVAL_MS);
        }
        if self.blue.current_percentage() == 0 {
            self.startup_animation_step += 1;
            log::info!(
                "Startup animation {}%",
                self.startup_animation_step as u32 * 100 / (STARTUP_LOOPS as u32 * 2)
            );
        }
    }

    pub fn display_pump_state(&mut self, pump_on: bool, pump_enabled: bool) {
        if pump_on && pump_enabled {
            self.green
                .fade_logarithmic_between_percentages(30, 100, 15, 15, 250, 250);
        } else if pump_enabled {
            self.green.fade_logarithmic_to_percent(30, 15);
        } else {
            self.green.fade_logarithmic_to_percent(0, 10);
        }
    }

    pub fn display_alert_state(
        &mut self,
        pump_enabled: bool,
        water_level_low: bool,
        temperature_error: bool,
    ) {
        if !pump_enabled {
            self.red.fade_logarithmic_to_percent(50, 10);
        } else if water_level_low {
            self.red
                .fade_logarithmic_between_percentages(0, 100, 30, 10, 300, 200);
        } else if temperature_error {
            self.red
                .fade_logarithmic_between_percentages(0, 100, 2, 2, 250, 250);
        } else {
            self.red.fade_logarithmic_to_percent(0, 10);
        }
    }

    pub fn display_connection_state(&mut self, connection_state: ConnectionState) {
        match connection_state {
            ConnectionState::WifiMqttConnected => {
                self.blue.fade_logarithmic_to_percent(30, 40);
            }
            ConnectionState::Disconnected => {
                self.blue
                    .fade_logarithmic_between_percentages(0, 100, 30, 30, 500, 500);
            }
            ConnectionState::WifiConnected => {
                self.blue
                    .fade_logarithmic_between_percentages(30, 100, 8, 8, 100, 100);
            }
            ConnectionState::PortalOpen => {
                self.blue
                    .fade_logarithmic_between_percentages(0, 100, 3, 3, 250, 250);
            }
            ConnectionState::Deactivated => {
                self.blue.fade_logarithmic_to_percent(0, 10);
            }
        }
    }
}
