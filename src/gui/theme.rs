// gui/theme.rs
// color palette definitions and how they get applied to egui's visual system
// dark theme is tokyo-night inspired; light is a warm paper tone
// also owns the global style pass: spacing, rounding, shadows, type scale

use egui::epaint::Shadow;
use egui::{Color32, Context, FontFamily, FontId, Margin, Rounding, Stroke, TextStyle, Visuals};

pub struct Palette {
    pub bg: Color32,
    pub bg_secondary: Color32,
    pub bg_highlight: Color32,
    pub text: Color32,
    pub text_dim: Color32,
    pub accent: Color32,
    pub danger: Color32,
    pub success: Color32,
    pub border: Color32,
}

impl Palette {
    pub fn dark(accent: Color32) -> Self {
        Self {
            // deep indigo-charcoal ramp instead of flat grey
            bg: Color32::from_rgb(16, 17, 27),
            bg_secondary: Color32::from_rgb(24, 26, 38),
            bg_highlight: Color32::from_rgb(41, 46, 66),
            text: Color32::from_rgb(202, 211, 245),
            text_dim: Color32::from_rgb(110, 118, 148),
            accent,
            danger: Color32::from_rgb(237, 105, 122),
            success: Color32::from_rgb(120, 210, 145),
            border: Color32::from_rgb(45, 50, 72),
        }
    }

    pub fn light(accent: Color32) -> Self {
        Self {
            bg: Color32::from_rgb(240, 240, 246),
            bg_secondary: Color32::from_rgb(228, 229, 238),
            bg_highlight: Color32::from_rgb(205, 210, 232),
            text: Color32::from_rgb(32, 34, 48),
            text_dim: Color32::from_rgb(120, 124, 148),
            accent,
            danger: Color32::from_rgb(196, 62, 82),
            success: Color32::from_rgb(52, 148, 88),
            border: Color32::from_rgb(198, 200, 216),
        }
    }

    // accent tinted toward the background - used for subtle fills
    pub fn accent_soft(&self) -> Color32 {
        lerp_color(self.bg_secondary, self.accent, 0.18)
    }
}

// linear blend between two colors, t in 0..=1
pub fn lerp_color(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let l = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color32::from_rgba_unmultiplied(
        l(a.r(), b.r()),
        l(a.g(), b.g()),
        l(a.b(), b.b()),
        l(a.a(), b.a()),
    )
}

// takes a palette and applies all the colors to egui's visuals struct
pub fn build_visuals(palette: &Palette) -> Visuals {
    let mut v = Visuals::dark();

    v.panel_fill = palette.bg;
    v.window_fill = palette.bg;
    v.window_stroke = Stroke::new(1.0, palette.border);
    v.window_rounding = Rounding::same(14.0);
    // soft drop shadow so the overlay floats above whatever is behind it
    v.window_shadow = Shadow {
        offset: egui::vec2(0.0, 6.0),
        blur: 28.0,
        spread: 0.0,
        color: Color32::from_black_alpha(96),
    };
    v.popup_shadow = Shadow {
        offset: egui::vec2(0.0, 4.0),
        blur: 16.0,
        spread: 0.0,
        color: Color32::from_black_alpha(80),
    };
    v.menu_rounding = Rounding::same(10.0);

    v.widgets.noninteractive.bg_fill = palette.bg_secondary;
    v.widgets.noninteractive.fg_stroke = Stroke::new(1.0, palette.text_dim);
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, palette.border);
    v.widgets.noninteractive.rounding = Rounding::same(8.0);

    v.widgets.inactive.bg_fill = palette.bg_secondary;
    v.widgets.inactive.weak_bg_fill = palette.bg_secondary;
    v.widgets.inactive.fg_stroke = Stroke::new(1.0, palette.text);
    v.widgets.inactive.rounding = Rounding::same(8.0);

    v.widgets.hovered.bg_fill = palette.bg_highlight;
    v.widgets.hovered.weak_bg_fill = palette.bg_highlight;
    v.widgets.hovered.fg_stroke = Stroke::new(1.5, palette.text);
    v.widgets.hovered.bg_stroke = Stroke::new(1.0, palette.accent.linear_multiply(0.6));
    v.widgets.hovered.rounding = Rounding::same(8.0);

    v.widgets.active.bg_fill = palette.accent;
    v.widgets.active.weak_bg_fill = palette.accent;
    v.widgets.active.fg_stroke = Stroke::new(1.5, palette.bg);
    v.widgets.active.rounding = Rounding::same(8.0);

    v.widgets.open.bg_fill = palette.bg_highlight;
    v.widgets.open.rounding = Rounding::same(8.0);

    v.selection.bg_fill = palette.accent.linear_multiply(0.35);
    v.selection.stroke = Stroke::new(1.0, palette.accent);

    v.override_text_color = Some(palette.text);

    v
}

// global style pass: breathing room, type scale, snappy scrolling
// call once at startup and again whenever the theme changes
pub fn apply_style(ctx: &Context) {
    let mut style = (*ctx.style()).clone();

    style.spacing.item_spacing = egui::vec2(8.0, 6.0);
    style.spacing.button_padding = egui::vec2(12.0, 6.0);
    style.spacing.menu_margin = Margin::same(8.0);
    style.spacing.window_margin = Margin::same(14.0);
    style.spacing.scroll.bar_width = 6.0;
    style.spacing.scroll.floating = true;

    style.text_styles = [
        (
            TextStyle::Heading,
            FontId::new(19.0, FontFamily::Proportional),
        ),
        (TextStyle::Body, FontId::new(14.5, FontFamily::Proportional)),
        (
            TextStyle::Button,
            FontId::new(14.0, FontFamily::Proportional),
        ),
        (
            TextStyle::Small,
            FontId::new(11.5, FontFamily::Proportional),
        ),
        (
            TextStyle::Monospace,
            FontId::new(13.0, FontFamily::Monospace),
        ),
    ]
    .into();

    // animations feel snappy, not sluggish
    style.animation_time = 0.12;

    ctx.set_style(style);
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
    Color32::from_rgb(0x4f, 0x8e, 0xf7) // fall back to the default blue if parsing fails
}

// checks if the os is in dark mode - for now we just always say yes
// doing this properly on macos needs objc ffi which is a whole thing
#[allow(dead_code)]
pub fn system_is_dark() -> bool {
    #[cfg(target_os = "macos")]
    {
        true // TODO: actually check NSApp.effectiveAppearance someday
    }
    #[cfg(not(target_os = "macos"))]
    {
        true
    }
}
