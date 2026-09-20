use crate::connectivity::WifiManager;
use crate::mqtt::{self, MqttEvent};
use esp_idf_svc::mqtt::client::EspMqttClient;
use std::sync::mpsc::Receiver;
use std::time::{Duration, Instant};

pub struct MqttCredentials {
    pub user: String,
    pub password: String,
    pub host: String,
    pub port: String,
}

pub struct ConnectionManager {
    wifi_manager: WifiManager,
    device: Option<mqtt::Device>,
    mqtt: Option<(EspMqttClient<'static>, Receiver<MqttEvent>)>,
    credentials: Option<MqttCredentials>,
    wifi_was_connected: bool,
    mqtt_connected: bool,
    just_reconnected: bool,
    last_reconnect_attempt: Instant,
}

impl ConnectionManager {
    pub fn new(
        wifi_manager: WifiManager,
        device: Option<mqtt::Device>,
        credentials: Option<MqttCredentials>,
    ) -> Self {
        let wifi_was_connected = wifi_manager.is_connected();
        let mut cm = Self {
            wifi_manager,
            device,
            mqtt: None,
            credentials,
            wifi_was_connected,
            mqtt_connected: false,
            just_reconnected: false,
            last_reconnect_attempt: Instant::now(),
        };
        if wifi_was_connected {
            cm.setup_mqtt();
        }
        cm
    }

    pub fn tick(&mut self) {
        self.handle_wifi_state();
    }

    fn handle_wifi_state(&mut self) {
        let is_connected = self.wifi_manager.is_connected();
        if !is_connected {
            if self.wifi_was_connected {
                log::warn!("WiFi connection lost.");
                self.mqtt = None;
                self.mqtt_connected = false;
            }
            self.try_reconnect();
        } else if !self.wifi_was_connected {
            log::info!("WiFi reconnected.");
            self.setup_mqtt();
        }
        self.wifi_was_connected = is_connected;
    }

    fn try_reconnect(&mut self) {
        if self.credentials.is_some()
            && self.last_reconnect_attempt.elapsed() >= Duration::from_secs(5)
        {
            if let Err(e) = self.wifi_manager.reconnect() {
                log::warn!("Reconnect failed: {}", e);
            }
            self.last_reconnect_attempt = Instant::now();
        }
    }

    pub fn next_message(&mut self) -> Option<(String, String)> {
        loop {
            let event = self.mqtt.as_ref().and_then(|(_, r)| r.try_recv().ok())?;
            match event {
                MqttEvent::Connected => self.on_mqtt_reconnected(),
                MqttEvent::Disconnected => {
                    log::warn!("MQTT disconnected.");
                    self.mqtt_connected = false;
                }
                MqttEvent::Message(topic, payload) => return Some((topic, payload)),
            }
        }
    }

    fn on_mqtt_reconnected(&mut self) {
        log::info!("MQTT reconnected, re-subscribing to command topics.");
        self.mqtt_connected = true;
        self.just_reconnected = true;
        let Some((client, _)) = self.mqtt.as_mut() else {
            return;
        };
        let Some(device) = self.device.as_mut() else {
            return;
        };
        device.send_discovery_message(client);
        device.subscribe_command_topics(client);
    }

    pub fn take_reconnected(&mut self) -> bool {
        std::mem::take(&mut self.just_reconnected)
    }

    pub fn mqtt_client(&mut self) -> Option<&mut EspMqttClient<'static>> {
        if !self.mqtt_connected {
            return None;
        }
        self.mqtt.as_mut().map(|(c, _)| c)
    }

    fn setup_mqtt(&mut self) {
        let Some(credentials) = &self.credentials else {
            return;
        };
        self.mqtt_connected = false;
        let (client, receiver) = mqtt::setup(
            &credentials.user,
            &credentials.password,
            &credentials.host,
            &credentials.port,
        );
        self.mqtt = Some((client, receiver));
    }
}
