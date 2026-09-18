use crate::interface::Switchable;
use crate::mqtt::{device::MqttConfig, ActuatorComponent};
use anyhow::Result;
use esp_idf_hal::sys::EspError;
use esp_idf_svc::mqtt::client::EspMqttClient;
use esp_idf_svc::mqtt::client::QoS;
use json::object;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

pub struct Switch {
    mqtt_config: MqttConfig,
    command_topic: String,
    additional_discovery_config: HashMap<String, String>,
    is_on: bool,
    listeners: Vec<Rc<RefCell<dyn Switchable>>>,
}

impl Switch {
    pub fn new(
        unique_id: String,
        name: String,
        additional_discovery_config: HashMap<String, String>,
    ) -> Self {
        let platfrom = String::from("switch");
        let command_topic = format!("homeassistant/{}/{}/set", platfrom, unique_id);
        let mqtt_config = MqttConfig::new(unique_id, name, platfrom);
        Self {
            mqtt_config,
            command_topic,
            additional_discovery_config,
            is_on: false,
            listeners: Vec::new(),
        }
    }

    pub fn register(&mut self, listener: Rc<RefCell<dyn Switchable>>) {
        self.listeners.push(listener);
    }

    pub fn is_on(&self) -> bool {
        self.is_on
    }

    pub fn switch_on(
        &mut self,
        maybe_mqtt_client: Option<&mut EspMqttClient>,
    ) -> Result<(), EspError> {
        if let Some(mqtt_client) = maybe_mqtt_client {
            self.send_state(mqtt_client, SwitchState::On)?;
        }
        self.is_on = true;
        self.update_listeners();
        Ok(())
    }

    pub fn switch_off(
        &mut self,
        maybe_mqtt_client: Option<&mut EspMqttClient>,
    ) -> Result<(), EspError> {
        if let Some(mqtt_client) = maybe_mqtt_client {
            self.send_state(mqtt_client, SwitchState::Off)?;
        }
        self.is_on = false;
        self.update_listeners();
        Ok(())
    }

    pub fn toggle(
        &mut self,
        maybe_mqtt_client: Option<&mut EspMqttClient>,
    ) -> Result<(), EspError> {
        let new_state = if self.is_on {
            SwitchState::Off
        } else {
            SwitchState::On
        };
        if let Some(mqtt_client) = maybe_mqtt_client {
            self.send_state(mqtt_client, new_state)?;
        }
        self.is_on = !self.is_on;
        self.update_listeners();
        Ok(())
    }

    fn send_state(
        &self,
        mqtt_client: &mut EspMqttClient,
        state: SwitchState,
    ) -> Result<(), EspError> {
        mqtt_client.publish(
            self.mqtt_config.state_topic().as_str(),
            QoS::AtLeastOnce,
            true,
            state.to_string().as_bytes(),
        )?;
        Ok(())
    }

    fn update_listeners(&self) {
        if self.is_on {
            self.listeners
                .iter()
                .for_each(|listeners| listeners.borrow_mut().switch_on());
        } else {
            self.listeners
                .iter()
                .for_each(|listeners| listeners.borrow_mut().switch_off());
        }
    }
}

impl ActuatorComponent for Switch {
    fn unique_id(&self) -> &String {
        self.mqtt_config.unique_id()
    }

    fn state_topic(&self) -> &String {
        self.mqtt_config.state_topic()
    }

    fn command_topic(&self) -> Option<&String> {
        Some(&self.command_topic)
    }

    fn process_message(&mut self, mqtt_client: &mut EspMqttClient, payload: &str) -> Result<()> {
        if payload.eq(&SwitchState::On.to_string()) {
            self.switch_on(Some(mqtt_client))?;
        } else if payload.eq(&SwitchState::Off.to_string()) {
            self.switch_off(Some(mqtt_client))?;
        } else {
            log::warn!("Ignoring unknown payload {}", payload);
        }
        Ok(())
    }

    fn to_discovery_payload(&self) -> json::JsonValue {
        let mut message = object! {
            platform: self.mqtt_config.platform().as_str(),
            name: self.mqtt_config.name().as_str(),
            unique_id: self.mqtt_config.unique_id().as_str(),
            state_topic: self.mqtt_config.state_topic().as_str(),
            command_topic: self.command_topic.as_str()
        };
        for (key, value) in &self.additional_discovery_config {
            message[key] = value.as_str().into();
        }
        message
    }
}

#[derive(strum_macros::Display)]
enum SwitchState {
    #[strum(serialize = "ON")]
    On,
    #[strum(serialize = "OFF")]
    Off,
}
