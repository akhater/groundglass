//! Headless camera checks: the same camera code as the app, without the window.
//!
//!   groundglass-cli <address> info
//!   groundglass-cli <address> live [seconds] [--display-off] [--save frame.jpg] [--stamp dir]
//!   groundglass-cli <address> stop
//!   groundglass-cli <address> get <iso|av|tv>
//!   groundglass-cli <address> set <iso|av|tv> <value>
//!   groundglass-cli <address> frame <file>   (one frame from live view that's already on)
//!   groundglass-cli <address> shoot [--no-af]   (takes a photo)
//!   groundglass-cli <address> focus <x> <y>   (fractions across the picture; live view on)
//!   groundglass-cli <address> af [seconds]   (holds AF-ON, printing the AF frames)
//!   groundglass-cli <address> details [seconds]   (level and AF frames; live view on)
//!   groundglass-cli <address> probe [--live] <path>...   (GET each /ccapi path, print the reply)
//!   groundglass-cli <host> ptp-info   (PTP/IP, camera in "Connect to EOS Utility"; reads only)
//!   groundglass-cli <host> ptp-live [seconds] [--save frame.jpg]   (PTP/IP live view speed)
//!   groundglass-cli <host> ptp-zoom <dir>   (x1, x5, x10, DOF preview on/off; a frame of each in dir)
//!   groundglass-cli <host> ptp-screen <dir>   (live view to us only, then to the camera's screen too, then x10 from x1)
//!
//! `--stamp dir` saves every 10th frame as `<ms>.jpg`, named by the PC clock
//! (milliseconds within 100 s) when it arrived. Point the camera at a clock
//! showing the same value: the difference is the true lag.

use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use groundglass_lib::ccapi::{Camera, ExposureSetting};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let Err(message) = run(&args) {
        eprintln!("error: {message}");
        std::process::exit(1);
    }
}

fn run(args: &[String]) -> Result<(), String> {
    let (Some(address), Some(command)) = (args.first(), args.get(1)) else {
        return Err("usage: groundglass-cli <address> info | live [seconds] [--display-off] [--save file] | stop | get <setting> | set <setting> <value> | frame <file> | shoot [--no-af] | focus <x> <y> | af [seconds] | details [seconds] | probe [--live] <path>... | ptp-info | ptp-live [seconds] [--save file]".into());
    };
    // PTP/IP is the camera's other Wi-Fi mode: no CCAPI session to open first.
    match command.as_str() {
        "ptp-info" => return ptp_info(address),
        "ptp-live" => {
            let seconds = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(5);
            let save = args.iter().position(|a| a == "--save").and_then(|i| args.get(i + 1));
            return ptp_live(address, Duration::from_secs(seconds), save);
        }
        "ptp-zoom" => return ptp_zoom(address, args.get(2).ok_or("ptp-zoom needs a folder for the frames")?),
        "ptp-screen" => return ptp_screen(address, args.get(2).ok_or("ptp-screen needs a folder for the frames")?),
        _ => {}
    }
    let (camera, info) = Camera::connect(address).map_err(|e| format!("{e:?}"))?;
    println!("{} (firmware {})", info.model, info.firmware);
    match command.as_str() {
        "info" => Ok(()),
        "stop" => camera.stop_live_view().map_err(|e| format!("{e:?}")),
        "live" => {
            let seconds = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(5);
            let display_on = !args.iter().any(|a| a == "--display-off");
            let option = |name: &str| args.iter().position(|a| a == name).and_then(|i| args.get(i + 1));
            live(&camera, Duration::from_secs(seconds), display_on, option("--save"), option("--stamp"))
        }
        "get" => {
            let state = camera.exposure_setting(setting(args.get(2))?).map_err(|e| format!("{e:?}"))?;
            println!("{} (choices: {})", state.value, state.choices.join(" "));
            Ok(())
        }
        "set" => {
            let value = args.get(3).ok_or("set needs a value")?;
            camera.set_exposure_setting(setting(args.get(2))?, value).map_err(|e| format!("{e:?}"))
        }
        "probe" => probe(&camera, &args[2..]),
        "frame" => {
            let path = args.get(2).ok_or("frame needs a file")?;
            let jpeg = camera.frame().map_err(|e| format!("{e:?}"))?;
            std::fs::write(path, &jpeg).map_err(|e| e.to_string())?;
            println!("{} KB saved to {path}", jpeg.len() / 1024);
            Ok(())
        }
        "shoot" => {
            let autofocus = !args.iter().any(|a| a == "--no-af");
            camera.shoot(autofocus).map_err(|e| format!("{e:?}"))
        }
        "focus" => {
            let fraction = |i: usize| args.get(i).and_then(|s| s.parse().ok()).ok_or("focus needs x and y, 0 to 1");
            camera.set_focus_point(fraction(2)?, fraction(3)?).map_err(|e| format!("{e:?}"))
        }
        "af" => {
            let seconds: u64 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(2);
            let frames = || {
                let details = camera.read_raw("/ver100/shooting/liveview/flipdetail?kind=info").unwrap_or_default();
                let text = String::from_utf8_lossy(&details);
                let start = text.find("\"afframe\"").unwrap_or(0);
                let end = text[start..].find(']').map_or(text.len(), |i| start + i + 1);
                text[start..end].to_string()
            };
            println!("before: {}", frames());
            let pressed = Instant::now();
            camera.autofocus(true).map_err(|e| format!("{e:?}"))?;
            while pressed.elapsed() < Duration::from_secs(seconds) {
                println!("{:>5} ms: {}", pressed.elapsed().as_millis(), frames());
                std::thread::sleep(Duration::from_millis(150));
            }
            camera.autofocus(false).map_err(|e| format!("{e:?}"))?;
            println!("after: {}", frames());
            Ok(())
        }
        "details" => {
            let seconds: u64 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(5);
            let until = Instant::now() + Duration::from_secs(seconds);
            while Instant::now() < until {
                println!("{:?}", camera.live_details());
                std::thread::sleep(Duration::from_millis(500));
            }
            Ok(())
        }
        other => Err(format!("unknown command: {other}")),
    }
}

