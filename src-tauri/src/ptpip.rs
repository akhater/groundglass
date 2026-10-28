//! Canon's PTP over IP (TCP 15740): the camera's "Connect to EOS Utility" Wi-Fi
//! mode, separate from CCAPI (the camera runs one or the other). So far only
//! what a read-only check needs: pair, open a session, read what the camera
//! supports. Research and references: BACKLOG.md, "PTP/IP".

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream, ToSocketAddrs};
use std::time::Duration;

const PORT: u16 = 15740;
const PROTOCOL_VERSION: u32 = 0x0001_0000;
/// The camera pairs with this GUID and name, and reconnects without asking as
/// long as both stay the same.
const GUID: [u8; 16] = *b"Groundglass-v1\0\0";
const NAME: &str = "Groundglass";
/// Packets larger than this are refused (live view frames are well under).
const MAX_PACKET: usize = 16 << 20;

const GET_DEVICE_INFO: u16 = 0x1001;
const OPEN_SESSION: u16 = 0x1002;
const CLOSE_SESSION: u16 = 0x1003;
const RESPONSE_OK: u16 = 0x2001;

/// Packet types (PTP/IP header field 2).
mod packet {
    pub const INIT_COMMAND_REQUEST: u32 = 1;
    pub const INIT_COMMAND_ACK: u32 = 2;
    pub const INIT_EVENT_REQUEST: u32 = 3;
    pub const INIT_EVENT_ACK: u32 = 4;
    pub const INIT_FAIL: u32 = 5;
    pub const OPERATION_REQUEST: u32 = 6;
    pub const OPERATION_RESPONSE: u32 = 7;
    pub const START_DATA: u32 = 9;
    pub const DATA: u32 = 10;
    pub const END_DATA: u32 = 12;
}

/// What the camera says it can do (PTP DeviceInfo), without its serial number.
#[derive(Debug, PartialEq)]
pub struct DeviceInfo {
    pub manufacturer: String,
    pub model: String,
    pub version: String,
    pub operations: Vec<u16>,
    pub properties: Vec<u16>,
}

/// An open PTP session with the camera: the command and event connections.
pub struct Session {
    command: TcpStream,
    _event: TcpStream,
    transaction: u32,
}

impl Session {
    /// Pairs and opens a session. The first time, the camera asks on its screen
    /// to confirm the pairing: up to `pairing_wait` is left for that.
    pub fn open(host: &str, pairing_wait: Duration) -> Result<Self, String> {
        let address = (host, PORT)
            .to_socket_addrs()
            .map_err(|e| e.to_string())?
            .next()
            .ok_or("no address for the camera")?;
        let mut command = connect(address)?;
        command.set_read_timeout(Some(pairing_wait)).map_err(|e| e.to_string())?;
        send(&mut command, packet::INIT_COMMAND_REQUEST, &init_command_request(&GUID, NAME))?;
        let (kind, ack) = receive(&mut command)?;
        match kind {
            packet::INIT_COMMAND_ACK if ack.len() >= 4 => {}
            packet::INIT_FAIL => return Err(format!("the camera refused to pair (reason {:?})", ack.get(..4))),
            other => return Err(format!("unexpected answer to pairing: packet type {other}")),
        }
        command.set_read_timeout(Some(Duration::from_secs(10))).map_err(|e| e.to_string())?;

        let mut event = connect(address)?;
        event.set_read_timeout(Some(Duration::from_secs(10))).map_err(|e| e.to_string())?;
        send(&mut event, packet::INIT_EVENT_REQUEST, &ack[..4])?;
        match receive(&mut event)? {
            (packet::INIT_EVENT_ACK, _) => {}
            (other, _) => return Err(format!("unexpected answer on the event connection: packet type {other}")),
        }

        let mut session = Self { command, _event: event, transaction: 0 };
        session.operation(OPEN_SESSION, &[1])?; // transaction 0, as PTP asks
        Ok(session)
    }

    pub fn device_info(&mut self) -> Result<DeviceInfo, String> {
        let data = self.operation(GET_DEVICE_INFO, &[])?;
        parse_device_info(&data)
    }

