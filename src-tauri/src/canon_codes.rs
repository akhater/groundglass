//! Canon EOS exposure codes (PTP properties 0xD103 ISO, 0xD101 aperture,
//! 0xD102 shutter speed) written as CCAPI writes them, so both Wi-Fi functions
//! show the same: "6400", "f9.0", "f11", "1/125", "0\"8", "auto", "bulb".
//! Codes from libgphoto2's camlibs/ptp2/config.c (canon_isospeed, canon_aperture,
//! canon_shutterspeed; LGPL-2.1+, Marcus Meissner and contributors), with long
//! shutter speeds written the way Canon writes them. Codes count eighths of a
//! stop: +3 and +5 are thirds, +4 a half.

/// The ISO speed, or the code in hex if it isn't one we know.
pub fn iso(code: u32) -> String {
    label(ISO, code)
}

/// The f-number, or the code in hex if it isn't one we know.
pub fn aperture(code: u32) -> String {
    label(APERTURE, code)
}

/// The shutter speed, or the code in hex if it isn't one we know.
pub fn shutter_speed(code: u32) -> String {
    label(SHUTTER_SPEED, code)
}

fn label(table: &[(u32, &str)], code: u32) -> String {
    table
        .iter()
        .find(|(known, _)| *known == code)
        .map_or_else(|| format!("0x{code:02x}"), |(_, label)| label.to_string())
}

const ISO: &[(u32, &str)] = &[
    (0x00, "auto"),
    (0x01, "auto"),
    (0x28, "6"),
    (0x30, "12"),
    (0x38, "25"),
    (0x40, "50"),
    (0x43, "64"),
    (0x45, "80"),
    (0x48, "100"),
    (0x4b, "125"),
    (0x4d, "160"),
    (0x50, "200"),
    (0x53, "250"),
    (0x55, "320"),
    (0x58, "400"),
    (0x5b, "500"),
    (0x5d, "640"),
    (0x60, "800"),
    (0x63, "1000"),
    (0x65, "1250"),
    (0x68, "1600"),
    (0x6b, "2000"),
    (0x6d, "2500"),
    (0x70, "3200"),
    (0x73, "4000"),
    (0x75, "5000"),
    (0x78, "6400"),
    (0x7b, "8000"),
    (0x7d, "10000"),
    (0x80, "12800"),
    (0x83, "16000"),
    (0x85, "20000"),
    (0x88, "25600"),
    (0x8b, "32000"),
    (0x8d, "40000"),
    (0x90, "51200"),
    (0x93, "64000"),
    (0x95, "80000"),
    (0x98, "102400"),
    (0xa0, "204800"),
    (0xa8, "409600"),
    (0xb0, "819200"),
];

const APERTURE: &[(u32, &str)] = &[
    (0x00, "auto"),
    (0xb0, "auto"),
    (0xffff, "auto"),
    (0x08, "f1.0"),
    (0x0b, "f1.1"),
    (0x0c, "f1.2"),
    (0x0d, "f1.2"),
    (0x10, "f1.4"),
    (0x13, "f1.6"),
    (0x14, "f1.8"),
    (0x15, "f1.8"),
    (0x18, "f2.0"),
    (0x1b, "f2.2"),
    (0x1c, "f2.5"),
    (0x1d, "f2.5"),
    (0x20, "f2.8"),
    (0x23, "f3.2"),
    (0x24, "f3.5"),
    (0x25, "f3.5"),
    (0x28, "f4.0"),
    (0x2b, "f4.5"),
    (0x2c, "f4.5"),
    (0x2d, "f5.0"),
    (0x30, "f5.6"),
    (0x33, "f6.3"),
    (0x34, "f6.7"),
    (0x35, "f7.1"),
    (0x38, "f8.0"),
    (0x3b, "f9.0"),
    (0x3c, "f9.5"),
    (0x3d, "f10"),
    (0x40, "f11"),
    (0x43, "f13"),
    (0x44, "f13"),
    (0x45, "f14"),
    (0x48, "f16"),
    (0x4b, "f18"),
    (0x4c, "f19"),
    (0x4d, "f20"),
    (0x50, "f22"),
    (0x53, "f25"),
    (0x54, "f27"),
    (0x55, "f29"),
    (0x58, "f32"),
    (0x5b, "f36"),
    (0x5c, "f38"),
    (0x5d, "f40"),
    (0x60, "f45"),
    (0x63, "f51"),
    (0x64, "f54"),
    (0x65, "f57"),
    (0x68, "f64"),
    (0x6b, "f72"),
    (0x6c, "f76"),
    (0x6d, "f81"),
    (0x70, "f91"),
];

