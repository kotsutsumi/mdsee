//! Theme（design.md §61, §62）。
//!
//! `SemanticStyle`（§19）を実際の表示属性へ変換する。
//! Sprint 2では組込みの dark / light のみ。

use crate::style::{Rgb, StyleSpec};

/// テキストの表示属性。§61の `TextStyle`。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextStyle {
    pub fg: Option<Rgb>,
    pub bold: bool,
    pub italic: bool,
    pub strike: bool,
    pub underline: bool,
}

impl TextStyle {
    pub const DEFAULT: Self = Self {
        fg: None,
        bold: false,
        italic: false,
        strike: false,
        underline: false,
    };

    pub const fn fg(fg: Rgb) -> Self {
        Self {
            fg: Some(fg),
            ..Self::DEFAULT
        }
    }

    pub const fn bold(mut self) -> Self {
        self.bold = true;
        self
    }

    pub const fn italic(mut self) -> Self {
        self.italic = true;
        self
    }

    pub const fn underline(mut self) -> Self {
        self.underline = true;
        self
    }

    fn to_spec(self) -> StyleSpec {
        StyleSpec {
            fg: self.fg,
            bold: self.bold,
            italic: self.italic,
            strike: self.strike,
            underline: self.underline,
        }
    }
}

/// Alert種別ごとのstyle（§61）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AlertTheme {
    pub note: TextStyle,
    pub tip: TextStyle,
    pub important: TextStyle,
    pub warning: TextStyle,
    pub caution: TextStyle,
}

/// Theme（§61）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Theme {
    pub body: TextStyle,
    pub muted: TextStyle,

    pub h1: TextStyle,
    pub h2: TextStyle,
    pub h3: TextStyle,

    pub link: TextStyle,

    pub inline_code: TextStyle,

    pub quote: TextStyle,

    pub border: TextStyle,

    pub alerts: AlertTheme,

    pub syntax_theme: String,
}

impl Theme {
    /// 組込みdark theme（§62）。
    pub fn dark() -> Self {
        Self {
            body: TextStyle::DEFAULT,
            muted: TextStyle::fg(Rgb(139, 148, 158)),
            h1: TextStyle::fg(Rgb(63, 185, 80)).bold(),
            h2: TextStyle::fg(Rgb(88, 166, 255)).bold(),
            h3: TextStyle::fg(Rgb(233, 208, 115)).bold(),
            link: TextStyle::fg(Rgb(88, 166, 255)).underline(),
            inline_code: TextStyle::fg(Rgb(79, 193, 233)),
            quote: TextStyle::fg(Rgb(139, 148, 158)),
            border: TextStyle::fg(Rgb(48, 54, 61)),
            alerts: AlertTheme {
                note: TextStyle::fg(Rgb(88, 166, 255)),
                tip: TextStyle::fg(Rgb(63, 185, 80)),
                important: TextStyle::fg(Rgb(163, 113, 247)),
                warning: TextStyle::fg(Rgb(219, 171, 121)),
                caution: TextStyle::fg(Rgb(248, 81, 73)),
            },
            syntax_theme: "base16-ocean.dark".to_string(),
        }
    }

    /// 組込みlight theme（§62）。
    pub fn light() -> Self {
        Self {
            body: TextStyle::DEFAULT,
            muted: TextStyle::fg(Rgb(101, 109, 118)),
            h1: TextStyle::fg(Rgb(31, 122, 46)).bold(),
            h2: TextStyle::fg(Rgb(9, 76, 178)).bold(),
            h3: TextStyle::fg(Rgb(148, 108, 8)).bold(),
            link: TextStyle::fg(Rgb(9, 76, 178)).underline(),
            inline_code: TextStyle::fg(Rgb(6, 122, 174)),
            quote: TextStyle::fg(Rgb(101, 109, 118)),
            border: TextStyle::fg(Rgb(208, 211, 215)),
            alerts: AlertTheme {
                note: TextStyle::fg(Rgb(9, 76, 178)),
                tip: TextStyle::fg(Rgb(31, 122, 46)),
                important: TextStyle::fg(Rgb(111, 66, 193)),
                warning: TextStyle::fg(Rgb(154, 103, 29)),
                caution: TextStyle::fg(Rgb(191, 38, 34)),
            },
            syntax_theme: "InspiredGitHub".to_string(),
        }
    }

