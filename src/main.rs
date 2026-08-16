use esp_idf_hal::delay::FreeRtos;
use esp_idf_hal::peripherals::Peripherals;
use esp_idf_svc::eventloop::EspSystemEventLoop;
use esp_idf_svc::nvs::EspDefaultNvsPartition;
use plant_tower_rs::captive_portal::CaptivePortal;
use plant_tower_rs::hardware::{self, NvsManager};
use plant_tower_rs::interface::Switchable;
use plant_tower_rs::mqtt::{self, Component};
use plant_tower_rs::wifi::WifiManager;
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

fn main() {
    esp_idf_svc::sys::link_patches();
    esp_idf_svc::log::EspLogger::initialize_default();
    let peripherals = Peripherals::take().expect("Failed to initialize peripherals");
    let sys_loop = EspSystemEventLoop::take().expect("Failed to initialize system loop");
    let nvs_default_partition = EspDefaultNvsPartition::take().expect("Failed to initialize NVS");

    let config_properties = HashMap::from([
        (String::from("mqtt_host"), 512),
        (String::from("mqtt_port"), 512),
        (String::from("mqtt_user"), 512),
        (String::from("mqtt_password"), 512),
        (String::from("wifi_ssid"), 512),
        (String::from("wifi_password"), 512),
        (String::from("mqtt_dev_name"), 512),
        (String::from("mqtt_dev_id"), 512),
    ]);
    let mut nvs_manager = NvsManager::new(
        nvs_default_partition.clone(),
        String::from("PLANT_TWR_CFG"),
        config_properties.clone(),
    )
    .expect("Failed to create NvsManager");

    let mut wifi_manager =
        WifiManager::new(peripherals.modem, sys_loop, nvs_default_partition.clone())
            .expect("Failed to initialize Wifi");
    wifi_manager
        .to_access_point_mode("Plant Tower Rust")
        .expect("Failed to start access point");

    let ip_address = wifi_manager
        .ip_address()
        .expect("Failed to get own IP address");
    log::info!("AP IP address: {}", ip_address);

    let captive_portal =
        CaptivePortal::new(ip_address, config_properties.keys().cloned().collect())
            .expect("Failed to bootstrap captive portal");
    captive_portal
        .run(&mut nvs_manager)
        .expect("Failed to run captive portal");

    let config = nvs_manager
        .load_all_properties()
        .expect("Failed to load config from NVS");

    wifi_manager
        .to_client_mode(
            config
                .get("wifi_ssid")
                .expect("WiFi SSID is not set")
                .as_str(),
            config
                .get("wifi_password")
                .expect("WiFi password is not set")
                .as_str(),
        )
        .expect("Failed to setup Wifi");

    let (mut mqtt_client, receiver) = mqtt::setup(
        config.get("mqtt_user").expect("MQTT user not set").as_str(),
        config
            .get("mqtt_password")
            .expect("MQTT password not set")
            .as_str(),
        config.get("mqtt_host").expect("MQTT host not set").as_str(),
        config.get("mqtt_port").expect("MQTT port not set").as_str(),
    )
    .expect("Failed to setup MQTT");

    let mut plant_tower = mqtt::Device::new(
        config
            .get("mqtt_dev_id")
            .expect("MQTT device name not set")
            .clone(),
        config
            .get("mqtt_dev_name")
            .expect("MQTT device ID not set")
            .clone(),
        String::from("Myself"),
    );
    let pump_switch = Rc::new(RefCell::new(mqtt::Switch::new(
        Components::PumpSwitch.to_string(),
        String::from("Pump"),
        HashMap::from([(String::from("icon"), String::from("mdi:pump"))]),
    )));
    plant_tower.register(Rc::clone(&pump_switch) as Rc<RefCell<dyn Component>>);

    let pump = Rc::new(RefCell::new(hardware::Pump::new(peripherals.pins.gpio26)));
    pump_switch
        .borrow_mut()
        .register(pump as Rc<RefCell<dyn Switchable>>);
    plant_tower.send_discovery_message(&mut mqtt_client);
    plant_tower.subscribe_command_topics(&mut mqtt_client);

    pump_switch
        .borrow_mut()
        .switch_on(&mut mqtt_client)
        .unwrap_or_else(|err| log::warn!("Failed to switch on pump: {}", err));
    let mut last_switched = Instant::now();

    loop {
        if let Ok((topic, payload)) = receiver.try_recv() {
            plant_tower.dispatch_event(&mut mqtt_client, topic.as_str(), payload.as_str());
        }
        if last_switched.elapsed() >= Duration::from_secs(10) {
            pump_switch
                .borrow_mut()
                .toggle(&mut mqtt_client)
                .unwrap_or_else(|err| log::warn!("Failed to toggle pump switch: {}", err));
            last_switched = Instant::now();
        }
        FreeRtos::delay_ms(10);
    }
}
