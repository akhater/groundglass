# Changelog

Newest first.

## 26.44.2 - unreleased

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
