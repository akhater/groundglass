//! Canon's PTP over IP (TCP 15740): the camera's "Connect to EOS Utility" Wi-Fi
//! mode, separate from CCAPI (the camera runs one or the other). So far only
//! what a read-only check needs: pair, open a session, read what the camera
//! supports. Research and references: BACKLOG.md, "PTP/IP".

use crate::camera::Level;
use std::fmt;
use std::io::{Read, Write};
use std::net::{IpAddr, SocketAddr, TcpStream, ToSocketAddrs, UdpSocket};
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
const REMOTE_RELEASE_ON: u16 = 0x9128;
const REMOTE_RELEASE_OFF: u16 = 0x9129;
const DO_AF: u16 = 0x9154;
const TOUCH_AF_POSITION: u16 = 0x915b;
const AF_CANCEL: u16 = 0x9160;
const ZOOM: u16 = 0x9158;
const ZOOM_POSITION: u16 = 0x9159;
// Canon EOS properties.
const EVF_OUTPUT_DEVICE: u32 = 0xd1b0;
const EVF_MODE: u32 = 0xd1b1;
const DEPTH_OF_FIELD_PREVIEW: u32 = 0xd1b2;

// Canon EOS report records (GetEventData).
const PROPERTY_VALUE_CHANGED: u32 = 0xc189;
const ALLOWED_VALUES_CHANGED: u32 = 0xc18a;

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

/// What went wrong talking to the camera.
#[derive(Debug, PartialEq)]
pub enum Error {
    /// No connection, or it failed or timed out.
    Connection(String),
    /// The camera turned the pairing down (Init_Fail).
    PairingRefused,
    /// The camera can't answer this instant: busy (0x2019), or no live view frame yet (0xA102).
    Busy,
    /// The camera answered `operation` with `response` instead of OK.
    Response { operation: u16, response: u16 },
    /// The camera sent something this client doesn't expect.
    Protocol(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Connection(detail) | Self::Protocol(detail) => f.write_str(detail),
            Self::PairingRefused => f.write_str("the camera refused to pair"),
            Self::Busy => f.write_str("the camera is busy"),
            Self::Response { operation, response } => {
                write!(f, "the camera answered 0x{response:04x} to 0x{operation:04x}")
            }
        }
    }
}

impl From<std::io::Error> for Error {
    fn from(error: std::io::Error) -> Self {
        Self::Connection(error.to_string())
    }
}

impl From<Error> for String {
    fn from(error: Error) -> Self {
        error.to_string()
    }
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
    /// What the camera has reported so far, kept up to date by every report read.
    properties: Properties,
}

impl Session {
    /// Pairs and opens a session. The first time, the camera asks on its screen
    /// to confirm the pairing: up to `pairing_wait` is left for that.
    pub fn open(host: &str, pairing_wait: Duration) -> Result<Self, Error> {
        let address = socket_address(host, PORT)?;
        discover(address)?;
        let mut command = connect(address)?;
        command.set_read_timeout(Some(pairing_wait))?;
        send(&mut command, packet::INIT_COMMAND_REQUEST, &init_command_request(&GUID, NAME))?;
        let (kind, ack) = receive(&mut command)?;
        match kind {
            packet::INIT_COMMAND_ACK if ack.len() >= 4 => {}
            packet::INIT_FAIL => return Err(Error::PairingRefused),
            other => return Err(Error::Protocol(format!("unexpected answer to pairing: packet type {other}"))),
        }
        command.set_read_timeout(Some(Duration::from_secs(10)))?;

        let mut event = connect(address)?;
        event.set_read_timeout(Some(Duration::from_secs(10)))?;
        send(&mut event, packet::INIT_EVENT_REQUEST, &ack[..4])?;
        match receive(&mut event)? {
            (packet::INIT_EVENT_ACK, _) => {}
            (other, _) => {
                return Err(Error::Protocol(format!("unexpected answer on the event connection: packet type {other}")));
            }
        }

        // As EOS Utility opens (recorded 2026-10-29): device info first, then
        // OpenSession with session 0x41, both as transaction 0.
        let mut session = Self { command, _event: event, transaction: 0, properties: Properties::default() };
        session.operation(GET_DEVICE_INFO, &[])?;
        session.transaction = 0;
        session.operation(OPEN_SESSION, &[0x41])?;
        Ok(session)
    }

