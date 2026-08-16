pub mod captive_portal {
    pub mod portal;
    pub use portal::CaptivePortal;
    pub mod dns_server;
    pub mod http_server;
}

pub mod hardware {
    mod digital_output;
    pub use digital_output::DigitalOutput;

    mod nvs_manager;
    pub use nvs_manager::NvsManager;

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

pub mod wifi;
