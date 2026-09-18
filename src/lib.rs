pub mod connectivity {
    mod wifi;
    pub use wifi::WifiManager;

    mod connection_manager;
    pub use connection_manager::{ConnectionManager, MqttCredentials};
}

pub mod captive_portal {
    pub mod portal;
    pub use portal::CaptivePortal;
    pub mod dns_server;
    pub mod http_server;
}

pub mod hardware {
    mod digital_output;
    pub use digital_output::DigitalOutput;
    mod digital_input;
    pub use digital_input::DigitalInput;

    mod nvs_manager;
    pub use nvs_manager::LoadedConfig;
    pub use nvs_manager::NvsKey;
    pub use nvs_manager::NvsManager;
    pub use nvs_manager::PropertyNotSet;

    mod pump;
    pub use pump::Pump;
}

pub mod interface {
    mod switchable;
    pub use switchable::Switchable;
}

pub mod mqtt {
    mod setup;
    pub use setup::setup;

    mod device;
    pub use device::Component;
    pub use device::Device;

    mod switch;
    pub use switch::Switch;
}