    pub fn device_info(&mut self) -> Result<DeviceInfo, Error> {
        let data = self.operation(GET_DEVICE_INFO, &[])?;
        parse_device_info(&data).map_err(Error::Protocol)
    }

    /// How long to wait for each answer from now on (10 s once open).
    pub fn set_timeout(&mut self, wait: Duration) -> Result<(), Error> {
        Ok(self.command.set_read_timeout(Some(wait))?)
    }

    /// Ends the session. On the R6 Mark III (2026-10-29), live view output off
    /// then CloseSession gave the camera its screen back; open-eos-control's
    /// USB hand-back (remote mode 0, 1, event mode 0) left the screen off.
    pub fn close(mut self) -> Result<(), Error> {
        self.operation(CLOSE_SESSION, &[]).map(|_| ())
    }

    /// Starts live view towards us: remote mode 5 and event mode 1 as EOS
    /// Utility sends them after OpenSession (the R6 Mark III refuses remote mode
    /// 1 there), then as open-eos-control does over USB: EVF mode 1, EVF output
    /// to the computer (2).
    pub fn start_live_view(&mut self) -> Result<(), Error> {
        self.operation(SET_REMOTE_MODE, &[5])?;
        self.operation(SET_EVENT_MODE, &[1])?;
        self.events()?;
        self.set_property(EVF_MODE, 1)?;
        self.set_property(EVF_OUTPUT_DEVICE, 2)?;
        self.events()
    }

    /// Where live view goes while it runs: to us only (2), or to us and the
    /// camera's own screen (3, being tried: the TFT bit plus the computer bit).
    pub fn live_view_output(&mut self, camera_screen_too: bool) -> Result<(), Error> {
        self.set_property(EVF_OUTPUT_DEVICE, if camera_screen_too { 3 } else { 2 })
    }

    /// Live view back to the camera's own screen only (1). Output "none" (0)
    /// sometimes left the R6 Mark III's screen off after the session (2026-10-29).
    pub fn stop_live_view(&mut self) -> Result<(), Error> {
        self.set_property(EVF_OUTPUT_DEVICE, 1)
    }

    /// The newest live view frame, or None while the camera has none ready.
    pub fn frame(&mut self) -> Result<Option<Vec<u8>>, Error> {
        Ok(self.viewfinder_data()?.and_then(|data| viewfinder_jpeg(&data).map(<[u8]>::to_vec)))
    }

    /// A whole live view reply: the picture and the blocks that come with it
    /// (length, type, body each). None while the camera has none ready.
    pub fn viewfinder_data(&mut self) -> Result<Option<Vec<u8>>, Error> {
        let (response, data) = self.transact(GET_VIEWFINDER_DATA, &[0x0020_0000, 0, 0], None)?;
        match response {
            RESPONSE_OK => Ok(Some(data)),
            RESPONSE_BUSY | RESPONSE_NOT_READY => Ok(None),
            other => Err(response_error(GET_VIEWFINDER_DATA, other)),
        }
    }

    /// Takes a photo: a full press and release, as libgphoto2's "Press Full"
    /// then "Release" (RemoteReleaseOn [3, 0], RemoteReleaseOff [3]). The
    /// press doesn't force focusing either way: being checked that the camera
    /// applies its own shutter-button setting (back-button focus: no AF).
    pub fn shoot(&mut self) -> Result<(), Error> {
        self.operation(REMOTE_RELEASE_ON, &[3, 0])?;
        self.operation(REMOTE_RELEASE_OFF, &[3]).map(|_| ())
    }

    /// Moves the AF point to (`x`, `y`) in sensor pixels, as touching the
    /// camera's screen does, without focusing (that's AF-ON's job). Sent as
    /// open-eos-control does over USB: TouchAfPosition with [3, x, y].
    pub fn touch_af(&mut self, x: u32, y: u32) -> Result<(), Error> {
        self.operation(TOUCH_AF_POSITION, &[3, x, y]).map(|_| ())
    }

    /// Back-button focus: DoAf (0x9154) while held, AfCancel (0x9160) on
    /// release, as open-eos-control does over USB.
    pub fn autofocus(&mut self, held: bool) -> Result<(), Error> {
        self.operation(if held { DO_AF } else { AF_CANCEL }, &[]).map(|_| ())
    }

