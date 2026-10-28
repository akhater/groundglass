//! Canon's PTP over IP (TCP 15740): the camera's "Connect to EOS Utility" Wi-Fi
//! mode, separate from CCAPI (the camera runs one or the other). So far only
//! what a read-only check needs: pair, open a session, read what the camera
//! supports. Research and references: BACKLOG.md, "PTP/IP".

use crate::ccapi::Level;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream, ToSocketAddrs};
use std::time::Duration;

const PORT: u16 = 15740;
/// 1.0, as Canon's EOS Utility sends it to the R6 Mark III (recorded 2026-10-29).
const PROTOCOL_VERSION: u32 = 0x0001_0000;
/// The camera pairs with this GUID and name, and reconnects without asking as
/// long as both stay the same. A random UUID, generated once.
const GUID: [u8; 16] = [
    0xd5, 0xca, 0x31, 0x78, 0xf1, 0x48, 0x4b, 0xee, 0xb9, 0x64, 0xe2, 0xcb, 0xa3, 0x3a, 0xe3, 0x61,
];
const NAME: &str = "Groundglass";
/// Packets larger than this are refused (live view frames are well under).
const MAX_PACKET: usize = 16 << 20;

const GET_DEVICE_INFO: u16 = 0x1001;
const OPEN_SESSION: u16 = 0x1002;
const CLOSE_SESSION: u16 = 0x1003;
// Canon EOS operations (names as in libgphoto2's ptp.h).
const SET_DEVICE_PROP_VALUE_EX: u16 = 0x9110;
const SET_REMOTE_MODE: u16 = 0x9114;
const SET_EVENT_MODE: u16 = 0x9115;
const GET_EVENT_DATA: u16 = 0x9116;
const GET_VIEWFINDER_DATA: u16 = 0x9153;
const DO_AF: u16 = 0x9154;
const AF_CANCEL: u16 = 0x9160;
const ZOOM: u16 = 0x9158;
const ZOOM_POSITION: u16 = 0x9159;
// Canon EOS properties.
const EVF_OUTPUT_DEVICE: u32 = 0xd1b0;
const EVF_MODE: u32 = 0xd1b1;
const DEPTH_OF_FIELD_PREVIEW: u32 = 0xd1b2;

