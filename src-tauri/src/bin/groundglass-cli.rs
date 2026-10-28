//! Headless camera checks: the same camera code as the app, without the window.
//!
//!   groundglass-cli <address> info
//!   groundglass-cli <address> live [seconds] [--display-off] [--save frame.jpg] [--stamp dir]
//!   groundglass-cli <address> stop
//!   groundglass-cli <address> get <iso|av|tv>
//!   groundglass-cli <address> set <iso|av|tv> <value>
//!   groundglass-cli <address> probe [--live] <path>...   (GET each /ccapi path, print the reply)
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
        return Err("usage: groundglass-cli <address> info | live [seconds] [--display-off] [--save file] | stop | get <setting> | set <setting> <value> | probe [--live] <path>...".into());
    };
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