    /// Live view magnification: 1 (whole picture), 5 or 10, as EOS Utility
    /// sends it (recorded 2026-10-29).
    pub fn zoom(&mut self, level: u32) -> Result<(), Error> {
        self.operation(ZOOM, &[level]).map(|_| ())
    }

    /// Where the magnified area sits, in sensor pixels (EOS Utility moved it in
    /// steps of 68 around 2716, 1856 on the R6 Mark III).
    pub fn zoom_position(&mut self, x: u32, y: u32) -> Result<(), Error> {
        self.operation(ZOOM_POSITION, &[x, y]).map(|_| ())
    }

    /// Depth of field preview: the lens stops down to the set aperture.
    /// The camera doesn't list this property, but EOS Utility sets it.
    pub fn depth_of_field_preview(&mut self, on: bool) -> Result<(), Error> {
        self.set_property(DEPTH_OF_FIELD_PREVIEW, on as u32)
    }

    /// Reads (and drops) what the camera has to report. EOS Utility asks
    /// continuously; the camera expects to be asked.
    pub fn events(&mut self) -> Result<(), Error> {
        self.event_data().map(|_| ())
    }

    /// What the camera reports since it was last asked: records of length,
    /// type, body (0xC189 a property's new value), ending with a type 0 record.
    pub fn event_data(&mut self) -> Result<Vec<u8>, Error> {
        let data = self.operation(GET_EVENT_DATA, &[])?;
        self.properties.apply(&data);
        Ok(data)
    }

    /// The camera's properties as reported so far (all of them arrive with
    /// the first report after live view starts).
    pub fn properties(&self) -> &Properties {
        &self.properties
    }

    /// Sets a camera property (SetDevicePropValueEx, 32-bit value).
    pub fn set_property(&mut self, property: u32, value: u32) -> Result<(), Error> {
        let (response, _) = self.transact(SET_DEVICE_PROP_VALUE_EX, &[], Some(&property_value(property, value)))?;
        match response {
            RESPONSE_OK => Ok(()),
            other => Err(response_error(SET_DEVICE_PROP_VALUE_EX, other)),
        }
    }

    /// Runs one operation that sends no data; returns what the camera sends back.
    fn operation(&mut self, code: u16, params: &[u32]) -> Result<Vec<u8>, Error> {
        match self.transact(code, params, None)? {
            (RESPONSE_OK, data) => Ok(data),
            (response, _) => Err(response_error(code, response)),
        }
    }

    /// One operation: sends `out` as its data when given; returns the camera's
    /// response code and any data it sends back.
    fn transact(&mut self, code: u16, params: &[u32], out: Option<&[u8]>) -> Result<(u16, Vec<u8>), Error> {
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
                other => return Err(Error::Protocol(format!("unexpected packet type {other} during 0x{code:04x}"))),
            }
        }
    }
}

/// What EOS Utility does just before it connects (recorded 2026-10-29): ask
/// the network for Canon cameras, then read the camera's UPnP description.
/// The camera drops a pairing request without it.
fn discover(camera: SocketAddr) -> Result<(), Error> {
    search(camera.ip())?;
    device_description(&camera.ip().to_string()).map(|_| ())
}

/// Asks the network for Canon cameras (SSDP M-SEARCH). Sent from the address
/// that reaches the camera, so that a phone on the camera's hotspot with
/// mobile data on doesn't send it out over mobile data, and to the camera
/// directly as well.
fn search(camera: IpAddr) -> Result<(), Error> {
    let search = "M-SEARCH * HTTP/1.1\r\nHOST: 239.255.255.250:1900\r\nMAN: \"ssdp:discover\"\r\nMX: 1\r\n\
                  ST: urn:schemas-canon-com:service:ICPO-WFTEOSSystemService:1\r\n\r\n";
    let route = UdpSocket::bind("0.0.0.0:0")?;
    route.connect((camera, 1900))?; // sends nothing: only picks the local address
    let udp = UdpSocket::bind((route.local_addr()?.ip(), 0))?;
    let multicast = udp.send_to(search.as_bytes(), "239.255.255.250:1900");
    let direct = udp.send_to(search.as_bytes(), (camera, 1900));
    if let (Err(error), Err(_)) = (multicast, direct) {
        return Err(error.into());
    }
    // The answer only tells us what we already know (the camera is there): wait briefly for it.
    udp.set_read_timeout(Some(Duration::from_millis(1500)))?;
    let mut answer = [0u8; 1024];
    let _ = udp.recv_from(&mut answer);
    Ok(())
}

