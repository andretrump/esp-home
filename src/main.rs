use esp_idf_hal::delay::FreeRtos;
use esp_idf_hal::ledc::{config::TimerConfig, LedcTimerDriver};
use esp_idf_hal::peripherals::Peripherals;
use esp_idf_svc::eventloop::EspSystemEventLoop;
use esp_idf_svc::nvs::EspDefaultNvsPartition;
use esp_idf_svc::sys;
use plant_tower_rs::captive_portal::http_server::CaptivePortalTimeout;
use plant_tower_rs::captive_portal::CaptivePortal;
use plant_tower_rs::connectivity::{ConnectionManager, MqttCredentials, WifiManager};
use plant_tower_rs::hardware::{self, NvsKey, NvsManager};
use plant_tower_rs::mqtt::{self, ActuatorComponent, SensorComponent};
use plant_tower_rs::nvs_keys;
use plant_tower_rs::utils::Timer;
use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt;
use std::rc::Rc;
enum Components {
    EnablePumpSwitch,
    PumpSwitch,
    TemperatureSensor,
    WaterLevelSensor,
}

impl fmt::Display for Components {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Components::EnablePumpSwitch => write!(f, "plant_tower_rs_enable_pump_switch"),
            Components::PumpSwitch => write!(f, "plant_tower_rs_pump_switch"),
            Components::TemperatureSensor => write!(f, "plant_tower_rs_temperature_sensor"),
            Components::WaterLevelSensor => write!(f, "plant_tower_rs_water_level_sensor"),
        }
    }
}

nvs_keys! {
    enum ConfigKey {
        WifiSsid => "wifi_ssid",
        WifiPassword => "wifi_password",
        MqttHost => "mqtt_host",
        MqttPort => "mqtt_port",
        MqttUser => "mqtt_user",
        MqttPassword => "mqtt_password",
        MqttDevName => "mqtt_dev_name",
        MqttDevId => "mqtt_dev_id"
    }
}

nvs_keys! {
    enum StateKey {
        ForcePortal => "force_portal"
    }
}

const PUMP_ON_SECS: u64 = 10;
const PUMP_OFF_SECS: u64 = 20;
const SENSOR_REFRESH_SECS: u64 = 10;

