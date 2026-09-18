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
use plant_tower_rs::mqtt::{self, Component};
use plant_tower_rs::nvs_keys;
use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt;
use std::rc::Rc;
use std::time::{Duration, Instant};

enum Components {
    PumpSwitch,
}

impl fmt::Display for Components {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Components::PumpSwitch => write!(f, "plant_tower_rs_pump_switch"),
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
        hardware::DigitalInput::new(peripherals.pins.gpio32, true, true, 20);
    let pump = Rc::new(RefCell::new(hardware::Pump::new(peripherals.pins.gpio26)));
    let pump_switch = Rc::new(RefCell::new(mqtt::Switch::new(
        Components::PumpSwitch.to_string(),
        String::from("Pump"),
        HashMap::from([(String::from("icon"), String::from("mdi:pump"))]),
    )));
    pump_switch
        .borrow_mut()
        .register(pump as Rc<RefCell<dyn Switchable>>);

    let (device, credentials) = setup_network(&nvs_config_manager, &mut wifi_manager, &pump_switch);
    let mut connection_manager = ConnectionManager::new(wifi_manager, device, credentials);

    pump_switch
        .borrow_mut()
        .switch_on(connection_manager.mqtt_client())
        .unwrap_or_else(|err| log::warn!("Failed to switch on pump: {}", err));
    let mut last_switched = Instant::now();

    loop {
        if reset_connectivity_button.true_at_least_for(5) {
            nvs_state_manager.store_property(StateKey::ForcePortal, "1");
            unsafe { sys::esp_restart() }
        }
        connection_manager.tick();

        if last_switched.elapsed() >= Duration::from_secs(10) {
            pump_switch
                .borrow_mut()
                .toggle(connection_manager.mqtt_client())
                .unwrap_or_else(|e| log::warn!("Toggle failed: {}", e));
            last_switched = Instant::now();
        }

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
    tower.register(Rc::clone(pump_switch) as Rc<RefCell<dyn Component>>);
    let credentials = MqttCredentials {
        user: config.get(ConfigKey::MqttUser).to_string(),
        password: config.get(ConfigKey::MqttPassword).to_string(),
        host: config.get(ConfigKey::MqttHost).to_string(),
        port: config.get(ConfigKey::MqttPort).to_string(),
    };
    (Some(tower), Some(credentials))
}
