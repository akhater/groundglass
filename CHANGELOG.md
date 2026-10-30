# Changelog

Newest first.

## 26.44.5 - unreleased

### Added

- **New version notice:** when a newer release is published, its version shows
  beside the title before Start and opens the release page. Checked at start
  and every 6 hours, silently; nothing is downloaded or installed.

## 26.44.4 - 2026-10-29

### Added

- **ETTR (expose to the right):** one tap beside AF-ON sets the brightest
  exposure that clips nothing, from the camera's live histogram, in a second
  or two. Pin up to two of shutter, aperture and ISO from their scale (a lock
  shows by the name); ETTR moves the others, raising ISO last. A spot under
  0.1% of the picture (a lamp, a glint) may clip. A setting at the end of its
  range flashes; turning a dial stops it. Needs M with ISO not on auto.
- **Battery:** the camera's battery level next to the frame rate, red at 20%
  and under, in both Wi-Fi modes. It is read every 30 seconds.
- **Portrait shooting:** with the camera held vertical (either way, or upside
  down), live view turns upright, as on the camera's own screen. The AF
  frames, tap to focus, guides, crop (portrait ratios) and level turn with it;
  the level reads from vertical. On a phone held upright, the picture gets a
  taller area. A rotation lock in the aids keeps the current turn whatever
  the camera does.

### Changed

- **Magnify:** one button steps through x1, x5 and x10 and back, as the
  camera's magnify button does, and shows the current level (EOS Utility mode;
  dimmed in CCAPI mode).
- **Live view screen:** the picture takes all the room the screen allows. On
  a phone on its side, the aids (zebra, peaking, guides, crop, level,
  histogram, magnify, depth of field preview) sit in a band on the left with
  the frame rate and battery, and the camera (shutter, aperture, ISO, look,
  AF-ON, shutter button) in a band on the right. Upright, the aids sit under
  the picture and the camera at the bottom; on a large screen both bands float
  over the picture and fold away. Each aid shows its setting and lights up when
  on. Choices open beside the control that asked: a scale with - and + for
  exposure, all the choices at once for the rest, Rotate in the guides.
- **Histogram:** sits in the picture's bottom left corner, beside the choices
  when they open over that corner instead of hiding under them; large under
  the aids on a phone held upright.
- **Overlays:** guides, crop frame, AF frames and the level's markers have a
  dark edge, so they show on a bright sky.

### Fixed

- **Settings the camera sets:** in Av, Tv and P, the shutter or aperture the
  camera sets shows its value with a red lock, apart from the gold lock of an
  ETTR pin, and doesn't open; a tap says why ("Set by the camera in Av"). In
  EOS Utility mode it used to open and list a raw code such as 0x1500. ETTR is
  dimmed in these modes and says on a tap that it needs manual exposure.
- **Picture after stopping:** the picture goes black when live view stops,
  the app is hidden or the camera is lost, instead of keeping the last frame.
- **Level on a dark picture:** the roll and tilt tracks have a light edge, so
  they show on dark scenes too.
- **AF frames in EOS Utility mode:** drawn where and as the camera draws them
  (around the areas in focus, or the frame following a subject after a tap),
  green when One-Shot holds focus and blue while Servo tracks, as in CCAPI
  mode.
- **Battery in EOS Utility mode:** the camera's exact percent instead of a
  rounded step. Where a camera or battery gives only a step (full, 75, 50,
  25, low), in either mode, the gauge shows it with no number.

## 26.44.3 - 2026-10-29

### Added

- **EOS Utility mode:** live view also works with the camera's Wi-Fi set to
  "Connect to EOS Utility" (PTP/IP). Groundglass asks the camera which Wi-Fi
  function it runs, so the address field works the same for both. The first
  time, press OK on the camera to pair. Looks, zebra, peaking, crop, guides,
  level, exposure, tap to focus, shoot and AF-ON work in both modes. Live view
  runs in Picture Style Neutral, and the camera's own style is put back on
  stop, exit and hide.
- **x5 / x10 and DOF preview** (EOS Utility mode): magnify around the AF
  point, and stop the lens down from the app. In CCAPI mode these buttons are
  dimmed, since the camera doesn't offer them there.
- **Histogram:** the camera's brightness histogram over the picture, in both
  modes.
- `groundglass-cli`: `ptp-*` commands for headless checks in EOS Utility mode.

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
