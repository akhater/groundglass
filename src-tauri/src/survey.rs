//! What a camera offers on its Wi-Fi when no session opens: the services its
//! UPnP description names and the ports that accept a connection. Names and
//! numbers only, so nothing identifying (the description also holds the serial).

use std::net::{SocketAddr, TcpStream};
use std::thread;
use std::time::Duration;

use serde::Serialize;

use crate::{camera, ptpip};

/// PTP/IP last, after CCAPI's and UPnP's.
const PORTS: [u16; 5] = [80, 443, 8080, 49152, 15740];
const PORT_WAIT: Duration = Duration::from_secs(1);
/// After a failed attempt: the camera has wedged on PTP/IP traffic right after a session.
const SETTLE: Duration = Duration::from_secs(3);

#[derive(Serialize)]
pub struct Survey {
    pub services: Vec<String>,
    pub ports: Vec<u16>,
}

/// Bare connections, closed at once: never a request and never a session.
pub fn survey(address: &str) -> Survey {
    thread::sleep(SETTLE);
    let host = camera::host(address);
    let services = ptpip::device_description(host).map_or_else(|_| Vec::new(), |description| service_names(&description));
    let ports = PORTS.into_iter().filter(|port| is_open(host, *port)).collect();
    Survey { services, ports }
}

fn is_open(host: &str, port: u16) -> bool {
    format!("{host}:{port}")
        .parse::<SocketAddr>()
        .is_ok_and(|address| TcpStream::connect_timeout(&address, PORT_WAIT).is_ok())
}

/// `ICPO-SmartPhoneEOSSystemService` from each `urn:...:service:ICPO-...:1`.
fn service_names(description: &[u8]) -> Vec<String> {
    String::from_utf8_lossy(description)
        .split("<serviceType>")
        .skip(1)
        .filter_map(|rest| rest.split("</serviceType>").next()?.split(':').rev().nth(1))
        .map(str::to_string)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::service_names;

    #[test]
    fn service_names_are_read_from_the_description_and_nothing_else() {
        let description = b"HTTP/1.1 200 OK\r\n\r\n<root><device>\
            <UDN>uuid:00000000-0000-1000-8000-SERIAL</UDN>\
            <serviceType>urn:schemas-canon-com:service:ICPO-SmartPhoneEOSSystemService:1</serviceType>\
            <serviceType>urn:schemas-canon-com:service:ICPO-CameraControlAPIService:1</serviceType>\
            </device></root>";
        assert_eq!(
            service_names(description),
            ["ICPO-SmartPhoneEOSSystemService", "ICPO-CameraControlAPIService"]
        );
        assert!(service_names(b"").is_empty());
    }
}
