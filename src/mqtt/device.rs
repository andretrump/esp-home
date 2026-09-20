use anyhow::Result;
use esp_idf_svc::mqtt::client::EspMqttClient;
use esp_idf_svc::mqtt::client::QoS;
use json::{object, JsonValue};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

pub struct Device {
    mqtt_config: MqttConfig,
    discovery_topic: String,
    manufacturer: String,
    actuator_components: HashMap<String, Rc<RefCell<dyn ActuatorComponent>>>,
    sensor_payloads: HashMap<String, JsonValue>,
}

impl Device {
    pub fn new(unique_id: String, name: String, manufacturer: String) -> Self {
        let platform = String::from("device");
        let discovery_topic = format!("homeassistant/{}/{}/config", platform, unique_id);
        let mqtt_config = MqttConfig::new(unique_id, name, platform);
        Self {
            mqtt_config,
            discovery_topic,
            manufacturer,
            actuator_components: HashMap::new(),
            sensor_payloads: HashMap::new(),
        }
    }

    pub fn register_actuator(&mut self, component: Rc<RefCell<dyn ActuatorComponent>>) {
        let unique_id = component.borrow().unique_id().clone();
        self.actuator_components.insert(unique_id, component);
    }

    pub fn register_sensor(&mut self, component: &dyn SensorComponent) {
        self.sensor_payloads
            .insert(component.unique_id().clone(), component.to_discovery_payload());
    }

    pub fn send_discovery_message(&mut self, mqtt_client: &mut EspMqttClient) {
        let payload = self.build_discovery_payload();
        log::info!(
            "Sending discovery message to topic {} with payload\n{}",
            self.discovery_topic,
            json::stringify_pretty(payload.clone(), 2)
        );
        let payload = json::stringify(payload);
        mqtt_client
            .publish(
                &self.discovery_topic,
                QoS::ExactlyOnce,
                true,
                payload.as_bytes(),
            )
            .expect("Failed to send MQTT discovery message");
    }

    fn build_discovery_payload(&self) -> JsonValue {
        let mut payload = object! {
            state_topic: self.mqtt_config.state_topic.as_str(),
            qos: 2,
            dev: {
                ids: self.mqtt_config.unique_id.as_str(),
                name: self.mqtt_config.name.as_str(),
                mf: self.manufacturer.as_str(),
                mdl: self.mqtt_config.name.as_str()
            },
            o: {
                name: self.mqtt_config.name.as_str()
            },
            cmps: {}
        };
        self.actuator_components.values().for_each(|component| {
            let component_payload = component.borrow().to_discovery_payload();
            payload["cmps"][component.borrow().unique_id()] = component_payload;
        });
        self.sensor_payloads.iter().for_each(|(unique_id, component_payload)| {
            payload["cmps"][unique_id.as_str()] = component_payload.clone();
        });
        payload
    }

    pub fn subscribe_command_topics(&self, mqtt_client: &mut EspMqttClient) {
        for component in self.actuator_components.values() {
            if let Some(topic) = component.borrow().command_topic().cloned() {
                mqtt_client
                    .subscribe(&topic, QoS::ExactlyOnce)
                    .unwrap_or_else(|_| panic!("Failed to subscribe to command topic {}", topic));
            }
        }
    }

    pub fn dispatch_event(&mut self, mqtt_client: &mut EspMqttClient, topic: &str, payload: &str) {
        let unique_id = match topic.split("/").nth(2) {
            Some(unique_id) => String::from(unique_id),
            None => {
                log::warn!("Topic {} does not match expected pattern", topic);
                return;
            }
        };
        match self.actuator_components.get_mut(&unique_id) {
            Some(component) => match component.borrow_mut().process_message(mqtt_client, payload) {
                Ok(_) => (),
                Err(err) => log::warn!(
                    "Component {} failed to process event with payload\n{}\nwith error {}",
                    unique_id,
                    payload,
                    err
                ),
            },
            None => log::warn!(
                "Received event with unknown unique id {} in topic {}. Payload is\n{}",
                unique_id,
                topic,
                json::stringify_pretty(payload, 2),
            ),
        }
    }
}

pub struct MqttConfig {
    unique_id: String,
    name: String,
    platform: String,
    state_topic: String,
}

impl MqttConfig {
    pub fn new(unique_id: String, name: String, platform: String) -> Self {
        let state_topic = format!("homeassistant/{}/{}/state", platform, unique_id);
        Self {
            unique_id,
            name,
            platform,
            state_topic,
        }
    }

    pub fn unique_id(&self) -> &String {
        &self.unique_id
    }

    pub fn name(&self) -> &String {
        &self.name
    }

    pub fn platform(&self) -> &String {
        &self.platform
    }

    pub fn state_topic(&self) -> &String {
        &self.state_topic
    }
}

pub trait ActuatorComponent {
    fn unique_id(&self) -> &String;
    fn state_topic(&self) -> &String;
    fn command_topic(&self) -> Option<&String>;
    fn to_discovery_payload(&self) -> JsonValue;
    fn process_message(&mut self, mqtt_client: &mut EspMqttClient, payload: &str) -> Result<()>;
}

pub trait SensorComponent {
    fn unique_id(&self) -> &String;
    fn state_topic(&self) -> &String;
    fn to_discovery_payload(&self) -> JsonValue;
}