const RESPONSE_OK: u16 = 0x2001;
const RESPONSE_BUSY: u16 = 0x2019;
/// Canon's "live view frame not ready yet".
const RESPONSE_NOT_READY: u16 = 0xa102;

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
        discover(address)?;
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

        // As EOS Utility opens (recorded 2026-10-29): device info first, then
        // OpenSession with session 0x41, both as transaction 0.
        let mut session = Self { command, _event: event, transaction: 0 };
        session.operation(GET_DEVICE_INFO, &[])?;
        session.transaction = 0;
        session.operation(OPEN_SESSION, &[0x41])?;
        Ok(session)
    }

    pub fn device_info(&mut self) -> Result<DeviceInfo, String> {
        let data = self.operation(GET_DEVICE_INFO, &[])?;
        parse_device_info(&data)
    }

    /// Ends the session. On the R6 Mark III (2026-10-29), live view output off
    /// then CloseSession gave the camera its screen back; open-eos-control's
    /// USB hand-back (remote mode 0, 1, event mode 0) left the screen off.
    pub fn close(mut self) -> Result<(), String> {
        self.operation(CLOSE_SESSION, &[]).map(|_| ())
    }

    /// Starts live view towards us: remote mode 5 and event mode 1 as EOS
    /// Utility sends them after OpenSession (the R6 Mark III refuses remote mode
    /// 1 there), then as open-eos-control does over USB: EVF mode 1, EVF output
    /// to the computer (2).
    pub fn start_live_view(&mut self) -> Result<(), String> {
        self.operation(SET_REMOTE_MODE, &[5])?;
        self.operation(SET_EVENT_MODE, &[1])?;
        self.events()?;
        self.set_property(EVF_MODE, 1)?;
        self.set_property(EVF_OUTPUT_DEVICE, 2)?;
        self.events()
    }

    /// Where live view goes while it runs: to us only (2), or to us and the
    /// camera's own screen (3, being tried: the TFT bit plus the computer bit).
    pub fn live_view_output(&mut self, camera_screen_too: bool) -> Result<(), String> {
        self.set_property(EVF_OUTPUT_DEVICE, if camera_screen_too { 3 } else { 2 })
    }

    /// Live view back to the camera's own screen only (1). Output "none" (0)
    /// sometimes left the R6 Mark III's screen off after the session (2026-10-29).
    pub fn stop_live_view(&mut self) -> Result<(), String> {
        self.set_property(EVF_OUTPUT_DEVICE, 1)
    }

    /// The newest live view frame, or None while the camera has none ready.
    pub fn frame(&mut self) -> Result<Option<Vec<u8>>, String> {
        Ok(self.viewfinder_data()?.and_then(|data| viewfinder_jpeg(&data).map(<[u8]>::to_vec)))
    }

    /// A whole live view reply: the picture and the blocks that come with it
    /// (length, type, body each). None while the camera has none ready.
    pub fn viewfinder_data(&mut self) -> Result<Option<Vec<u8>>, String> {
        let (response, data) = self.transact(GET_VIEWFINDER_DATA, &[0x0020_0000, 0, 0], None)?;
        match response {
            RESPONSE_OK => Ok(Some(data)),
            RESPONSE_BUSY | RESPONSE_NOT_READY => Ok(None),
            other => Err(format!("the camera answered 0x{other:04x} to a live view frame request")),
        }
    }

    /// Back-button focus: DoAf (0x9154) while held, AfCancel (0x9160) on
    /// release, as open-eos-control does over USB.
    pub fn autofocus(&mut self, held: bool) -> Result<(), String> {
        self.operation(if held { DO_AF } else { AF_CANCEL }, &[]).map(|_| ())
    }

    /// Live view magnification: 1 (whole picture), 5 or 10, as EOS Utility
    /// sends it (recorded 2026-10-29).
    pub fn zoom(&mut self, level: u32) -> Result<(), String> {
        self.operation(ZOOM, &[level]).map(|_| ())
    }

    /// Where the magnified area sits, in sensor pixels (EOS Utility moved it in
    /// steps of 68 around 2716, 1856 on the R6 Mark III).
    pub fn zoom_position(&mut self, x: u32, y: u32) -> Result<(), String> {
        self.operation(ZOOM_POSITION, &[x, y]).map(|_| ())
    }

    /// Depth of field preview: the lens stops down to the set aperture.
    /// The camera doesn't list this property, but EOS Utility sets it.
    pub fn depth_of_field_preview(&mut self, on: bool) -> Result<(), String> {
        self.set_property(DEPTH_OF_FIELD_PREVIEW, on as u32)
    }

    /// Reads (and drops) what the camera has to report. EOS Utility asks
    /// continuously; the camera expects to be asked.
    pub fn events(&mut self) -> Result<(), String> {
        self.event_data().map(|_| ())
    }

    /// What the camera reports since it was last asked: records of length,
    /// type, body (0xC189 a property's new value), ending with a type 0 record.
    pub fn event_data(&mut self) -> Result<Vec<u8>, String> {
        self.operation(GET_EVENT_DATA, &[])
    }

    fn set_property(&mut self, property: u32, value: u32) -> Result<(), String> {
        let (response, _) = self.transact(SET_DEVICE_PROP_VALUE_EX, &[], Some(&property_value(property, value)))?;
        match response {
            RESPONSE_OK => Ok(()),
            other => Err(format!("the camera answered 0x{other:04x} to setting property 0x{property:04x}")),
        }
    }

    /// Runs one operation that sends no data; returns what the camera sends back.
    fn operation(&mut self, code: u16, params: &[u32]) -> Result<Vec<u8>, String> {
        match self.transact(code, params, None)? {
            (RESPONSE_OK, data) => Ok(data),
            (response, _) => Err(format!("the camera answered 0x{response:04x} to 0x{code:04x}")),
        }
    }

    /// One operation: sends `out` as its data when given; returns the camera's
    /// response code and any data it sends back.
    fn transact(&mut self, code: u16, params: &[u32], out: Option<&[u8]>) -> Result<(u16, Vec<u8>), String> {
        let transaction = self.transaction;
        self.transaction += 1;
        let mut request = operation_request(code, transaction, params);
        if out.is_some() {
            request[..4].copy_from_slice(&2u32.to_le_bytes()); // data phase: we send data
        }
        send(&mut self.command, packet::OPERATION_REQUEST, &request)?;
        if let Some(out) = out {
            let mut start = transaction.to_le_bytes().to_vec();
            start.extend_from_slice(&(out.len() as u64).to_le_bytes());
            send(&mut self.command, packet::START_DATA, &start)?;
            let mut end = transaction.to_le_bytes().to_vec();
            end.extend_from_slice(out);
            send(&mut self.command, packet::END_DATA, &end)?;
        }
        let mut data = Vec::new();
        loop {
            let (kind, payload) = receive(&mut self.command)?;
            match kind {
                packet::START_DATA => {}
                // Data packets start with the transaction id.
                packet::DATA | packet::END_DATA => data.extend_from_slice(payload.get(4..).unwrap_or_default()),
                packet::OPERATION_RESPONSE if payload.len() >= 2 => {
                    return Ok((u16::from_le_bytes([payload[0], payload[1]]), data));
                }
                other => return Err(format!("unexpected packet type {other} during 0x{code:04x}")),
            }
        }
    }
}