/// The camera's UPnP description (GET /upnp/CameraDevDesc.xml on port
/// 49152), the whole HTTP answer. It names the camera's Wi-Fi function.
pub fn device_description(host: &str) -> Result<Vec<u8>, Error> {
    let http = socket_address(host, 49152)?;
    let mut stream = TcpStream::connect_timeout(&http, Duration::from_secs(3))?;
    stream.set_read_timeout(Some(Duration::from_secs(3)))?;
    let request = format!(
        "GET /upnp/CameraDevDesc.xml HTTP/1.1\r\nUser-Agent: wsCDeviceDetector\r\nHost: {http}\r\n\
         Cache-Control: no-cache\r\nConnection: close\r\n\r\n"
    );
    stream.write_all(request.as_bytes())?;
    let mut description = Vec::new();
    stream.read_to_end(&mut description)?;
    Ok(description)
}

fn socket_address(host: &str, port: u16) -> Result<SocketAddr, Error> {
    (host, port)
        .to_socket_addrs()?
        .next()
        .ok_or_else(|| Error::Connection("no address for the camera".into()))
}

fn connect(address: SocketAddr) -> Result<TcpStream, Error> {
    let stream = TcpStream::connect_timeout(&address, Duration::from_secs(5))
        .map_err(|e| Error::Connection(format!("can't reach the camera on port {PORT}: {e}")))?;
    stream.set_nodelay(true)?;
    Ok(stream)
}

/// A packet: total length and type (both u32, little-endian), then the payload.
fn send(stream: &mut TcpStream, kind: u32, payload: &[u8]) -> Result<(), Error> {
    let mut bytes = ((8 + payload.len()) as u32).to_le_bytes().to_vec();
    bytes.extend_from_slice(&kind.to_le_bytes());
    bytes.extend_from_slice(payload);
    Ok(stream.write_all(&bytes)?)
}