/// Reads each path, with live view running when `--live` is given (some
/// replies only exist then), and always stops it again.
fn probe(camera: &Camera, args: &[String]) -> Result<(), String> {
    let live = args.iter().any(|a| a == "--live");
    if live {
        camera.start_live_view(true).map_err(|e| format!("{e:?}"))?;
    }
    for path in args.iter().filter(|a| *a != "--live") {
        match camera.read_raw(path) {
            Ok(body) => println!("== {path}
{}", String::from_utf8_lossy(&body)),
            Err(error) => println!("== {path}
{error:?}"),
        }
    }
    if live {
        camera.stop_live_view().map_err(|e| format!("{e:?}"))?;
    }
    Ok(())
}

/// Pairs over PTP/IP, reads what the camera supports, and closes. Changes nothing.
fn ptp_info(host: &str) -> Result<(), String> {
    use groundglass_lib::ptpip::Session;
    println!("Connecting over PTP/IP. The first time, press OK on the camera to pair (up to 90 s).");
    let mut session = Session::open(host, Duration::from_secs(90))?;
    let info = session.device_info();
    session.close()?;
    let info = info?;
    println!("{} {} (version {})", info.manufacturer, info.model, info.version);
    let has_op = |code: u16| if info.operations.contains(&code) { "yes" } else { "no" };
    for (code, what) in [
        (0x9153, "live view frames (GetViewFinderData)"),
        (0x9158, "live view magnification (Zoom)"),
        (0x9159, "magnified area position (ZoomPosition)"),
        (0x9110, "setting properties (SetDevicePropValueEx)"),
    ] {
        println!("  0x{code:04x} {what}: {}", has_op(code));
    }
    // The R6 Mark III doesn't list it, yet EOS Utility sets it (recorded 2026-10-29).
    println!("  depth of field preview: property 0xd1b2 through 0x9110, unlisted but used by EOS Utility");
    let list = |codes: &[u16]| codes.iter().map(|c| format!("{c:04x}")).collect::<Vec<_>>().join(" ");
    println!("{} operations: {}", info.operations.len(), list(&info.operations));
    println!("{} properties: {}", info.properties.len(), list(&info.properties));
    Ok(())
}

/// Live view over PTP/IP for `length`: frames per second and size, as `live`
/// does over CCAPI. Always turns live view output off and closes the session.
fn ptp_live(host: &str, length: Duration, save: Option<&String>) -> Result<(), String> {
    use groundglass_lib::ptpip::Session;
    let mut session = Session::open(host, Duration::from_secs(90))?;
    let started = Instant::now();
    let result = (|| {
        session.start_live_view()?;
        println!("live view started in {} ms", started.elapsed().as_millis());
        let (mut frames, mut empty, mut bytes, mut repeats) = (0u32, 0u32, 0usize, 0u32);
        let mut last = Vec::new();
        let pulling = Instant::now();
        let mut events_at = Instant::now();
        while pulling.elapsed() < length {
            match session.frame()? {
                // A frame identical to the last is the camera repeating itself, not a new picture.
                Some(jpeg) if jpeg == last => repeats += 1,
                Some(jpeg) => {
                    frames += 1;
                    bytes += jpeg.len();
                    last = jpeg;
                }
                None => {
                    empty += 1;
                    std::thread::sleep(Duration::from_millis(20));
                }
            }
            // The camera expects its events to be read now and then, as EOS Utility does.
            if events_at.elapsed() > Duration::from_secs(1) {
                session.events()?;
                events_at = Instant::now();
            }
        }
        let secs = pulling.elapsed().as_secs_f64();
        println!(
            "{frames} new frames in {secs:.1} s = {:.1} fps, average {} KB, {empty} not ready, {repeats} repeats",
            frames as f64 / secs,
            bytes / frames.max(1) as usize / 1024
        );
        if let Some(path) = save.filter(|_| !last.is_empty()) {
            std::fs::write(path, &last).map_err(|e| e.to_string())?;
            println!("last frame saved to {path}");
        }
        Ok(())
    })();
    let stopped = session.stop_live_view();
    println!("live view output off: {stopped:?}");
    let closed = session.close();
    println!("session closed: {closed:?}");
    result.and(stopped).and(closed)
}

/// Steps through x1, x5, x10, back to x1, then DOF preview on and off, saving
/// a frame after each (`<dir>/1-x1.jpg` ...). Always restores x1, DOF off,
/// live view output off, and closes the session.
fn ptp_zoom(host: &str, dir: &str) -> Result<(), String> {
    use groundglass_lib::ptpip::Session;
    let mut session = Session::open(host, Duration::from_secs(90))?;
    let result = (|| {
        session.start_live_view()?;
        let grab = |session: &mut Session, name: &str| -> Result<(), String> {
            // Let the camera settle, then keep the newest frame.
            let settle = Instant::now();
            let mut last = None;
            while settle.elapsed() < Duration::from_millis(1500) {
                if let Some(jpeg) = session.frame()? {
                    last = Some(jpeg);
                }
            }
            session.events()?;
            let jpeg = last.ok_or_else(|| format!("no frame for {name}"))?;
            let path = format!("{dir}/{name}.jpg");
            std::fs::write(&path, &jpeg).map_err(|e| e.to_string())?;
            println!("{name}: {} KB saved to {path}", jpeg.len() / 1024);
            Ok(())
        };
        grab(&mut session, "1-x1")?;
        session.zoom(5)?;
        grab(&mut session, "2-x5")?;
        session.zoom(10)?;
        grab(&mut session, "3-x10")?;
        session.zoom(1)?;
        grab(&mut session, "4-x1-again")?;
        session.depth_of_field_preview(true)?;
        grab(&mut session, "5-dof-on")?;
        session.depth_of_field_preview(false)?;
        grab(&mut session, "6-dof-off")
    })();
    let restored = [
        ("x1", session.zoom(1)),
        ("DOF preview off", session.depth_of_field_preview(false)),
        ("live view output off", session.stop_live_view()),
    ];
    for (what, outcome) in &restored {
        println!("{what}: {outcome:?}");
    }
    let closed = session.close();
    println!("session closed: {closed:?}");
    result.and(closed)
}

/// Five seconds with live view to us only, five with the camera's screen too
/// (watch the camera), then x10 straight from x1 held three seconds. Always
/// restores x1, live view output off, and closes the session.
fn ptp_screen(host: &str, dir: &str) -> Result<(), String> {
    use groundglass_lib::ptpip::Session;
    let mut session = Session::open(host, Duration::from_secs(90))?;
    let result = (|| {
        let run = |session: &mut Session, length: Duration| -> Result<Option<Vec<u8>>, String> {
            let started = Instant::now();
            let mut last = None;
            while started.elapsed() < length {
                if let Some(jpeg) = session.frame()? {
                    last = Some(jpeg);
                }
            }
            session.events()?;
            Ok(last)
        };
        session.start_live_view()?;
        println!("live view to the computer only (2), 5 s: is the camera's screen off?");
        run(&mut session, Duration::from_secs(5))?;
        println!("camera screen too (3): {:?}", session.live_view_output(true));
        run(&mut session, Duration::from_secs(5))?;
        session.zoom(10)?;
        println!("x10 from x1, 3 s");
        let jpeg = run(&mut session, Duration::from_secs(3))?.ok_or("no frame at x10")?;
        let path = format!("{dir}/7-x10-direct.jpg");
        std::fs::write(&path, &jpeg).map_err(|e| e.to_string())?;
        println!("x10: {} KB saved to {path}", jpeg.len() / 1024);
        Ok(())
    })();
    println!("x1: {:?}", session.zoom(1));
    println!("live view output off: {:?}", session.stop_live_view());
    let closed = session.close();
    println!("session closed: {closed:?}");
    result.and(closed)
}

fn setting(name: Option<&String>) -> Result<ExposureSetting, String> {
    match name.map(String::as_str) {
        Some("iso") => Ok(ExposureSetting::Iso),
        Some("av") => Ok(ExposureSetting::Av),
        Some("tv") => Ok(ExposureSetting::Tv),
        _ => Err("expected iso, av or tv".into()),
    }
}

/// Starts live view, pulls frames as fast as the camera gives them, then always stops.
fn live(
    camera: &Camera,
    length: Duration,
    display_on: bool,
    save: Option<&String>,
    stamp: Option<&String>,
) -> Result<(), String> {
    let started = Instant::now();
    if let Err(error) = camera.start_live_view(display_on) {
        let _ = camera.stop_live_view();
        return Err(format!("start failed: {error:?}"));
    }
    println!("live view started in {} ms", started.elapsed().as_millis());
    let mut frames = 0u32;
    let mut bytes = 0usize;
    let mut last = Vec::new();
    let mut failure = None;
    let pulling = Instant::now();
    while pulling.elapsed() < length {
        match camera.frame() {
            Ok(jpeg) => {
                frames += 1;
                bytes += jpeg.len();
                if let Some(dir) = stamp.filter(|_| frames.is_multiple_of(10)) {
                    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default();
                    let path = format!("{dir}/{:05}.jpg", now.as_millis() % 100_000);
                    std::fs::write(path, &jpeg).map_err(|e| e.to_string())?;
                }
                last = jpeg;
            }
            Err(error) => {
                failure = Some(format!("{error:?}"));
                break;
            }
        }
    }
    let secs = pulling.elapsed().as_secs_f64();
    println!(
        "{frames} frames in {secs:.1} s = {:.1} fps, average {} KB",
        frames as f64 / secs,
        bytes / frames.max(1) as usize / 1024
    );
    if let Some(path) = save.filter(|_| !last.is_empty()) {
        std::fs::write(path, &last).map_err(|e| e.to_string())?;
        println!("last frame saved to {path}");
    }
    let stopped = camera.stop_live_view().map_err(|e| format!("{e:?}"));
    match failure {
        Some(error) => Err(format!("frame failed: {error}")),
        None => stopped,
    }
}