fn main() {
    esp_idf_svc::sys::link_patches();
    esp_idf_svc::log::EspLogger::initialize_default();
    let peripherals = Peripherals::take().expect("Failed to initialize peripherals");
    let sys_loop = EspSystemEventLoop::take().expect("Failed to initialize system loop");
    let nvs_default_partition = EspDefaultNvsPartition::take().expect("Failed to initialize NVS");

    let nvs_config_manager =
        NvsManager::<ConfigKey>::new(nvs_default_partition.clone(), String::from("PLANT_TWR_CFG"));
    let nvs_state_manager =
        NvsManager::<StateKey>::new(nvs_default_partition.clone(), String::from("PLANT_TWR_ST"));
    let mut wifi_manager =
        WifiManager::new(peripherals.modem, sys_loop, nvs_default_partition.clone());

    run_captive_portal_if_needed(&nvs_config_manager, &nvs_state_manager, &mut wifi_manager);

    let mut enable_pump_button =
        hardware::DigitalInput::new(peripherals.pins.gpio18, true, true, 20);
    let mut reset_connectivity_button =
        hardware::DigitalInput::new(peripherals.pins.gpio19, true, true, 20);

    let mut temperature_sensor = hardware::OneWireTemperatureSensor::new(peripherals.pins.gpio23);
    let mut water_level_sensor =
        hardware::DigitalInput::new(peripherals.pins.gpio22, true, true, 20);
    let (mqtt_temperature_sensor, mqtt_water_level_sensor) = create_sensors();
    let (enable_pump_switch, pump_switch, pump) = create_pump(peripherals.pins.gpio13);

    let (device, credentials) = setup_network(
        &nvs_config_manager,
        &mut wifi_manager,
        &enable_pump_switch,
        &pump_switch,
        &mqtt_temperature_sensor,
        &mqtt_water_level_sensor,
    );
    let mut connection_manager = ConnectionManager::new(wifi_manager, device, credentials);

    enable_pump_switch
        .borrow_mut()
        .switch_on(connection_manager.mqtt_client())
        .unwrap_or_else(|err| log::warn!("Failed to enable pump: {}", err));
    pump_switch
        .borrow_mut()
        .switch_on(connection_manager.mqtt_client())
        .unwrap_or_else(|err| log::warn!("Failed to switch on pump: {}", err));

    let ledc_timer = LedcTimerDriver::new(peripherals.ledc.timer0, &TimerConfig::default())
        .expect("Failed to initialize LEDC timer");
    let mut led_group = hardware::LedGroup::new(
        hardware::Led::new(
            peripherals.ledc.channel0,
            &ledc_timer,
            peripherals.pins.gpio25,
        ),
        hardware::Led::new(
            peripherals.ledc.channel1,
            &ledc_timer,
            peripherals.pins.gpio26,
        ),
        hardware::Led::new(
            peripherals.ledc.channel2,
            &ledc_timer,
            peripherals.pins.gpio33,
        ),
    );
    while !led_group.run_startup_animation() {
        FreeRtos::delay_ms(10);
    }

    let mut temperature_error = false;
    let mut sensor_refresh_timer = Timer::new(SENSOR_REFRESH_SECS);
    let mut pump_on_timer = Timer::new(PUMP_ON_SECS);
    let mut pump_off_timer = Timer::new(PUMP_OFF_SECS);
    let mut prev_pump_enabled = true;

    loop {
        connection_manager.tick();

        enable_pump_button.refresh_state();
        if enable_pump_button.falling_edge() {
            enable_pump_switch
                .borrow_mut()
                .toggle(connection_manager.mqtt_client())
                .unwrap_or_else(|err| log::warn!("Failed to toggle enable pump switch: {}", err));
        }

        if reset_connectivity_button.true_at_least_for(5) {
            nvs_state_manager.store_property(StateKey::ForcePortal, "1");
            unsafe { sys::esp_restart() }
        }

        sensor_refresh_timer.run(|| {
            match temperature_sensor.get_temperature() {
                Some(temperature) => {
                    temperature_error = false;
                    mqtt_temperature_sensor
                        .borrow_mut()
                        .set_value(temperature, connection_manager.mqtt_client());
                }
                None => {
                    temperature_error = true;
                    mqtt_temperature_sensor
                        .borrow_mut()
                        .clear_value(connection_manager.mqtt_client());
                }
            };

            mqtt_water_level_sensor.borrow_mut().set_value(
                water_level_sensor.refresh_state().state(),
                connection_manager.mqtt_client(),
            );
        });

        let pump_enabled = enable_pump_switch.borrow().is_on();
        if !pump_enabled && prev_pump_enabled {
            pump_switch
                .borrow_mut()
                .switch_off(connection_manager.mqtt_client())
                .unwrap_or_else(|e| log::warn!("Failed to stop pump: {}", e));
            pump_on_timer.reset();
            pump_off_timer.reset();
        }
        prev_pump_enabled = pump_enabled;

        if pump_enabled {
            let pump_is_on = pump_switch.borrow().is_on();
            if pump_is_on {
                pump_on_timer.run(|| {
                    pump_switch
                        .borrow_mut()
                        .switch_off(connection_manager.mqtt_client())
                        .unwrap_or_else(|e| log::warn!("Failed to stop pump: {}", e));
                });
            } else {
                pump_off_timer.run(|| {
                    pump_switch
                        .borrow_mut()
                        .switch_on(connection_manager.mqtt_client())
                        .unwrap_or_else(|e| log::warn!("Failed to start pump: {}", e));
                });
            }
            let new_pump_is_on = pump_switch.borrow().is_on();
            if new_pump_is_on != pump_is_on {
                if new_pump_is_on {
                    pump_on_timer.reset();
                } else {
                    pump_off_timer.reset();
                }
            }
        }

        let pump_on = pump_switch.borrow().is_on();
        let pump_enabled = enable_pump_switch.borrow().is_on();
        if pump_on && pump_enabled {
            pump.borrow_mut().switch_on();
        } else {
            pump.borrow_mut().switch_off();
        }

        let connection_state = if connection_manager.mqtt_client().is_some() {
            hardware::ConnectionState::WifiMqttConnected
        } else {
            hardware::ConnectionState::Disconnected
        };
        led_group.display_pump_state(pump_on, pump_enabled);
        led_group.display_alert_state(pump_enabled, water_level_sensor.state(), temperature_error);
        led_group.display_connection_state(connection_state);

        FreeRtos::delay_ms(10);
    }
}

