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
    mod led;
    pub use led::Led;

    mod led_group;
    pub use led_group::{ConnectionState, LedGroup};

    mod digital_output;
    pub use digital_output::DigitalOutput;

    mod digital_input;
    pub use digital_input::DigitalInput;

    mod nvs_manager;
    pub use nvs_manager::{LoadedConfig, NvsKey, NvsManager, PropertyNotSet};

    mod mock_sensor;
    pub use mock_sensor::MockSensor;

    mod onewire_temperature_sensor;
    pub use onewire_temperature_sensor::OneWireTemperatureSensor;
}

pub mod mqtt {
    mod setup;
    pub use setup::setup;

    mod device;
    pub use device::{ActuatorComponent, Device, SensorComponent};

    mod sensor;
    pub use sensor::{DeviceClass, Sensor, SensorKind};

    mod switch;
    pub use switch::Switch;
}

pub mod utils {
    mod timer;
    pub use timer::Timer;
}
