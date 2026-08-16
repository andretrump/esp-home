use std::net::{Ipv4Addr, SocketAddr, UdpSocket};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

pub struct DnsServerHandle {
    running: Arc<AtomicBool>,
}

impl DnsServerHandle {
    pub fn stop(&self) {
        self.running.store(false, Ordering::Relaxed);
    }
}

pub struct DnsServer {
    target_ip: Ipv4Addr,
    running: Arc<AtomicBool>,
}

impl DnsServer {
    pub fn new(target_ip: Ipv4Addr) -> Self {
        Self {
            target_ip,
            running: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn start(self) -> DnsServerHandle {
        let handle = DnsServerHandle {
            running: Arc::clone(&self.running),
        };
        std::thread::spawn(move || {
            let socket = match UdpSocket::bind("0.0.0.0:53") {
                Ok(s) => s,
                Err(e) => {
                    log::error!("DNS bind failed: {e}");
                    return;
                }
            };
            match socket.set_read_timeout(Some(Duration::from_millis(100))) {
                Ok(_) => (),
                Err(e) => {
                    log::error!("DNS set timeout failed: {e}");
                    return;
                }
            };
            self.running.store(true, Ordering::Relaxed);
            let mut buffer = [0u8; 512];
            loop {
                if !self.running.load(Ordering::Relaxed) {
                    break;
                }
                match socket.recv_from(&mut buffer) {
                    Ok((_, src)) => self.handle_query(&socket, &buffer, src),
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => continue,
                    Err(e) if e.kind() == std::io::ErrorKind::TimedOut => continue,
                    Err(e) => log::warn!("DNS receive error: {e}"),
                }
            }
        });
        handle
    }

    fn handle_query(&self, socket: &UdpSocket, buffer: &[u8], src: SocketAddr) {
        log::info!("Received DNS query from src {src}");
        if let Some(response) = self.build_response(buffer) {
            socket.send_to(&response, src).unwrap_or_else(|e| {
                log::warn!("DNS send error: {e}");
                0
            });
        } else {
            log::warn!("Failed to parse DNS query from {src}");
        }
    }

    fn build_response(&self, query: &[u8]) -> Option<Vec<u8>> {
        if query.len() < 12 {
            return None;
        }

        let qname_end = parse_qname_end(query, 12)?;
        let question_end = qname_end.checked_add(4)?; // QTYPE (2) + QCLASS (2)
        if question_end > query.len() {
            return None;
        }

        let qtype = u16::from_be_bytes([query[qname_end], query[qname_end + 1]]);
        let is_qtype_a = qtype == 1;

        let mut response = Vec::with_capacity(question_end + 16);
        self.add_response_header(query, is_qtype_a, &mut response);
        response.extend_from_slice(&query[12..question_end]); // question section
        if is_qtype_a {
            self.add_response_answer(&mut response);
        }
        Some(response)
    }

    fn add_response_header(&self, query: &[u8], is_qtype_a: bool, response: &mut Vec<u8>) {
        response.push(query[0]);
        response.push(query[1]); // Transaction ID
        response.push(0x81); // QR=1, AA=1
        response.push(0x80); // RCODE=0
        response.push(query[4]);
        response.push(query[5]); // QDCOUNT (echo)
        response.push(0x00);
        response.push(if is_qtype_a { 0x01 } else { 0x00 }); // ANCOUNT
        response.push(0x00);
        response.push(0x00); // NSCOUNT
        response.push(0x00);
        response.push(0x00); // ARCOUNT
    }

    fn add_response_answer(&self, response: &mut Vec<u8>) {
        response.push(0xc0);
        response.push(0x0c); // Name pointer → offset 12
        response.push(0x00);
        response.push(0x01); // Type A
        response.push(0x00);
        response.push(0x01); // Class IN
        response.push(0x00);
        response.push(0x00);
        response.push(0x00);
        response.push(0x3c); // TTL 60s
        response.push(0x00);
        response.push(0x04); // RDLENGTH 4
        response.extend_from_slice(&self.target_ip.octets());
    }
}

fn parse_qname_end(buffer: &[u8], mut offset: usize) -> Option<usize> {
    loop {
        let len = *buffer.get(offset)? as usize;
        if len == 0 {
            return Some(offset + 1);
        }
        if len & 0xc0 == 0xc0 {
            return None;
        }
        offset = offset.checked_add(1)?.checked_add(len)?;
        if offset > buffer.len() {
            return None;
        }
    }
}
