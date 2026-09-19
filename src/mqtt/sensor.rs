use crate::mqtt::device::MqttConfig;
use crate::mqtt::device::SensorComponent;
use esp_idf_svc::mqtt::client::EspMqttClient;
use esp_idf_svc::mqtt::client::QoS;
use json::object;
use std::collections::HashMap;

pub enum SensorKind {
    Measurement {
        device_class: DeviceClass,
        unit: String,
        value_template: String,
    },
    Binary {
        value_template: String,
    },
}

#[derive(strum_macros::Display)]
pub enum DeviceClass {
    #[strum(serialize = "temperature")]
    Temperature,
    #[strum(serialize = "duration")]
    Duration,
}

pub struct Sensor<T: Into<json::JsonValue> + PartialEq + Clone> {
    mqtt_config: MqttConfig,
    additional_discovery_config: HashMap<String, String>,
    kind: SensorKind,
    value: Option<T>,
}

impl<T: Into<json::JsonValue> + PartialEq + Clone> Sensor<T> {
    pub fn new(
        unique_id: String,
        name: String,
        additional_discovery_config: HashMap<String, String>,
        kind: SensorKind,
    ) -> Self {
        let platfrom: String = String::from("sensor");
        let mqtt_config = MqttConfig::new(unique_id, name, platfrom);
        Self {
            mqtt_config,
            additional_discovery_config,
            kind,
            value: None,
        }
    }

    pub fn set_value(&mut self, value: T, maybe_mqtt_client: Option<&mut EspMqttClient>) {
        if self.value.as_ref() == Some(&value) {
            return;
        }
        self.value = Some(value.clone());
        if let Some(mqtt_client) = maybe_mqtt_client {
            let payload = self.to_state_payload(value);
            if let Err(e) = mqtt_client.publish(
                self.mqtt_config.state_topic(),
                QoS::AtLeastOnce,
                true,
                payload.as_bytes(),
            ) {
                log::warn!("Failed to publish sensor state: {}", e);
            }
        }
    }

    pub fn clear_value(&mut self, maybe_mqtt_client: Option<&mut EspMqttClient>) {
        self.value = None;
        if let Some(mqtt_client) = maybe_mqtt_client {
            if let Err(e) =
                mqtt_client.publish(self.mqtt_config.state_topic(), QoS::AtLeastOnce, true, b"")
            {
                log::warn!("Failed to clear sensor state: {}", e);
            }
        }
    }

    fn to_state_payload(&self, value: T) -> String {
        let json_value: json::JsonValue = value.into();
        match &self.kind {
            SensorKind::Measurement { device_class, .. } => {
                let mut obj = json::JsonValue::new_object();
                let key = device_class.to_string();
                obj[key.as_str()] = json_value;
                obj.to_string()
            }
            SensorKind::Binary { .. } => {
                let mut obj = json::JsonValue::new_object();
                obj["state"] = json_value;
                obj.to_string()
            }
        }
    }
}

impl<T: Into<json::JsonValue> + PartialEq + Clone> SensorComponent for Sensor<T> {
    fn unique_id(&self) -> &String {
        self.mqtt_config.unique_id()
    }

    fn state_topic(&self) -> &String {
        self.mqtt_config.state_topic()
    }

    fn to_discovery_payload(&self) -> json::JsonValue {
        let mut message = object! {
            platform: self.mqtt_config.platform().as_str(),
            name: self.mqtt_config.name().as_str(),
            unique_id: self.mqtt_config.unique_id().as_str(),
            state_topic: self.mqtt_config.state_topic().as_str(),
        };
        match &self.kind {
            SensorKind::Measurement {
                device_class,
                unit,
                value_template,
            } => {
                message["device_class"] = device_class.to_string().as_str().into();
                message["unit_of_measurement"] = unit.as_str().into();
                message["value_template"] = value_template.as_str().into();
            }
            SensorKind::Binary { value_template } => {
                message["value_template"] = value_template.as_str().into();
            }
        }
        for (key, value) in &self.additional_discovery_config {
            message[key] = value.as_str().into();
        }
        message
    }
}
