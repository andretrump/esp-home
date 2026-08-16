use anyhow::Result;
use esp_idf_hal::modem::WifiModemPeripheral;
use esp_idf_svc::ipv4;
use esp_idf_svc::netif::{EspNetif, NetifConfiguration};
use esp_idf_svc::{
    eventloop::EspSystemEventLoop,
    nvs::EspDefaultNvsPartition,
    wifi::{
        AccessPointConfiguration, AuthMethod, BlockingWifi, ClientConfiguration, Configuration,
        EspWifi, Protocol,
    },
};
use std::net::Ipv4Addr;
use std::sync::atomic::{AtomicU32, Ordering};

static NETIF_KEY_CTR: AtomicU32 = AtomicU32::new(0);

pub struct WifiManager {
    access_point_ip: Ipv4Addr,
    wifi: BlockingWifi<EspWifi<'static>>,
}

impl WifiManager {
    pub fn new<M>(
        modem: M,
        sysloop: EspSystemEventLoop,
        nvs_partition: EspDefaultNvsPartition,
    ) -> Result<Self>
    where
        M: WifiModemPeripheral + 'static,
    {
        let esp_wifi = EspWifi::new(modem, sysloop.clone(), Some(nvs_partition))?;
        let wifi = BlockingWifi::wrap(esp_wifi, sysloop)?;
        let wifi_manager = Self {
            access_point_ip: Ipv4Addr::new(192, 169, 71, 1),
            wifi,
        };
        Ok(wifi_manager)
    }

    pub fn to_access_point_mode(&mut self, ssid: &str) -> Result<()> {
        if self.wifi.is_started()? {
            self.wifi.stop()?;
        }

        let network_interface = self.build_network_interface()?;
        self.wifi.wifi_mut().swap_netif_ap(network_interface)?;

        let access_point_config = self.build_access_point_config(ssid);
        self.wifi.set_configuration(&access_point_config)?;

        self.wifi.start()?;
        Ok(())
    }

    fn build_network_interface(&self) -> Result<EspNetif> {
        let key_n = NETIF_KEY_CTR.fetch_add(1, Ordering::Relaxed);
        let key = format!("AP_{key_n}");
        let interface = EspNetif::new_with_conf(&NetifConfiguration {
            ip_configuration: Some(ipv4::Configuration::Router(ipv4::RouterConfiguration {
                subnet: ipv4::Subnet {
                    gateway: self.access_point_ip,
                    mask: ipv4::Mask(24),
                },
                dhcp_enabled: true,
                dns: Some(self.access_point_ip),
                secondary_dns: None,
            })),
            key: key.as_str().try_into().unwrap(),
            ..NetifConfiguration::wifi_default_router()
        })?;
        Ok(interface)
    }

    fn build_access_point_config(&self, ssid: &str) -> Configuration {
        Configuration::AccessPoint(AccessPointConfiguration {
            ssid: ssid.try_into().unwrap(),
            ssid_hidden: false,
            channel: 1,
            secondary_channel: None,
            protocols: Protocol::P802D11B | Protocol::P802D11BG | Protocol::P802D11BGN,
            auth_method: AuthMethod::None,
            password: "".try_into().unwrap(),
            max_connections: 255,
        })
    }

    pub fn to_client_mode(&mut self, ssid: &str, password: &str) -> Result<()> {
        if self.wifi.is_started()? {
            self.wifi.stop()?;
        }

        let mut auth_method = AuthMethod::WPA2Personal;
        if password.is_empty() {
            auth_method = AuthMethod::None;
            log::info!("Wifi password is empty");
        }
        self.wifi
            .set_configuration(&Configuration::Client(ClientConfiguration::default()))?;

        log::info!("Starting wifi...");
        self.wifi.start()?;

        log::info!("Scanning...");
        let access_point_infos = self.wifi.scan()?;
        let ours = access_point_infos.into_iter().find(|a| a.ssid == ssid);
        let channel = if let Some(ours) = ours {
            log::info!(
                "Found configured access point {} on channel {}",
                ssid,
                ours.channel
            );
            Some(ours.channel)
        } else {
            log::info!(
                "Configured access point {} not found during scanning, will go with unknown channel",
                ssid
            );
            None
        };

        self.wifi
            .set_configuration(&Configuration::Client(ClientConfiguration {
                ssid: ssid
                    .try_into()
                    .expect("Could not parse the given SSID into WiFi config"),
                password: password
                    .try_into()
                    .expect("Could not parse the given password into WiFi config"),
                channel,
                auth_method,
                ..Default::default()
            }))?;

        log::info!("Connecting wifi...");
        self.wifi.connect()?;

        log::info!("Waiting for DHCP lease...");
        self.wifi.wait_netif_up()?;

        Ok(())
    }

    pub fn ip_address(&self) -> Result<Ipv4Addr> {
        let ip_info = self.wifi.wifi().ap_netif().get_ip_info()?;
        Ok(std::net::Ipv4Addr::from(ip_info.ip.octets()))
    }
}