    pub fn close(mut self) -> Result<(), String> {
        self.operation(CLOSE_SESSION, &[]).map(|_| ())
    }

    /// Runs one operation that sends no data; returns what the camera sends back.
    fn operation(&mut self, code: u16, params: &[u32]) -> Result<Vec<u8>, String> {
        let transaction = self.transaction;
        self.transaction += 1;
        send(&mut self.command, packet::OPERATION_REQUEST, &operation_request(code, transaction, params))?;
        let mut data = Vec::new();
        loop {
            let (kind, payload) = receive(&mut self.command)?;
            match kind {
                packet::START_DATA => {}
                // Data packets start with the transaction id.
                packet::DATA | packet::END_DATA => data.extend_from_slice(payload.get(4..).unwrap_or_default()),
                packet::OPERATION_RESPONSE if payload.len() >= 2 => {
                    let response = u16::from_le_bytes([payload[0], payload[1]]);
                    return match response {
                        RESPONSE_OK => Ok(data),
                        _ => Err(format!("the camera answered 0x{response:04x} to 0x{code:04x}")),
                    };
                }
                other => return Err(format!("unexpected packet type {other} during 0x{code:04x}")),
            }
        }
    }
}

fn connect(address: SocketAddr) -> Result<TcpStream, String> {
    let stream = TcpStream::connect_timeout(&address, Duration::from_secs(5))
        .map_err(|e| format!("can't reach the camera on port {PORT}: {e}"))?;
    stream.set_nodelay(true).map_err(|e| e.to_string())?;
    Ok(stream)
}

/// A packet: total length and type (both u32, little-endian), then the payload.
fn send(stream: &mut TcpStream, kind: u32, payload: &[u8]) -> Result<(), String> {
    let mut bytes = ((8 + payload.len()) as u32).to_le_bytes().to_vec();
    bytes.extend_from_slice(&kind.to_le_bytes());
    bytes.extend_from_slice(payload);
    stream.write_all(&bytes).map_err(|e| e.to_string())
}

fn receive(stream: &mut TcpStream) -> Result<(u32, Vec<u8>), String> {
    let mut header = [0u8; 8];
    stream.read_exact(&mut header).map_err(|e| format!("no answer from the camera: {e}"))?;
    let length = u32::from_le_bytes([header[0], header[1], header[2], header[3]]) as usize;
    let kind = u32::from_le_bytes([header[4], header[5], header[6], header[7]]);
    if !(8..=MAX_PACKET).contains(&length) {
        return Err(format!("bad packet length {length}"));
    }
    let mut payload = vec![0; length - 8];
    stream.read_exact(&mut payload).map_err(|e| e.to_string())?;
    Ok((kind, payload))
}

/// Init_Command_Request payload: who we are, so the camera can pair with us.
fn init_command_request(guid: &[u8; 16], name: &str) -> Vec<u8> {
    let mut out = guid.to_vec();
    for unit in name.encode_utf16().chain([0]) {
        out.extend_from_slice(&unit.to_le_bytes());
    }
    out.extend_from_slice(&PROTOCOL_VERSION.to_le_bytes());
    out
}

/// Operation_Request payload for an operation that sends no data.
fn operation_request(code: u16, transaction: u32, params: &[u32]) -> Vec<u8> {
    let mut out = 1u32.to_le_bytes().to_vec(); // data phase: nothing sent
    out.extend_from_slice(&code.to_le_bytes());
    out.extend_from_slice(&transaction.to_le_bytes());
    for param in params {
        out.extend_from_slice(&param.to_le_bytes());
    }
    out
}

fn parse_device_info(data: &[u8]) -> Result<DeviceInfo, String> {
    let mut read = Reader(data);
    let short = || "the camera's device info is shorter than expected".to_string();
    read.skip(2 + 4 + 2).ok_or_else(short)?; // versions and vendor extension id
    read.string().ok_or_else(short)?; // vendor extension description
    read.skip(2).ok_or_else(short)?; // functional mode
    let operations = read.array16().ok_or_else(short)?;
    read.array16().ok_or_else(short)?; // events
    let properties = read.array16().ok_or_else(short)?;
    read.array16().ok_or_else(short)?; // capture formats
    read.array16().ok_or_else(short)?; // image formats
    Ok(DeviceInfo {
        manufacturer: read.string().ok_or_else(short)?,
        model: read.string().ok_or_else(short)?,
        version: read.string().ok_or_else(short)?,
        operations,
        properties,
    })
}

