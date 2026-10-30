use num_traits::ToPrimitive;

pub(crate) const COOLEST: f64 = 6500.0;
pub(crate) const WARMEST: f64 = 1200.0;

/// Approximate the sRGB color of a blackbody at `kelvin` with Tanner Helland's
/// curve fit.
pub(crate) fn rgb(kelvin: f64) -> [u8; 3] {
    let t = kelvin / 100.0;
    let red = if t <= 66.0 {
        255.0
    } else {
        329.698_727_446 * (t - 60.0).powf(-0.133_204_759_2)
    };
    let green = if t <= 66.0 {
        99.470_802_586_1 * t.ln() - 161.119_568_166_1
    } else {
        288.122_169_528_3 * (t - 60.0).powf(-0.075_514_849_2)
    };
    let blue = if t >= 66.0 {
        255.0
    } else if t <= 19.0 {
        0.0
    } else {
        138.517_731_223_1 * (t - 10.0).ln() - 305.044_792_730_7
    };
    [channel(red), channel(green), channel(blue)]
}

fn channel(value: f64) -> u8 {
    value.round().clamp(0.0, 255.0).to_u8().unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use test_case::test_case;

    #[test_case(1200.0, [0xFF, 0x56, 0x00])]
    #[test_case(1900.0, [0xFF, 0x84, 0x00])]
    #[test_case(2400.0, [0xFF, 0x9B, 0x3D])]
    #[test_case(3000.0, [0xFF, 0xB1, 0x6E])]
    #[test_case(3400.0, [0xFF, 0xBE, 0x87])]
    #[test_case(4500.0, [0xFF, 0xDA, 0xBB])]
    #[test_case(5500.0, [0xFF, 0xED, 0xDE])]
    #[test_case(6500.0, [0xFF, 0xFE, 0xFA])]
    fn matches_the_design_reference_colors(kelvin: f64, expected: [u8; 3]) {
        assert_eq!(rgb(kelvin), expected);
    }
}
