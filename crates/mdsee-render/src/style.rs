//! SGR sequence組み立てと色の降格（design.md §19, §35）。
//!
//! 実際の色は `Theme`（§61）が持つ。ここではANSIへの変換のみを担当する。

use mdsee_terminal::ColorLevel;

/// RGB色。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rgb(pub u8, pub u8, pub u8);

/// 表示属性。§61の `TextStyle` と同型。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct StyleSpec {
    pub fg: Option<Rgb>,
    pub bold: bool,
    pub italic: bool,
    pub strike: bool,
    pub underline: bool,
}

/// SGR（Select Graphic Rendition）開始sequenceを組み立てる。
///
/// `ColorLevel::None` の場合は色・装飾とも出力しない（§71 plain rendering）。
pub(crate) fn sgr_sequence(spec: &StyleSpec, level: ColorLevel) -> String {
    if level == ColorLevel::None {
        return String::new();
    }

    let mut params: Vec<String> = Vec::new();
    if spec.bold {
        params.push("1".to_string());
    }
    if spec.italic {
        params.push("3".to_string());
    }
    if spec.underline {
        params.push("4".to_string());
    }
    if spec.strike {
        params.push("9".to_string());
    }
    if let Some(rgb) = spec.fg {
        match level {
            ColorLevel::TrueColor => {
                params.push(format!("38;2;{};{};{}", rgb.0, rgb.1, rgb.2));
            }
            ColorLevel::Ansi256 => {
                params.push(format!("38;5;{}", rgb_to_256(rgb)));
            }
            ColorLevel::Ansi16 => {
                if let Some(code) = fg_16(rgb) {
                    params.push(code.to_string());
                }
            }
            ColorLevel::None => {}
        }
    }
    if params.is_empty() {
        String::new()
    } else {
        format!("\x1b[{}m", params.join(";"))
    }
}

/// TrueColor → xterm 256色への降格。
pub(crate) fn rgb_to_256(rgb: Rgb) -> u8 {
    let cube_levels = [0u8, 95, 135, 175, 215, 255];
    let nearest = |value: u8| {
        cube_levels
            .iter()
            .enumerate()
            .min_by_key(|(_, level)| level.abs_diff(value))
            .map(|(index, _)| index)
            .unwrap_or(0)
    };
    let (ri, gi, bi) = (nearest(rgb.0), nearest(rgb.1), nearest(rgb.2));
    let cube_index = 16 + 36 * ri + 6 * gi + bi;
    let cube_rgb = Rgb(cube_levels[ri], cube_levels[gi], cube_levels[bi]);

    let gray = (u16::from(rgb.0) + u16::from(rgb.1) + u16::from(rgb.2)) / 3;
    let (gray_index, gray_value) = if gray < 8 {
        (16u16, 0u8)
    } else if gray > 248 {
        (231u16, 255u8)
    } else {
        let index = 232 + (gray - 8) / 10;
        let value = (8 + ((gray - 8) / 10) * 10) as u8;
        (index, value)
    };

    let cube_distance = distance(rgb, cube_rgb);
    let gray_rgb = Rgb(gray_value, gray_value, gray_value);
    let gray_distance = distance(rgb, gray_rgb);
    if cube_distance <= gray_distance {
        cube_index as u8
    } else {
        gray_index as u8
    }
}

/// 256色 → 16色への降格。SGR前景color code（30〜37 / 90〜97）を返す。
pub(crate) fn fg_16(rgb: Rgb) -> Option<u8> {
    const PALETTE: [Rgb; 16] = [
        Rgb(0, 0, 0),
        Rgb(205, 49, 49),
        Rgb(13, 188, 121),
        Rgb(190, 180, 81),
        Rgb(36, 114, 200),
        Rgb(188, 82, 188),
        Rgb(17, 168, 205),
        Rgb(229, 229, 234),
        Rgb(102, 102, 102),
        Rgb(241, 76, 76),
        Rgb(35, 209, 139),
        Rgb(245, 208, 101),
        Rgb(59, 142, 234),
        Rgb(237, 112, 237),
        Rgb(41, 184, 219),
        Rgb(255, 255, 255),
    ];

    let index = PALETTE
        .iter()
        .enumerate()
        .min_by_key(|(_, candidate)| distance(rgb, **candidate))
        .map(|(index, _)| index)?;
    Some(if index < 8 {
        30 + index as u8
    } else {
        82 + index as u8 // 90 + (index - 8)
    })
}

fn distance(a: Rgb, b: Rgb) -> u32 {
    u32::from(a.0.abs_diff(b.0)) + u32::from(a.1.abs_diff(b.1)) + u32::from(a.2.abs_diff(b.2))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rgb_to_256_known_values() {
        assert_eq!(rgb_to_256(Rgb(0, 0, 0)), 16);
        assert_eq!(rgb_to_256(Rgb(255, 255, 255)), 231);
        assert_eq!(rgb_to_256(Rgb(255, 0, 0)), 196);
        assert_eq!(rgb_to_256(Rgb(0, 255, 0)), 46);
        assert_eq!(rgb_to_256(Rgb(0, 0, 255)), 21);
        // 灰色はgrayscale帯へ
        assert_eq!(rgb_to_256(Rgb(128, 128, 128)), 244);
    }

    #[test]
    fn fg_16_known_values() {
        assert_eq!(fg_16(Rgb(255, 0, 0)), Some(31)); // red（距離最近傍）
        assert_eq!(fg_16(Rgb(13, 188, 121)), Some(32)); // green
        assert_eq!(fg_16(Rgb(255, 255, 255)), Some(97)); // bright white
        assert_eq!(fg_16(Rgb(0, 0, 0)), Some(30)); // black
    }

    #[test]
    fn truecolor_uses_38_2() {
        let spec = StyleSpec {
            fg: Some(Rgb(10, 20, 30)),
            bold: false,
            italic: false,
            strike: false,
            underline: false,
        };
        let seq = sgr_sequence(&spec, ColorLevel::TrueColor);
        assert_eq!(seq, "\x1b[38;2;10;20;30m");
    }

    #[test]
    fn ansi256_uses_38_5() {
        let spec = StyleSpec {
            fg: Some(Rgb(255, 0, 0)),
            bold: false,
            italic: false,
            strike: false,
            underline: false,
        };
        let seq = sgr_sequence(&spec, ColorLevel::Ansi256);
        assert_eq!(seq, "\x1b[38;5;196m");
    }

    #[test]
    fn ansi16_uses_basic_code() {
        let spec = StyleSpec {
            fg: Some(Rgb(255, 0, 0)),
            bold: false,
            italic: false,
            strike: false,
            underline: false,
        };
        let seq = sgr_sequence(&spec, ColorLevel::Ansi16);
        assert_eq!(seq, "\x1b[31m");
    }

    #[test]
    fn decorations_are_combined() {
        let spec = StyleSpec {
            fg: Some(Rgb(1, 2, 3)),
            bold: true,
            italic: true,
            strike: true,
            underline: true,
        };
        let seq = sgr_sequence(&spec, ColorLevel::TrueColor);
        assert_eq!(seq, "\x1b[1;3;4;9;38;2;1;2;3m");
    }

    #[test]
    fn plain_level_emits_nothing() {
        let spec = StyleSpec {
            fg: Some(Rgb(1, 2, 3)),
            bold: true,
            italic: false,
            strike: false,
            underline: false,
        };
        assert_eq!(sgr_sequence(&spec, ColorLevel::None), "");
    }
}