fn receive(stream: &mut TcpStream) -> Result<(u32, Vec<u8>), Error> {
    let mut header = [0u8; 8];
    stream
        .read_exact(&mut header)
        .map_err(|e| Error::Connection(format!("no answer from the camera: {e}")))?;
    let length = u32::from_le_bytes([header[0], header[1], header[2], header[3]]) as usize;
    let kind = u32::from_le_bytes([header[4], header[5], header[6], header[7]]);
    if !(8..=MAX_PACKET).contains(&length) {
        return Err(Error::Protocol(format!("bad packet length {length}")));
    }
    let mut payload = vec![0; length - 8];
    stream.read_exact(&mut payload)?;
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

/// The camera's properties as its reports describe them: each one's current
/// value (0xC189 records) and the values it allows now (0xC18A records).
#[derive(Default)]
pub struct Properties {
    values: std::collections::HashMap<u32, u32>,
    allowed: std::collections::HashMap<u32, Vec<u32>>,
}

impl Properties {
    pub fn value(&self, property: u32) -> Option<u32> {
        self.values.get(&property).copied()
    }

    pub fn allowed(&self, property: u32) -> &[u32] {
        self.allowed.get(&property).map_or(&[], Vec::as_slice)
    }

    /// Takes in a GetEventData reply: records of length, type, body, ending
    /// with a type 0 record.
    fn apply(&mut self, data: &[u8]) {
        let word = |b: &[u8], i: usize| b.get(i..i + 4).map(|w| u32::from_le_bytes(w.try_into().unwrap()));
        let mut rest = data;
        while let (Some(size), Some(kind)) = (word(rest, 0), word(rest, 4)) {
            let size = size as usize;
            if kind == 0 || size < 8 || size > rest.len() {
                break;
            }
            let body = &rest[8..size];
            match (kind, word(body, 0), word(body, 4)) {
                (PROPERTY_VALUE_CHANGED, Some(property), Some(value)) => {
                    self.values.insert(property, value);
                }
                // Property, value type, count, then the values (32-bit each).
                (ALLOWED_VALUES_CHANGED, Some(property), Some(_)) => {
                    let count = word(body, 8).unwrap_or(0) as usize;
                    let values = (0..count).map_while(|i| word(body, 12 + 4 * i)).collect();
                    self.allowed.insert(property, values);
                }
                _ => {}
            }
            rest = &rest[size..];
        }
    }
}

/// An answer other than OK to `operation`.
fn response_error(operation: u16, response: u16) -> Error {
    match response {
        RESPONSE_BUSY | RESPONSE_NOT_READY => Error::Busy,
        _ => Error::Response { operation, response },
    }
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
pub fn viewfinder_jpeg(data: &[u8]) -> Option<&[u8]> {
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

/// How many of live view's samples fall on each of 256 levels, per channel.
#[derive(Debug, PartialEq)]
pub struct Histogram {
    pub luminance: [u32; 256],
    pub red: [u32; 256],
    pub green: [u32; 256],
    pub blue: [u32; 256],
}

/// The camera's histogram from a GetViewFinderData reply: block 0x11 holds
/// 4 x 256 u32, channels Y, R, G, B in that order (checked 2026-10-29 against
/// the R6 Mark III's own frame).
pub fn viewfinder_histogram(data: &[u8]) -> Option<Histogram> {
    let body: &[u8; 4096] = viewfinder_block(data, 0x11)?.try_into().ok()?;
    let words = body.as_chunks::<4>().0;
    let channel = |c: usize| std::array::from_fn(|level| u32::from_le_bytes(words[256 * c + level]));
    Some(Histogram { luminance: channel(0), red: channel(1), green: channel(2), blue: channel(3) })
}

/// The body of the first block of type `wanted` in a GetViewFinderData reply.
pub fn viewfinder_block(data: &[u8], wanted: u32) -> Option<&[u8]> {
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
    fn busy_answers_are_busy() {
        assert_eq!(response_error(0x9128, 0x2019), Error::Busy);
        assert_eq!(response_error(0x9116, 0xa102), Error::Busy);
        assert_eq!(response_error(0x9114, 0x2002), Error::Response { operation: 0x9114, response: 0x2002 });
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
    fn reports_give_values_and_allowed_values() {
        // As the R6 Mark III reported them (2026-10-29): ISO's allowed list
        // (0xC18A: property, type 3, count, values), a property value
        // (0xC189), then the end record.
        let record = |kind: u32, body: &[u32]| {
            let mut out = ((8 + 4 * body.len()) as u32).to_le_bytes().to_vec();
            out.extend_from_slice(&kind.to_le_bytes());
            for word in body {
                out.extend_from_slice(&word.to_le_bytes());
            }
            out
        };
        let mut data = record(0xc18a, &[0xd103, 3, 3, 0x48, 0x4b, 0x4d]);
        data.extend(record(0xc189, &[0xd103, 0x4b]));
        data.extend(record(0, &[]));
        let mut properties = Properties::default();
        properties.apply(&data);
        assert_eq!(properties.value(0xd103), Some(0x4b));
        assert_eq!(properties.allowed(0xd103), &[0x48, 0x4b, 0x4d]);
        assert_eq!(properties.value(0xd101), None);
        assert!(properties.allowed(0xd101).is_empty());
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
    fn viewfinder_data_gives_the_histogram() {
        // Block 0x11: 4 channels (Y, R, G, B) x 256 levels x u32, after another block.
        let mut data = 16u32.to_le_bytes().to_vec();
        data.extend_from_slice(&0x0eu32.to_le_bytes());
        data.extend_from_slice(&[0; 8]);
        data.extend_from_slice(&(8 + 4096u32).to_le_bytes());
        data.extend_from_slice(&0x11u32.to_le_bytes());
        for channel in 0..4u32 {
            for level in 0..256u32 {
                data.extend_from_slice(&(channel * 1000 + level).to_le_bytes());
            }
        }
        let histogram = viewfinder_histogram(&data).unwrap();
        assert_eq!((histogram.luminance[0], histogram.luminance[255]), (0, 255));
        assert_eq!((histogram.red[0], histogram.red[7]), (1000, 1007));
        assert_eq!(histogram.green[128], 2128);
        assert_eq!(histogram.blue[255], 3255);
        assert_eq!(viewfinder_histogram(&data[..16]), None); // no histogram block
        assert_eq!(viewfinder_histogram(&data[..16 + 8 + 4092]), None); // cut short
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
