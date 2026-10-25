//! Desktop appearance settings the UI follows.

use std::fmt;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ColorScheme {
    Light,
    Dark,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ControlSide {
    Left,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ControlButton {
    Close,
    Minimize,
    Maximize,
}

/// Where the window buttons go and which ones, in on-screen order (left to right).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WindowControls {
    pub side: ControlSide,
    pub buttons: Vec<ControlButton>,
}

impl Default for WindowControls {
    /// GNOME's stock layout, and our fallback when the setting is unknown.
    fn default() -> Self {
        Self {
            side: ControlSide::Right,
            buttons: vec![ControlButton::Close],
        }
    }
}

/// An sRGB colour, e.g. the portal accent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Rgb {
    /// Convert portal floats (0.0..=1.0 per channel) to bytes, clamping bad values.
    pub fn from_unit(r: f64, g: f64, b: f64) -> Rgb {
        let ch = |v: f64| {
            if v.is_nan() {
                0
            } else {
                (v.clamp(0.0, 1.0) * 255.0).round() as u8
            }
        };
        Rgb {
            r: ch(r),
            g: ch(g),
            b: ch(b),
        }
    }

    /// `#RRGGBB`, upper-case.
    pub fn to_hex(self) -> String {
        format!("#{:02X}{:02X}{:02X}", self.r, self.g, self.b)
    }

    pub fn parse_hex(s: &str) -> Option<Rgb> {
        let h = s.strip_prefix('#')?;
        if h.len() != 6 || !h.bytes().all(|b| b.is_ascii_hexdigit()) {
            return None;
        }
        let n = u32::from_str_radix(h, 16).ok()?;
        Some(Rgb {
            r: (n >> 16) as u8,
            g: (n >> 8) as u8,
            b: n as u8,
        })
    }
}

impl fmt::Display for Rgb {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_hex())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Appearance {
    pub color_scheme: ColorScheme,
    /// `None` means the default blue.
    pub accent: Option<Rgb>,
    pub high_contrast: bool,
    pub reduced_motion: bool,
    pub controls: WindowControls,
}

impl Default for Appearance {
    fn default() -> Self {
        Self {
            color_scheme: ColorScheme::Light,
            accent: None,
            high_contrast: false,
            reduced_motion: false,
            controls: WindowControls::default(),
        }
    }
}

/// Parsing GNOME's `button-layout` (also GTK's `gtk-decoration-layout`).
pub mod button_layout {
    use super::{ControlButton, ControlSide, WindowControls};

    /// Parse a layout string such as `appmenu:minimize,maximize,close`.
    ///
    /// Text before the first `:` is the left side, after it the right side. With no
    /// colon everything is on the left (GTK and mutter behave this way). Unknown
    /// items (`appmenu`, `icon`, `spacer`, anything else) are ignored.
    ///
    /// We draw controls on one side only (ADR-0006). If both sides hold window
    /// buttons, the side with `close` wins, then the side with more buttons, then
    /// the right. A layout with no window buttons at all yields the default
    /// (`right: [close]`) so the window can always be closed.
    pub fn parse(layout: &str) -> WindowControls {
        try_parse(layout).unwrap_or_default()
    }

    /// Like [`parse`], but `None` when the layout has no window buttons.
    pub fn try_parse(layout: &str) -> Option<WindowControls> {
        let (left_raw, right_raw) = layout.split_once(':').unwrap_or((layout, ""));
        let left = buttons(left_raw);
        let right = buttons(right_raw);
        let has_close = |b: &[ControlButton]| b.contains(&ControlButton::Close);

        let side = match (left.is_empty(), right.is_empty()) {
            (true, true) => return None,
            (false, true) => ControlSide::Left,
            (true, false) => ControlSide::Right,
            (false, false) => match (has_close(&left), has_close(&right)) {
                (true, false) => ControlSide::Left,
                (false, true) => ControlSide::Right,
                _ if left.len() > right.len() => ControlSide::Left,
                _ => ControlSide::Right,
            },
        };
        let buttons = if side == ControlSide::Left {
            left
        } else {
            right
        };
        Some(WindowControls { side, buttons })
    }