/// Reads PTP data front to back.
struct Reader<'a>(&'a [u8]);

impl Reader<'_> {
    fn take(&mut self, n: usize) -> Option<&[u8]> {
        if self.0.len() < n {
            return None;
        }
        let (head, rest) = self.0.split_at(n);
        self.0 = rest;
        Some(head)
    }

    fn skip(&mut self, n: usize) -> Option<()> {
        self.take(n).map(|_| ())
    }

    /// A count of characters including the final null, then UTF-16LE.
    fn string(&mut self) -> Option<String> {
        let count = *self.take(1)?.first()? as usize;
        let units: Vec<u16> = self.take(count * 2)?.as_chunks::<2>().0.iter().map(|p| u16::from_le_bytes(*p)).collect();
        Some(String::from_utf16_lossy(&units).trim_end_matches('\0').to_string())
    }

    fn array16(&mut self) -> Option<Vec<u16>> {
        let count = u32::from_le_bytes(self.take(4)?.try_into().ok()?) as usize;
        let bytes = self.take(count.checked_mul(2)?)?;
        Some(bytes.as_chunks::<2>().0.iter().map(|p| u16::from_le_bytes(*p)).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A PTP string: character count including the final null, then UTF-16LE.
    fn ptp_string(text: &str) -> Vec<u8> {
        let mut out = vec![(text.encode_utf16().count() + 1) as u8];
        for unit in text.encode_utf16().chain([0]) {
            out.extend_from_slice(&unit.to_le_bytes());
        }
        out
    }

    fn array16(items: &[u16]) -> Vec<u8> {
        let mut out = (items.len() as u32).to_le_bytes().to_vec();
        for item in items {
            out.extend_from_slice(&item.to_le_bytes());
        }
        out
    }

    #[test]
    fn init_command_request_carries_guid_name_and_version() {
        let guid = *b"0123456789abcdef";
        let request = init_command_request(&guid, "Gg");
        assert_eq!(&request[..16], b"0123456789abcdef");
        assert_eq!(&request[16..22], &[b'G', 0, b'g', 0, 0, 0]);
        assert_eq!(&request[22..], &[0x00, 0x00, 0x01, 0x00]);
    }

    #[test]
    fn operation_request_says_no_data_out() {
        assert_eq!(
            operation_request(0x1002, 0, &[1]),
            vec![1, 0, 0, 0, 0x02, 0x10, 0, 0, 0, 0, 1, 0, 0, 0]
        );
    }

    #[test]
    fn device_info_lists_operations_and_properties() {
        let mut data = vec![100, 0]; // standard version
        data.extend_from_slice(&11u32.to_le_bytes()); // vendor extension id (Canon)
        data.extend_from_slice(&[100, 0]); // vendor extension version
        data.extend(ptp_string("")); // vendor extension description
        data.extend_from_slice(&[0, 0]); // functional mode
        data.extend(array16(&[0x1001, 0x9153, 0x9158]));
        data.extend(array16(&[0x4002])); // events
        data.extend(array16(&[0xd1b2]));
        data.extend(array16(&[])); // capture formats
        data.extend(array16(&[0x3801])); // image formats
        data.extend(ptp_string("Canon Inc."));
        data.extend(ptp_string("Canon EOS R6 Mark III"));
        data.extend(ptp_string("1-1.1.0"));
        data.extend(ptp_string("SERIAL")); // read past, never kept
        assert_eq!(
            parse_device_info(&data),
            Ok(DeviceInfo {
                manufacturer: "Canon Inc.".into(),
                model: "Canon EOS R6 Mark III".into(),
                version: "1-1.1.0".into(),
                operations: vec![0x1001, 0x9153, 0x9158],
                properties: vec![0xd1b2],
            })
        );
        assert!(parse_device_info(&data[..20]).is_err());
    }
}