/// What EOS Utility does just before it connects (recorded 2026-10-29): ask
/// the network for Canon cameras, then read the camera's UPnP description.
/// The camera drops a pairing request without it.
fn discover(camera: SocketAddr) -> Result<(), String> {
    let search = "M-SEARCH * HTTP/1.1\r\nHOST: 239.255.255.250:1900\r\nMAN: \"ssdp:discover\"\r\nMX: 1\r\n\
                  ST: urn:schemas-canon-com:service:ICPO-WFTEOSSystemService:1\r\n\r\n";
    let udp = std::net::UdpSocket::bind("0.0.0.0:0").map_err(|e| e.to_string())?;
    udp.send_to(search.as_bytes(), "239.255.255.250:1900").map_err(|e| e.to_string())?;
    // The answer only tells us what we already know (the camera is there): wait briefly for it.
    udp.set_read_timeout(Some(Duration::from_millis(1500))).map_err(|e| e.to_string())?;
    let mut answer = [0u8; 1024];
    let _ = udp.recv_from(&mut answer);

    let http = SocketAddr::new(camera.ip(), 49152);
    let mut stream = TcpStream::connect_timeout(&http, Duration::from_secs(5)).map_err(|e| e.to_string())?;
    stream.set_read_timeout(Some(Duration::from_secs(5))).map_err(|e| e.to_string())?;
    let request = format!(
        "GET /upnp/CameraDevDesc.xml HTTP/1.1\r\nUser-Agent: wsCDeviceDetector\r\nHost: {http}\r\n\
         Cache-Control: no-cache\r\nConnection: close\r\n\r\n"
    );
    stream.write_all(request.as_bytes()).map_err(|e| e.to_string())?;
    let mut description = Vec::new();
    stream.read_to_end(&mut description).map_err(|e| e.to_string())?;
    Ok(())
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

/// SetDevicePropValueEx data: total length, property code, value.
fn property_value(property: u32, value: u32) -> [u8; 12] {
    let mut out = [0u8; 12];
    out[..4].copy_from_slice(&12u32.to_le_bytes());
    out[4..8].copy_from_slice(&property.to_le_bytes());
    out[8..].copy_from_slice(&value.to_le_bytes());
    out
}

/// The JPEG in a GetViewFinderData reply: blocks of length, type, body; the
/// picture is type 1 (or 11).
fn viewfinder_jpeg(data: &[u8]) -> Option<&[u8]> {
    viewfinder_block(data, 0x01).or_else(|| viewfinder_block(data, 0x0b))
}

/// The camera's level from a GetViewFinderData reply: block 0x10 holds two
/// words, then roll and pitch in hundredths of a degree, 0..36000, the same
/// units as CCAPI (found 2026-10-29 by tilting the R6 Mark III).
pub fn viewfinder_level(data: &[u8]) -> Option<Level> {
    let body = viewfinder_block(data, 0x10)?;
    let word = |i: usize| Some(u32::from_le_bytes(body.get(i..i + 4)?.try_into().ok()?));
    let degrees = |hundredths: u32| {
        let d = hundredths as f32 / 100.0;
        if d > 180.0 { d - 360.0 } else { d }
    };
    Some(Level { roll: degrees(word(8)?), pitch: degrees(word(12)?) })
}

/// The body of the first block of type `wanted` in a GetViewFinderData reply.
fn viewfinder_block(data: &[u8], wanted: u32) -> Option<&[u8]> {
    let mut rest = data;
    while rest.len() >= 8 {
        let length = u32::from_le_bytes(rest[..4].try_into().ok()?) as usize;
        let kind = u32::from_le_bytes(rest[4..8].try_into().ok()?);
        if length < 8 || length > rest.len() {
            return None;
        }
        if kind == wanted {
            return Some(&rest[8..length]);
        }
        rest = &rest[length..];
    }
    None
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
    fn property_write_is_length_code_value() {
        // As EOS Utility turned depth of field preview on (recorded 2026-10-29).
        assert_eq!(property_value(0xd1b2, 1), [0x0c, 0, 0, 0, 0xb2, 0xd1, 0, 0, 1, 0, 0, 0]);
    }

    #[test]
    fn viewfinder_data_gives_the_jpeg_block() {
        let block = |kind: u32, body: &[u8]| {
            let mut out = ((8 + body.len()) as u32).to_le_bytes().to_vec();
            out.extend_from_slice(&kind.to_le_bytes());
            out.extend_from_slice(body);
            out
        };
        let mut data = block(0x0e, &[0; 16]);
        data.extend(block(0x01, &[0xff, 0xd8, 0xff, 0xd9]));
        assert_eq!(viewfinder_jpeg(&data), Some(&[0xff, 0xd8, 0xff, 0xd9][..]));
        assert_eq!(viewfinder_jpeg(&block(0x0b, &[0xff, 0xd8])), Some(&[0xff, 0xd8][..]));
        assert_eq!(viewfinder_jpeg(&block(0x0e, &[0; 16])), None);
        assert_eq!(viewfinder_jpeg(&[9, 0, 0, 0]), None); // cut short
    }

    #[test]
    fn viewfinder_data_gives_the_level() {
        // Block 0x10 as the R6 Mark III sent it, tilted 28 degrees and 1 down (2026-10-29).
        let mut data = 16u32.to_le_bytes().to_vec(); // a block before it
        data.extend_from_slice(&0x0eu32.to_le_bytes());
        data.extend_from_slice(&[0; 8]);
        data.extend_from_slice(&24u32.to_le_bytes());
        data.extend_from_slice(&0x10u32.to_le_bytes());
        for word in [2u32, 0, 2800, 35900] {
            data.extend_from_slice(&word.to_le_bytes());
        }
        assert_eq!(viewfinder_level(&data), Some(Level { roll: 28.0, pitch: -1.0 }));
        assert_eq!(viewfinder_level(&data[..16]), None);
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
