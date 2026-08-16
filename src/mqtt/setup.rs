use esp_idf_hal::sys::EspError;
use esp_idf_svc::mqtt::client::Details::Complete;
use esp_idf_svc::mqtt::client::EspMqttEvent;
use esp_idf_svc::mqtt::client::EventPayload::Received;
use esp_idf_svc::{mqtt::client::EspMqttClient, mqtt::client::MqttClientConfiguration};
use std::sync::mpsc::{channel, Receiver, Sender};

pub fn setup(
    mqtt_user: &str,
    mqtt_password: &str,
    mqtt_host: &str,
    mqtt_port: &str,
) -> Result<(EspMqttClient<'static>, Receiver<(String, String)>), EspError> {
    let broker_url = format!(
        "mqtt://{}:{}@{}:{}",
        mqtt_user, mqtt_password, mqtt_host, mqtt_port
    );
    let (sender, receiver) = channel::<(String, String)>();
    let mqtt_client = EspMqttClient::new_cb(
        &broker_url,
        &MqttClientConfiguration::default(),
        move |event| send_to_main_thread(&sender, event),
    )?;
    Ok((mqtt_client, receiver))
}

fn send_to_main_thread(sender: &Sender<(String, String)>, event: EspMqttEvent) {
    if let Received {
        topic,
        data: payload,
        details: Complete,
        ..
    } = event.payload()
    {
        let Some(topic) = topic else {
            log::warn!("Ignoring message with empty topic");
            return;
        };
        let Ok(payload) = std::str::from_utf8(payload) else {
            log::warn!("Failed to convert payload to string");
            return;
        };
        sender.send((topic.to_string(), payload.to_string())).ok();
    }
}
