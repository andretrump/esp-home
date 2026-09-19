use esp_idf_hal::delay::FreeRtos;
use esp_idf_hal::peripherals::Peripherals;
use esp_idf_svc::eventloop::EspSystemEventLoop;
use esp_idf_svc::nvs::EspDefaultNvsPartition;
use esp_idf_svc::sys;
use plant_tower_rs::captive_portal::http_server::CaptivePortalTimeout;
use plant_tower_rs::captive_portal::CaptivePortal;
use plant_tower_rs::connectivity::{ConnectionManager, MqttCredentials, WifiManager};
use plant_tower_rs::hardware::{self, NvsKey, NvsManager};
use plant_tower_rs::interface::Switchable;
use plant_tower_rs::mqtt::{self, ActuatorComponent, SensorComponent};
use plant_tower_rs::nvs_keys;
use plant_tower_rs::utils::Timer;
use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt;
use std::rc::Rc;
enum Components {
    PumpSwitch,
    TemperatureSensor,
    WaterLevelSensor,
}

impl fmt::Display for Components {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
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

    let mut reset_connectivity_button =
        hardware::DigitalInput::new(peripherals.pins.gpio19, true, true, 20);

    let mock_sensor = hardware::MockSensor::<f32>::new(18.0, 22.0);
    let mqtt_temperature_sensor = Rc::new(RefCell::new(mqtt::Sensor::new(
        Components::TemperatureSensor.to_string(),
        String::from("Temperature"),
        HashMap::new(),
        mqtt::SensorKind::Measurement {
            device_class: mqtt::DeviceClass::Temperature,
            unit: String::from("°C"),
            value_template: String::from("{{ value_json.temperature }}"),
        },
    )));

    let mut water_level_sensor =
        hardware::DigitalInput::new(peripherals.pins.gpio22, true, true, 20);
    let mqtt_water_level_sensor = Rc::new(RefCell::new(mqtt::Sensor::<bool>::new(
        Components::WaterLevelSensor.to_string(),
        String::from("Water level low"),
        HashMap::from([(String::from("icon"), String::from("mdi:water-alert"))]),
        mqtt::SensorKind::Binary {
            value_template: String::from("{{ value_json.state }}"),
        },
    )));

    let pump = Rc::new(RefCell::new(hardware::Pump::new(peripherals.pins.gpio26)));
    let pump_switch = Rc::new(RefCell::new(mqtt::Switch::new(
        Components::PumpSwitch.to_string(),
        String::from("Pump"),
        HashMap::from([(String::from("icon"), String::from("mdi:pump"))]),
    )));
    pump_switch
        .borrow_mut()
        .register(pump as Rc<RefCell<dyn Switchable>>);

    let (device, credentials) = setup_network(
        &nvs_config_manager,
        &mut wifi_manager,
        &pump_switch,
        &mqtt_temperature_sensor,
        &mqtt_water_level_sensor,
    );
    let mut connection_manager = ConnectionManager::new(wifi_manager, device, credentials);

    pump_switch
        .borrow_mut()
        .switch_on(connection_manager.mqtt_client())
        .unwrap_or_else(|err| log::warn!("Failed to switch on pump: {}", err));

    let mut every_10_secs = Timer::new(10);

    loop {
        connection_manager.tick();

        if reset_connectivity_button.true_at_least_for(5) {
            nvs_state_manager.store_property(StateKey::ForcePortal, "1");
            unsafe { sys::esp_restart() }
        }

        every_10_secs.run(|| {
            mqtt_temperature_sensor
                .borrow_mut()
                .set_value(mock_sensor.get_value(), connection_manager.mqtt_client());

            mqtt_water_level_sensor.borrow_mut().set_value(
                water_level_sensor.refresh_state().state(),
                connection_manager.mqtt_client(),
            );

            pump_switch
                .borrow_mut()
                .toggle(connection_manager.mqtt_client())
                .unwrap_or_else(|e| log::warn!("Toggle failed: {}", e));
        });

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

fn setup_network(
    nvs_config_manager: &NvsManager<ConfigKey>,
    wifi_manager: &mut WifiManager,
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