// libgphoto2 has 0x14 "20.3", 0x1d "10.3" and 0x23 "6.3": half and third
// stops that Canon shows as 20", 10" and 6". 1/10000 and 1/12800 (0xa3, 0xa5)
// aren't in its table; they follow the same eighths.
const SHUTTER_SPEED: &[(u32, &str)] = &[
    (0x00, "auto"),
    (0x04, "bulb"),
    (0x0c, "bulb"),
    (0x10, "30\""),
    (0x13, "25\""),
    (0x14, "20\""),
    (0x15, "20\""),
    (0x18, "15\""),
    (0x1b, "13\""),
    (0x1c, "10\""),
    (0x1d, "10\""),
    (0x20, "8\""),
    (0x23, "6\""),
    (0x24, "6\""),
    (0x25, "5\""),
    (0x28, "4\""),
    (0x2b, "3\"2"),
    (0x2c, "3\""),
    (0x2d, "2\"5"),
    (0x30, "2\""),
    (0x33, "1\"6"),
    (0x34, "1\"5"),
    (0x35, "1\"3"),
    (0x38, "1\""),
    (0x3b, "0\"8"),
    (0x3c, "0\"7"),
    (0x3d, "0\"6"),
    (0x40, "0\"5"),
    (0x43, "0\"4"),
    (0x44, "0\"3"),
    (0x45, "0\"3"),
    (0x48, "1/4"),
    (0x4b, "1/5"),
    (0x4c, "1/6"),
    (0x4d, "1/6"),
    (0x50, "1/8"),
    (0x53, "1/10"),
    (0x54, "1/10"),
    (0x55, "1/13"),
    (0x58, "1/15"),
    (0x5b, "1/20"),
    (0x5c, "1/20"),
    (0x5d, "1/25"),
    (0x60, "1/30"),
    (0x63, "1/40"),
    (0x64, "1/45"),
    (0x65, "1/50"),
    (0x68, "1/60"),
    (0x6b, "1/80"),
    (0x6c, "1/90"),
    (0x6d, "1/100"),
    (0x70, "1/125"),
    (0x73, "1/160"),
    (0x74, "1/180"),
    (0x75, "1/200"),
    (0x78, "1/250"),
    (0x7b, "1/320"),
    (0x7c, "1/350"),
    (0x7d, "1/400"),
    (0x80, "1/500"),
    (0x83, "1/640"),
    (0x84, "1/750"),
    (0x85, "1/800"),
    (0x88, "1/1000"),
    (0x8b, "1/1250"),
    (0x8c, "1/1500"),
    (0x8d, "1/1600"),
    (0x90, "1/2000"),
    (0x93, "1/2500"),
    (0x94, "1/3000"),
    (0x95, "1/3200"),
    (0x98, "1/4000"),
    (0x9b, "1/5000"),
    (0x9c, "1/6000"),
    (0x9d, "1/6400"),
    (0xa0, "1/8000"),
    (0xa3, "1/10000"),
    (0xa5, "1/12800"),
    (0xa8, "1/16000"),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iso_codes_read_as_the_camera_shows_them() {
        assert_eq!(iso(0x48), "100");
        assert_eq!(iso(0x70), "3200");
        assert_eq!(iso(0x78), "6400");
        assert_eq!(iso(0x7b), "8000");
        assert_eq!(iso(0x98), "102400");
        assert_eq!(iso(0x00), "auto");
        assert_eq!(iso(0x1234), "0x1234");
    }

    #[test]
    fn aperture_codes_read_as_ccapi_writes_them() {
        assert_eq!(aperture(0x20), "f2.8");
        assert_eq!(aperture(0x28), "f4.0");
        assert_eq!(aperture(0x08), "f1.0");
        assert_eq!(aperture(0x3b), "f9.0");
        assert_eq!(aperture(0x3c), "f9.5");
        assert_eq!(aperture(0x3d), "f10");
        assert_eq!(aperture(0x50), "f22");
        assert_eq!(aperture(0xffff), "auto");
        assert_eq!(aperture(0x01), "0x01");
    }

    #[test]
    fn shutter_speed_codes_read_as_the_camera_shows_them() {
        assert_eq!(shutter_speed(0x10), "30\"");
        assert_eq!(shutter_speed(0x14), "20\"");
        assert_eq!(shutter_speed(0x3b), "0\"8");
        assert_eq!(shutter_speed(0x70), "1/125");
        assert_eq!(shutter_speed(0xa0), "1/8000");
        assert_eq!(shutter_speed(0xa8), "1/16000");
        assert_eq!(shutter_speed(0x04), "bulb");
        assert_eq!(shutter_speed(0x00), "auto");
        assert_eq!(shutter_speed(0x01), "0x01");
    }
}