    /// 組込みtheme名（§62）。auto は `select_auto` で解決する。
    pub fn builtin(name: &str) -> Option<Self> {
        match name {
            "dark" => Some(Self::dark()),
            "light" => Some(Self::light()),
            _ => None,
        }
    }

    /// §19のSemanticStyleを表示属性へ変換する。
    pub(crate) fn spec(&self, style: mdsee_layout::SemanticStyle) -> StyleSpec {
        use mdsee_layout::SemanticStyle;
        let text_style = match style {
            SemanticStyle::Body => self.body,
            SemanticStyle::Muted => self.muted,
            SemanticStyle::Heading1 => self.h1,
            SemanticStyle::Heading2 => self.h2,
            SemanticStyle::Heading3 => self.h3,
            SemanticStyle::Heading4 => self.h1,
            SemanticStyle::Heading5 => self.h2,
            SemanticStyle::Heading6 => self.muted,
            SemanticStyle::Strong => TextStyle::DEFAULT.bold(),
            SemanticStyle::Emphasis => TextStyle::DEFAULT.italic(),
            SemanticStyle::Strike => TextStyle::DEFAULT,
            SemanticStyle::InlineCode => self.inline_code,
            SemanticStyle::Link => self.link,
            SemanticStyle::Quote => self.quote,
            SemanticStyle::Code => self.quote,
            SemanticStyle::Border => self.border,
            SemanticStyle::AlertNote => self.alerts.note,
            SemanticStyle::AlertTip => self.alerts.tip,
            SemanticStyle::AlertImportant => self.alerts.important,
            SemanticStyle::AlertWarning => self.alerts.warning,
            SemanticStyle::AlertCaution => self.alerts.caution,
        };
        text_style.to_spec()
    }
}

impl Default for Theme {
    fn default() -> Self {
        Self::dark()
    }
}

/// `auto` themeの解決（§62）。
///
/// 環境変数 `COLORFGBG`（`15;0` 形式）の背景色が暗い番号（0〜6, 8）なら
/// dark theme、明るい番号（7, 9〜15）なら light theme。
/// 判定できない場合はdark。
pub fn select_auto_theme(colorfgbg: Option<&str>) -> Theme {
    let background_is_light = colorfgbg.and_then(|value| {
        value
            .rsplit(';')
            .next()
            .and_then(|last| last.parse::<u8>().ok())
            .map(|bg| bg == 7 || (9..=15).contains(&bg))
    });
    match background_is_light {
        Some(true) => Theme::light(),
        _ => Theme::dark(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_themes_are_resolvable() {
        assert!(Theme::builtin("dark").is_some());
        assert!(Theme::builtin("light").is_some());
        assert!(Theme::builtin("solarized").is_none());
    }

    #[test]
    fn dark_and_light_differ() {
        assert_ne!(Theme::dark(), Theme::light());
    }

    #[test]
    fn auto_theme_uses_colorfgbg_background() {
        assert_eq!(select_auto_theme(Some("15;0")), Theme::dark());
        assert_eq!(select_auto_theme(Some("0;15")), Theme::light());
        assert_eq!(select_auto_theme(None), Theme::dark());
        assert_eq!(select_auto_theme(Some("garbage")), Theme::dark());
    }

    #[test]
    fn spec_maps_heading_and_link() {
        let theme = Theme::dark();
        let h1 = theme.spec(mdsee_layout::SemanticStyle::Heading1);
        assert!(h1.bold);
        assert_eq!(h1.fg, Some(Rgb(63, 185, 80)));

        let link = theme.spec(mdsee_layout::SemanticStyle::Link);
        assert!(link.underline);
    }
}
