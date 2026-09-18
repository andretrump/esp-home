use crate::mqtt;
use crate::connectivity::WifiManager;
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
    mqtt: Option<(EspMqttClient<'static>, Receiver<(String, String)>)>,
    credentials: Option<MqttCredentials>,
    was_connected: bool,
    last_reconnect_attempt: Instant,
}

impl ConnectionManager {
    pub fn new(
        wifi_manager: WifiManager,
        device: Option<mqtt::Device>,
        credentials: Option<MqttCredentials>,
    ) -> Self {
        let was_connected = wifi_manager.is_connected();
        let mut cm = Self {
            wifi_manager,
            device,
            mqtt: None,
            credentials,
            was_connected,
            last_reconnect_attempt: Instant::now(),
        };
        if was_connected {
            cm.setup_mqtt();
        }
        cm
    }

    pub fn tick(&mut self) {
        self.handle_wifi_state();
        self.dispatch_messages();
    }

    fn handle_wifi_state(&mut self) {
        let is_connected = self.wifi_manager.is_connected();
        if !is_connected {
            if self.was_connected {
                log::warn!("WiFi connection lost.");
                self.mqtt = None;
            }
            self.try_reconnect();
        } else if !self.was_connected {
            log::info!("WiFi reconnected.");
            self.setup_mqtt();
        }
        self.was_connected = is_connected;
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

    fn dispatch_messages(&mut self) {
        if let (Some((ref mut client, ref receiver)), Some(ref mut device)) =
            (&mut self.mqtt, &mut self.device)
        {
            if let Ok((topic, payload)) = receiver.try_recv() {
                device.dispatch_event(client, topic.as_str(), payload.as_str());
            }
        }
    }

    pub fn mqtt_client(&mut self) -> Option<&mut EspMqttClient<'static>> {
        self.mqtt.as_mut().map(|(c, _)| c)
    }

    fn setup_mqtt(&mut self) {
        let (user, password, host, port) = match &self.credentials {
            Some(c) => (c.user.clone(), c.password.clone(), c.host.clone(), c.port.clone()),
            None => return,
        };
        let Some(device) = self.device.as_mut() else { return; };
        let (mut client, receiver) = mqtt::setup(&user, &password, &host, &port);
        device.send_discovery_message(&mut client);
        device.subscribe_command_topics(&mut client);
        self.mqtt = Some((client, receiver));
    }
}
