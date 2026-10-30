# Groundglass

**Your looks and zebra on a Canon camera's live view.**

Groundglass shows a Canon camera's live view on a phone or computer over
Wi-Fi, through your own `.cube` or HaldCLUT PNG LUTs, with zebra, while you shoot stills.

> **Status: early development.** Tested with the EOS R6 Mark III. Android and
> Windows are the main targets; macOS and Linux builds are previews.

## Features

- Live view over Wi-Fi (Canon CCAPI), on your network or the camera's own
  access point.
- Zebra from 70% to 100%.
- Your own looks (`.cube` or HaldCLUT PNG LUTs) on live view.
- Shoot, AF-ON, ISO, aperture and shutter speed from the app.
- Camera screen off while connected, to save battery.
- `groundglass-cli` for live view without the window.

## Getting started

1. Enable CCAPI on the camera (Canon's CCAPI activation) and connect it to
   Wi-Fi, or turn on its access point and join it.
2. Open Groundglass, type the camera's address (e.g. `192.168.1.2:8080`) and
   tap Start.

Downloads are on the [Releases](../../releases) page: an APK for Android,
installers for Windows, macOS and Linux.

## Building

Needs Node 22 and Rust.

```
npm ci
npm run tauri dev
```

Releases are built by GitHub Actions when a release is published; see
`CHANGELOG.md` for the versioning.
