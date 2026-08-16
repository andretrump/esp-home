use anyhow::Result;
use esp_idf_svc::http::server::{
    Configuration as HttpConfig, EspHttpConnection, EspHttpServer, Request,
};
use esp_idf_svc::http::Method;
use esp_idf_svc::io::Write;
use std::borrow::Cow;
use std::collections::HashMap;
use std::net::Ipv4Addr;
use std::sync::mpsc::channel;
use std::sync::mpsc::Sender;

pub struct HttpServer {
    ip: Ipv4Addr,
    properties: Vec<String>,
    server: EspHttpServer<'static>,
}

impl HttpServer {
    pub fn new(ip: Ipv4Addr, properties: Vec<String>) -> Result<Self> {
        let server = EspHttpServer::new(&HttpConfig::default())?;
        Ok(Self {
            ip,
            properties,
            server,
        })
    }

    fn add_portal_page_handler(&mut self, sender: Sender<HashMap<String, String>>) -> Result<()> {
        self.server
            .fn_handler("/", Method::Get, |req| -> Result<()> {
                let mut res = req.into_ok_response()?;
                res.write(index_html().as_bytes())?;
                Ok(())
            })?;

        let required_keys = self.properties.clone();
        self.server
            .fn_handler("/save", Method::Post, move |mut req| -> Result<()> {
                let body = read_body(&mut req)?;
                log::info!("Received: {}", body);
                let config = parse_body(&body);
                let missing: Vec<&str> = required_keys
                    .iter()
                    .filter(|key| !config.contains_key(*key))
                    .map(|key| key.as_str())
                    .collect();
                if !missing.is_empty() {
                    let message = format!("Missing fields: {}", missing.join(", "));
                    req.into_response(400, Some("Bad Request"), &[])?
                        .write_all(message.as_bytes())?;
                    return Ok(());
                }
                let mut res = req.into_ok_response()?;
                res.write_all("Ok".as_bytes())?;
                sender.send(config).ok();
                Ok(())
            })?;

        Ok(())
    }

    fn add_portal_detection_handlers(&mut self) -> Result<()> {
        let portal_url = format!("http://{}", self.ip);

        // Windows 11
        self.server
            .fn_handler("/connecttest.txt", Method::Get, |req| -> Result<()> {
                redirect(req, "http://logout.net")
            })?;

        // Windows 10, 404 prevents it from spamming the device
        self.server
            .fn_handler("/wpad.dat", Method::Get, |req| -> Result<()> {
                not_found(req)
            })?;

        // Android
        let url = portal_url.clone();
        self.server
            .fn_handler("/gen_204", Method::Get, move |req| -> Result<()> {
                redirect(req, &url)
            })?;
        let url = portal_url.clone();
        self.server
            .fn_handler("/generate_204", Method::Get, move |req| -> Result<()> {
                redirect(req, &url)
            })?;

        // Microsoft
        let url = portal_url.clone();
        self.server
            .fn_handler("/redirect", Method::Get, move |req| -> Result<()> {
                redirect(req, &url)
            })?;

        // Apple
        let url = portal_url.clone();
        self.server.fn_handler(
            "/hotspot-detect.html",
            Method::Get,
            move |req| -> Result<()> { redirect(req, &url) },
        )?;

        // Firefox (redirect)
        let url = portal_url.clone();
        self.server
            .fn_handler("/canonical.html", Method::Get, move |req| -> Result<()> {
                redirect(req, &url)
            })?;

        // Firefox (200), must be non-empty and must NOT contain "success"
        self.server
            .fn_handler("/success.txt", Method::Get, |req| -> Result<()> {
                req.into_ok_response()?.write_all(b"ok")?;
                Ok(())
            })?;

        // Windows NCSI
        let url = portal_url.clone();
        self.server
            .fn_handler("/ncsi.txt", Method::Get, move |req| -> Result<()> {
                redirect(req, &url)
            })?;

        // Favicon, 404 to suppress unnecessary traffic
        self.server
            .fn_handler("/favicon.ico", Method::Get, |req| -> Result<()> {
                not_found(req)
            })?;

        Ok(())
    }

    pub fn run_until_config_received(&mut self) -> Result<HashMap<String, String>> {
        let (sender, receiver) = channel::<HashMap<String, String>>();
        self.add_portal_page_handler(sender)?;
        self.add_portal_detection_handlers()?;
        let config = receiver.recv()?;
        Ok(config)
    }
}

static INDEX_HTML: &str = include_str!(concat!(env!("OUT_DIR"), "/index.min.html"));

pub(crate) fn index_html() -> &'static str {
    INDEX_HTML
}

fn read_body(req: &mut Request<&mut EspHttpConnection>) -> Result<String> {
    let mut body = String::new();
    let mut buffer = [0u8; 256];
    loop {
        match req.read(&mut buffer) {
            Ok(0) => break,
            Ok(n) => body.push_str(std::str::from_utf8(&buffer[..n])?),
            Err(e) => return Err(e.into()),
        }
    }
    Ok(body)
}

fn parse_body(body: &str) -> HashMap<String, String> {
    body.split('&')
        .filter_map(|pair| {
            let mut parts = pair.splitn(2, '=');
            let key = parts.next()?.to_string();
            let encoded_value = parts.next().unwrap_or("").replace('+', " ");
            let value = urlencoding::decode(&encoded_value)
                .unwrap_or(Cow::Borrowed(&encoded_value))
                .into_owned();
            Some((key, value))
        })
        .collect()
}

fn redirect<'a>(
    req: esp_idf_svc::http::server::Request<&mut esp_idf_svc::http::server::EspHttpConnection<'a>>,
    url: &str,
) -> Result<()> {
    req.into_response(302, None, &[("Location", url)])?
        .write_all(b"")?;
    Ok(())
}

fn not_found<'a>(
    req: esp_idf_svc::http::server::Request<&mut esp_idf_svc::http::server::EspHttpConnection<'a>>,
) -> Result<()> {
    req.into_response(404, None, &[])?.write_all(b"")?;
    Ok(())
}
