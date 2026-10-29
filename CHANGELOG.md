# Changelog

Newest first.

## 26.44.3 - unreleased

### Added

- **EOS Utility mode:** live view also works with the camera's Wi-Fi set to
  "Connect to EOS Utility" (PTP/IP), with looks, zebra, peaking, crop, grid,
  level, shoot and AF-ON. Groundglass asks the camera which Wi-Fi function it
  runs, so the address field works the same for both. The first time, press
  OK on the camera to pair. Exposure and tap to focus are CCAPI only for now.

## 26.44.2 - 2026-10-28

### Added

- **Focus peaking:** edges in focus are painted on live view, Off / Low / Mid /
  High. It measures how abrupt an edge is, not how contrasty, so blurred
  backgrounds stay clean. With the camera's DOF preview held, what is painted
  is the depth of field.
- **Tap to focus:** tap or click the picture to move the camera's AF point
  there. The camera's own AF frame is drawn where it is, also when tracking:
  green once One-Shot holds focus, blue while Servo tracks.
- **Level:** the camera's electronic level, as a roll scale along the bottom
  and a tilt scale up the side, green when level.
- **Composition guides:** thirds, diagonal lines, golden triangle, golden
  spiral (turns to portrait), phi grid, armature and centre lines.
- **Crop frame:** 1:1, 5:4, 4:3 and 16:9, visual only; the guides fit the crop.
- `groundglass-cli`: `get` / `set` exposure, `shoot`, `focus`, `af`,
  `details`, `frame` and a read-only `probe`, for headless checks.

### Fixed

- **Shoot no longer drops live view.** While a photo is taken the camera
  refuses live view frames ("During shooting or recording"); live view now
  waits for it instead of reconnecting.
- **Shoot doesn't autofocus.** Focus stays with AF-ON, as on a camera set up
  with back-button focus.

## 26.44.1 - 2026-10-28

First release.

### Added

- **Live view over Wi-Fi** from a Canon camera with CCAPI (tested on the EOS R6
  Mark III), on the camera's network or the camera's own access point.
- **Zebra** on the camera's picture, from 70% to 100%.
- **Looks:** load your own `.cube` LUTs and see live view through them. The
  camera's picture is converted to linear first, so looks made for linear
  input apply as intended.
- **Shoot**, **AF-ON** (hold) and **ISO, aperture and shutter speed** from the
  app, kept in step with the camera's dials.
- **Camera screen off** while connected, to save battery; it always comes back
  on when live view stops or the app closes.
- **Recent cameras:** the last ten addresses, one tap away.
- A layout for phones and any window size, and `groundglass-cli` to run live
  view without the window.

## Versioning: `yy.isoWeek.release`

```
26.44.1
 |  |  +-- release within that week, counting from 1
 |  +----- ISO week number
 +-------- ISO year, two digits
```

The version says when it was built; two digits because Windows installers cap
the major version at 255. **Bump immediately after releasing, never before**,
so a number one ahead of the last release means "built from source". The
release commit sets the true `yy.ww.n`. Current week: `date +%g.%V`.

The version lives in three places that must match: `package.json`,
`src-tauri/Cargo.toml`, `src-tauri/tauri.conf.json`.