    fn buttons(part: &str) -> Vec<ControlButton> {
        let mut out = Vec::new();
        for item in part.split(',') {
            let b = match item.trim() {
                "close" => ControlButton::Close,
                "minimize" => ControlButton::Minimize,
                "maximize" => ControlButton::Maximize,
                _ => continue,
            };
            if !out.contains(&b) {
                out.push(b);
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::ControlButton::{Close, Maximize, Minimize};
    use super::ControlSide::{Left, Right};
    use super::*;

    fn wc(side: ControlSide, buttons: &[ControlButton]) -> WindowControls {
        WindowControls {
            side,
            buttons: buttons.to_vec(),
        }
    }

    #[test]
    fn button_layout_table() {
        let rows = [
            ("appmenu:close", wc(Right, &[Close])),
            (":close", wc(Right, &[Close])),
            (
                "appmenu:minimize,maximize,close",
                wc(Right, &[Minimize, Maximize, Close]),
            ),
            (
                "close,minimize,maximize:",
                wc(Left, &[Close, Minimize, Maximize]),
            ),
            (
                "close,minimize,maximize:appmenu",
                wc(Left, &[Close, Minimize, Maximize]),
            ),
            ("close:appmenu", wc(Left, &[Close])),
            ("close", wc(Left, &[Close])),
            (
                "icon:minimize,maximize,close",
                wc(Right, &[Minimize, Maximize, Close]),
            ),
            (
                "  close , minimize : appmenu ",
                wc(Left, &[Close, Minimize]),
            ),
            ("appmenu:close,close,close", wc(Right, &[Close])),
            ("spacer,close:appmenu,spacer", wc(Left, &[Close])),
            // both sides: close side wins
            ("minimize:close", wc(Right, &[Close])),
            ("close:minimize,maximize", wc(Left, &[Close])),
            // both sides, close on neither: more buttons wins, tie goes right
            (
                "minimize,maximize:maximize",
                wc(Left, &[Minimize, Maximize]),
            ),
            ("minimize:maximize", wc(Right, &[Maximize])),
            // both have close: more buttons, else right
            ("close:close", wc(Right, &[Close])),
            ("close,minimize:close", wc(Left, &[Close, Minimize])),
            // nothing usable falls back to default
            ("", WindowControls::default()),
            (":", WindowControls::default()),
            ("appmenu:", WindowControls::default()),
            ("appmenu:spacer", WindowControls::default()),
            ("garbage", WindowControls::default()),
            ("CLOSE", WindowControls::default()),
        ];
        for (input, want) in rows {
            assert_eq!(button_layout::parse(input), want, "{input:?}");
        }
    }

    #[test]
    fn try_parse_reports_missing_buttons() {
        assert_eq!(button_layout::try_parse("appmenu:"), None);
        assert!(button_layout::try_parse("appmenu:close").is_some());
    }

    #[test]
    fn default_is_right_close() {
        assert_eq!(WindowControls::default(), wc(Right, &[Close]));
    }

    #[test]
    fn rgb_hex_round_trip_and_floats() {
        let c = Rgb::from_unit(0.2078, 0.5176, 0.8941);
        assert_eq!(c.to_hex(), "#3584E4");
        assert_eq!(Rgb::parse_hex("#3584e4"), Some(c));
        assert_eq!(
            Rgb::from_unit(-1.0, 2.0, f64::NAN),
            Rgb { r: 0, g: 255, b: 0 }
        );
        for bad in ["3584E4", "#3584E", "#3584E4F", "#GGGGGG", ""] {
            assert_eq!(Rgb::parse_hex(bad), None, "{bad:?}");
        }
    }
}
