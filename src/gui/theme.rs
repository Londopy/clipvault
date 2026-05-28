// gui/theme.rs
// color palette definitions and how they get applied to egui's visual system
// dark and light theme, plus a hex color parser for the accent color

use egui::{Color32, Rounding, Stroke, Visuals};

pub struct Palette {
    pub bg: Color32,
    pub bg_secondary: Color32,
    pub bg_highlight: Color32,
    pub text: Color32,
    pub text_dim: Color32,
    pub accent: Color32,
    pub danger: Color32,
    pub border: Color32,
}

impl Palette {
    pub fn dark(accent: Color32) -> Self {
        Self {
            bg: Color32::from_rgb(20, 20, 24),
            bg_secondary: Color32::from_rgb(30, 30, 36),
            bg_highlight: Color32::from_rgb(45, 45, 55),
            text: Color32::from_rgb(220, 220, 220),
            text_dim: Color32::from_rgb(140, 140, 150),
            accent,
            danger: Color32::from_rgb(220, 80, 80),
            border: Color32::from_rgb(55, 55, 65),
        }
    }

    pub fn light(accent: Color32) -> Self {
        Self {
            bg: Color32::from_rgb(245, 245, 248),
            bg_secondary: Color32::from_rgb(232, 232, 238),
            bg_highlight: Color32::from_rgb(210, 215, 230),
            text: Color32::from_rgb(25, 25, 30),
            text_dim: Color32::from_rgb(110, 110, 120),
            accent,
            danger: Color32::from_rgb(200, 60, 60),
            border: Color32::from_rgb(200, 200, 210),
        }
    }
}

// takes a palette and applies all the colors to egui's visuals struct
pub fn build_visuals(palette: &Palette) -> Visuals {
    let mut v = Visuals::dark();

    v.panel_fill = palette.bg;
    v.window_fill = palette.bg_secondary;
    v.window_stroke = Stroke::new(1.0, palette.border);
    v.window_rounding = Rounding::same(8.0);

    v.widgets.noninteractive.bg_fill = palette.bg_secondary;
    v.widgets.noninteractive.fg_stroke = Stroke::new(1.0, palette.text_dim);
    v.widgets.inactive.bg_fill = palette.bg_secondary;
    v.widgets.inactive.fg_stroke = Stroke::new(1.0, palette.text);
    v.widgets.hovered.bg_fill = palette.bg_highlight;
    v.widgets.hovered.fg_stroke = Stroke::new(1.5, palette.accent);
    v.widgets.active.bg_fill = palette.accent;
    v.widgets.active.fg_stroke = Stroke::new(1.5, palette.bg);
    v.widgets.open.bg_fill = palette.bg_highlight;

    v.selection.bg_fill = palette.accent.linear_multiply(0.4);
    v.selection.stroke = Stroke::new(1.0, palette.accent);

    v.override_text_color = Some(palette.text);

    v
}

// parses "#4f8ef7" or "4f8ef7" into an egui Color32
pub fn parse_hex_color(s: &str) -> Color32 {
    let s = s.trim_start_matches('#');
    if s.len() == 6 {
        if let (Ok(r), Ok(g), Ok(b)) = (
            u8::from_str_radix(&s[0..2], 16),
            u8::from_str_radix(&s[2..4], 16),
            u8::from_str_radix(&s[4..6], 16),
        ) {
            return Color32::from_rgb(r, g, b);
        }
    }
    Color32::from_rgb(0x4f, 0x8e, 0xf7)  // fall back to the default blue if parsing fails
}

// checks if the os is in dark mode - for now we just always say yes
// doing this properly on macos needs objc ffi which is a whole thing
#[allow(dead_code)]
pub fn system_is_dark() -> bool {
    #[cfg(target_os = "macos")]
    {
        true // TODO: actually check NSApp.effectiveAppearance someday
    }