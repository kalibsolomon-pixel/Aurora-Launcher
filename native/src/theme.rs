//! Applies the persisted appearance to the native `Theme` global.
//!
//! Surface palettes mirror the three dark themes in `src/app.css` (the
//! approved token set). The accent family is always the Rust appearance
//! service's derived palette, so curated and validated custom accents render
//! identically in both hosts.

use slint::{Color, ComponentHandle};

use crate::api::Appearance;
use crate::{AppWindow, Theme};

struct Surfaces {
    background: u32,
    sidebar: u32,
    surface: u32,
    raised: u32,
    hover: u32,
    pressed: u32,
    sunken: u32,
    border: u32,
    border_strong: u32,
    text: u32,
    text_secondary: u32,
    text_muted: u32,
}

const AURORA_DARK: Surfaces = Surfaces {
    background: 0x101116,
    sidebar: 0x0a0b0e,
    surface: 0x16171d,
    raised: 0x1d1f27,
    hover: 0x242631,
    pressed: 0x1a1c24,
    sunken: 0x0d0e13,
    border: 0x24262f,
    border_strong: 0x343743,
    text: 0xedf1fb,
    text_secondary: 0xa7b0c5,
    text_muted: 0x929db4,
};

const MIDNIGHT: Surfaces = Surfaces {
    background: 0x0d1017,
    sidebar: 0x070910,
    surface: 0x121621,
    raised: 0x191e2b,
    hover: 0x202636,
    pressed: 0x161b26,
    sunken: 0x0a0d14,
    border: 0x1f2531,
    border_strong: 0x2c3444,
    text: 0xe9edf9,
    text_secondary: 0xa3adc4,
    text_muted: 0x919db5,
};

const OLED: Surfaces = Surfaces {
    background: 0x000000,
    sidebar: 0x000000,
    surface: 0x0b0b0e,
    raised: 0x141419,
    hover: 0x1b1b22,
    pressed: 0x101015,
    sunken: 0x000000,
    border: 0x1c1d23,
    border_strong: 0x2b2d35,
    text: 0xf0f3fc,
    text_secondary: 0xa9b2c7,
    text_muted: 0x939db4,
};

fn rgb(value: u32) -> Color {
    Color::from_rgb_u8((value >> 16) as u8, (value >> 8) as u8, value as u8)
}

/// A theme surface over the Borealis layer: the same tone, partly see-through,
/// so the pre-blurred bands read as a soft frosted backdrop.
fn translucent(value: u32, alpha: u8) -> Color {
    Color::from_argb_u8(alpha, (value >> 16) as u8, (value >> 8) as u8, value as u8)
}

/// Whether the persisted background is the stylized Borealis layer.
pub fn is_borealis(appearance: &Appearance) -> bool {
    appearance.background == "borealis"
}

/// Parses `#rrggbb` or the CSS `rgba(r, g, b, a)` strings the appearance DTO
/// uses for the soft/outline accent tokens.
pub fn parse_color(text: &str) -> Option<Color> {
    let text = text.trim();
    if let Some(hex) = text.strip_prefix('#') {
        if hex.len() == 6 {
            return u32::from_str_radix(hex, 16).ok().map(rgb);
        }
        return None;
    }
    let inner = text.strip_prefix("rgba(")?.strip_suffix(')')?;
    let parts: Vec<&str> = inner.split(',').map(str::trim).collect();
    if parts.len() != 4 {
        return None;
    }
    let channel = |value: &str| value.parse::<u8>().ok();
    let alpha = parts[3].parse::<f32>().ok()?.clamp(0.0, 1.0);
    Some(Color::from_argb_u8(
        (alpha * 255.0).round() as u8,
        channel(parts[0])?,
        channel(parts[1])?,
        channel(parts[2])?,
    ))
}

pub fn apply(window: &AppWindow, appearance: &Appearance) {
    let theme = window.global::<Theme>();
    let surfaces = match appearance.theme.as_str() {
        "midnight" => &MIDNIGHT,
        "oled" => &OLED,
        _ => &AURORA_DARK,
    };
    let borealis = is_borealis(appearance);
    theme.set_borealis(borealis);
    theme.set_background(rgb(surfaces.background));
    // Content panels and secondary surfaces show the bands softly; raised
    // layers (dialogs, menus) stay nearly opaque so text never sits on a
    // bright area.
    let (sidebar, surface, hover, sunken) = if borealis {
        (
            translucent(surfaces.sidebar, 0x8c),
            translucent(surfaces.surface, 0xb4),
            translucent(surfaces.hover, 0xd0),
            translucent(surfaces.sunken, 0xc8),
        )
    } else {
        (
            rgb(surfaces.sidebar),
            rgb(surfaces.surface),
            rgb(surfaces.hover),
            rgb(surfaces.sunken),
        )
    };
    theme.set_sidebar(sidebar);
    theme.set_surface(surface);
    theme.set_raised(if borealis {
        translucent(surfaces.raised, 0xf2)
    } else {
        rgb(surfaces.raised)
    });
    theme.set_hover(hover);
    theme.set_pressed(rgb(surfaces.pressed));
    theme.set_sunken(sunken);
    theme.set_border(rgb(surfaces.border));
    theme.set_border_strong(rgb(surfaces.border_strong));
    theme.set_text(rgb(surfaces.text));
    theme.set_text_secondary(rgb(surfaces.text_secondary));
    theme.set_text_muted(rgb(surfaces.text_muted));

    apply_palette(window, &appearance.palette);
}

/// Applies only the accent family (used by the live custom-accent preview).
pub fn apply_palette(window: &AppWindow, palette: &crate::api::AccentPalette) {
    let theme = window.global::<Theme>();
    let set = |value: &str, apply: &dyn Fn(Color)| {
        if let Some(color) = parse_color(value) {
            apply(color);
        }
    };
    set(&palette.accent, &|c| theme.set_accent(c));
    set(&palette.accent_strong, &|c| theme.set_accent_strong(c));
    set(&palette.accent_hover, &|c| theme.set_accent_hover(c));
    set(&palette.accent_pressed, &|c| theme.set_accent_pressed(c));
    set(&palette.accent_contrast, &|c| theme.set_accent_contrast(c));
    set(&palette.accent_soft, &|c| theme.set_accent_soft(c));
    set(&palette.accent_outline, &|c| theme.set_accent_outline(c));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_hex_and_css_rgba_tokens() {
        assert_eq!(
            parse_color("#8b80ff"),
            Some(Color::from_rgb_u8(0x8b, 0x80, 0xff))
        );
        assert_eq!(
            parse_color("rgba(139, 128, 255, 0.14)"),
            Some(Color::from_argb_u8(36, 139, 128, 255))
        );
        assert_eq!(parse_color("#12345"), None);
        assert_eq!(parse_color("rgb(1,2,3)"), None);
        assert_eq!(parse_color("rgba(300, 0, 0, 1)"), None);
    }
}
