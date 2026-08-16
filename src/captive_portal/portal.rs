use anyhow::Result;
use std::net::Ipv4Addr;

use crate::captive_portal::dns_server::DnsServer;
use crate::captive_portal::http_server::HttpServer;
use crate::hardware::NvsManager;

pub struct CaptivePortal {
    properties: Vec<String>,
    ip: Ipv4Addr,
}

impl CaptivePortal {
    pub fn new(ip: Ipv4Addr, properties: Vec<String>) -> Result<Self> {
        Ok(Self { properties, ip })
    }

    pub fn run(&self, nvs_manager: &mut NvsManager) -> Result<()> {
        let dns_server = DnsServer::new(self.ip);
        let dns_server_handle = dns_server.start();
        let mut http_server = HttpServer::new(self.ip, self.properties.clone())?;
        let config = http_server.run_until_config_received()?;
        nvs_manager.store_properties(config)?;
        dns_server_handle.stop();
        Ok(())
    }
}
