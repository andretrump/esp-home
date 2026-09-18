use anyhow::Result;
use std::collections::HashMap;
use std::net::Ipv4Addr;

use crate::captive_portal::dns_server::DnsServer;
use crate::captive_portal::http_server::{CaptivePortalTimeout, HttpServer};

pub struct CaptivePortal {
    properties: Vec<String>,
    ip: Ipv4Addr,
}

impl CaptivePortal {
    pub fn new(ip: Ipv4Addr, properties: Vec<String>) -> Self {
        Self { properties, ip }
    }

    pub fn run(
        &self,
        timeout_minutes: u64,
    ) -> Result<HashMap<String, String>, CaptivePortalTimeout> {
        let dns_server = DnsServer::new(self.ip);
        let dns_server_handle = dns_server.start();
        let mut http_server = HttpServer::new(self.ip, self.properties.clone());
        let config = http_server.run_until_config_received(timeout_minutes)?;
        dns_server_handle.stop();
        Ok(config)
    }
}