fn run_captive_portal_if_needed(
    nvs_config_manager: &NvsManager<ConfigKey>,
    nvs_state_manager: &NvsManager<StateKey>,
    wifi_manager: &mut WifiManager,
) {
    let force_portal = nvs_state_manager
        .load_property(StateKey::ForcePortal)
        .map(|v| v == "1")
        .unwrap_or(false);
    if nvs_config_manager.all_properties_set() && !force_portal {
        return;
    }
    log::info!("Starting captive portal...");
    wifi_manager.to_access_point_mode("Plant Tower Rust");
    let ip_address = wifi_manager.ip_address();
    log::info!("Access point IP address: {}", ip_address);
    let keys = ConfigKey::all_variants()
        .iter()
        .map(|k| k.key().to_string())
        .collect();
    let captive_portal = CaptivePortal::new(ip_address, keys);
    match captive_portal.run(5) {
        Ok(config) => {
            nvs_config_manager.store_properties(config);
            log::info!("Configuration saved to NVS.");
        }
        Err(CaptivePortalTimeout) => {
            log::warn!("Captive portal timed out. Falling back to stored configuration.");
        }
    }
    nvs_state_manager.store_property(StateKey::ForcePortal, "0");
}

fn create_sensors() -> (
    Rc<RefCell<mqtt::Sensor<f32>>>,
    Rc<RefCell<mqtt::Sensor<bool>>>,
) {
    let temperature = Rc::new(RefCell::new(mqtt::Sensor::new(
        Components::TemperatureSensor.to_string(),
        String::from("Temperature"),
        HashMap::new(),
        mqtt::SensorKind::Measurement {
            device_class: mqtt::DeviceClass::Temperature,
            unit: String::from("°C"),
            value_template: String::from("{{ value_json.temperature }}"),
        },
    )));
    let water_level = Rc::new(RefCell::new(mqtt::Sensor::<bool>::new(
        Components::WaterLevelSensor.to_string(),
        String::from("Water level low"),
        HashMap::from([(String::from("icon"), String::from("mdi:water-alert"))]),
        mqtt::SensorKind::Binary {
            value_template: String::from("{{ value_json.state }}"),
        },
    )));
    (temperature, water_level)
}

fn create_pump(
    pin: impl esp_idf_hal::gpio::OutputPin + 'static,
) -> (
    Rc<RefCell<mqtt::Switch>>,
    Rc<RefCell<mqtt::Switch>>,
    Rc<RefCell<hardware::DigitalOutput<'static>>>,
) {
    let enable_pump_switch = Rc::new(RefCell::new(mqtt::Switch::new(
        Components::EnablePumpSwitch.to_string(),
        String::from("Enable Pump"),
        HashMap::from([(String::from("icon"), String::from("mdi:power"))]),
    )));
    let pump_switch = Rc::new(RefCell::new(mqtt::Switch::new(
        Components::PumpSwitch.to_string(),
        String::from("Pump"),
        HashMap::from([(String::from("icon"), String::from("mdi:pump"))]),
    )));
    let pump = Rc::new(RefCell::new(hardware::DigitalOutput::new(pin)));
    (enable_pump_switch, pump_switch, pump)
}

fn setup_network(
    nvs_config_manager: &NvsManager<ConfigKey>,
    wifi_manager: &mut WifiManager,
    enable_pump_switch: &Rc<RefCell<mqtt::Switch>>,
    pump_switch: &Rc<RefCell<mqtt::Switch>>,
    temperature_sensor: &Rc<RefCell<mqtt::Sensor<f32>>>,
    water_level_sensor: &Rc<RefCell<mqtt::Sensor<bool>>>,
) -> (Option<mqtt::Device>, Option<MqttCredentials>) {
    let Ok(config) = nvs_config_manager.load_all_properties() else {
        return (None, None);
    };
    if let Err(e) = wifi_manager.to_client_mode(
        config.get(ConfigKey::WifiSsid),
        config.get(ConfigKey::WifiPassword),
    ) {
        log::warn!("Initial WiFi connection failed: {}", e);
    }
    let mut tower = mqtt::Device::new(
        config.get(ConfigKey::MqttDevId).to_string(),
        config.get(ConfigKey::MqttDevName).to_string(),
        String::from("Myself"),
    );
    tower.register_actuator(Rc::clone(enable_pump_switch) as Rc<RefCell<dyn ActuatorComponent>>);
    tower.register_actuator(Rc::clone(pump_switch) as Rc<RefCell<dyn ActuatorComponent>>);
    tower.register_sensor(Rc::clone(temperature_sensor) as Rc<RefCell<dyn SensorComponent>>);
    tower.register_sensor(Rc::clone(water_level_sensor) as Rc<RefCell<dyn SensorComponent>>);
    let credentials = MqttCredentials {
        user: config.get(ConfigKey::MqttUser).to_string(),
        password: config.get(ConfigKey::MqttPassword).to_string(),
        host: config.get(ConfigKey::MqttHost).to_string(),
        port: config.get(ConfigKey::MqttPort).to_string(),
    };
    (Some(tower), Some(credentials))
}
