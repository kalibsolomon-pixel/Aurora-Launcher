//! Aurora presentation tokens, derived from the persisted appearance service.
//!
//! This is deliberately *not* gpui-base's `Theme` (that only carries tokens for
//! the scrollbar/resizable/plot subsystems). The launcher owns its design
//! system: one small struct of colors plus a radius, installed as a GPUI
//! [`Global`] and read by `src/controls.rs`.

use gpui_kit::{App, Global, Hsla, Pixels, px, rgb, rgba};

use crate::borealis;

#[derive(Clone, PartialEq)]
pub struct AuroraTheme {
    pub background: Hsla,
    pub sidebar: Hsla,
    pub surface: Hsla,
    /// Dialogs and menus; nearly opaque over Borealis so text never sits on a
    /// bright band. Not drawn by any view yet.
    #[allow(dead_code)]
    pub raised: Hsla,
    /// Recessed wells (inputs, code blocks); not drawn by any view yet.
    #[allow(dead_code)]
    pub sunken: Hsla,
    pub surface_hover: Hsla,
    /// Content cards and panels: the main reading surface, kept nearly opaque
    /// over Borealis so body text never sits on a bright band.
    pub card: Hsla,
    /// Resting fill of secondary controls (a faint lift over any surface).
    pub control: Hsla,
    pub control_hover: Hsla,
    pub border: Hsla,
    /// Hairline separators and card outlines.
    pub border_subtle: Hsla,
    pub foreground: Hsla,
    pub muted_foreground: Hsla,
    /// Tertiary text: captions, timestamps, metadata.
    pub subtle_foreground: Hsla,
    /// Accent wash for selected rows, tabs and navigation.
    pub accent_soft: Hsla,
    pub accent: Hsla,
    pub accent_hover: Hsla,
    pub accent_foreground: Hsla,
    pub ring: Hsla,
    /// Text-selection highlight inside inputs. `InputEditorStyle::resolved`
    /// would derive this from the accent at 0.4 alpha if left transparent.
    pub selection: Hsla,
    pub caret: Hsla,
    pub danger: Hsla,
    pub danger_hover: Hsla,
    pub success: Hsla,
    pub warning: Hsla,
    /// Controls and inputs.
    pub radius: Pixels,
    /// Cards and panels.
    pub radius_lg: Pixels,
}

impl Global for AuroraTheme {}

impl Default for AuroraTheme {
    fn default() -> Self {
        Self {
            background: rgb(0x101116).into(),
            sidebar: rgb(0x0a0b0e).into(),
            surface: rgb(0x16171d).into(),
            raised: rgb(0x1d1f27).into(),
            sunken: rgb(0x0d0e13).into(),
            surface_hover: rgb(0x242631).into(),
            card: rgb(0x16171d).into(),
            control: rgba(0xffffff14).into(),
            control_hover: rgba(0xffffff22).into(),
            border: rgb(0x343743).into(),
            border_subtle: rgba(0xffffff14).into(),
            foreground: rgb(0xedf1fb).into(),
            muted_foreground: rgb(0xc4cbda).into(),
            subtle_foreground: rgb(0xacb5c8).into(),
            accent_soft: rgba(0x8b80ff26).into(),
            accent: rgb(0x6f5df2).into(),
            accent_hover: rgb(0x6f63e8).into(),
            accent_foreground: rgba(0xFFFFFFFF).into(),
            ring: rgb(0x8b80ff).into(),
            selection: rgba(0x8b80ff59).into(),
            caret: rgb(0xedf1fb).into(),
            danger: rgb(0xff9a9a).into(),
            danger_hover: rgb(0xff9a9a).into(),
            success: rgb(0x5fd39a).into(),
            warning: rgb(0xf2c26b).into(),
            radius: px(8.),
            radius_lg: px(12.),
        }
    }
}

/// The same tone, partly see-through, so the pre-blurred bands read as a soft
/// frosted backdrop (`translucent` in `native/src/theme.rs`).
fn translucent(color: Hsla, alpha: u8) -> Hsla {
    Hsla {
        a: f32::from(alpha) / 255.0,
        ..color
    }
}

fn color(text: &str, fallback: Hsla) -> Hsla {
    text.strip_prefix('#')
        .filter(|hex| hex.len() == 6)
        .and_then(|hex| u32::from_str_radix(hex, 16).ok())
        .map(|value| rgb(value).into())
        .unwrap_or(fallback)
}

/// Match the existing native host's theme surfaces. Accents remain core-owned.
fn resolved(appearance: Option<&crate::api::Appearance>) -> AuroraTheme {
    let mut palette = AuroraTheme::default();
    let Some(appearance) = appearance else {
        return palette;
    };
    let surfaces = match appearance.theme.as_str() {
        "midnight" => Some((
            0x0d1017, 0x070910, 0x121621, 0x191e2b, 0x202636, 0x0a0d14, 0x2c3444, 0xe9edf9,
            0x919db5,
        )),
        "oled" => Some((
            0x000000, 0x000000, 0x0b0b0e, 0x141419, 0x1b1b22, 0x000000, 0x2b2d35, 0xf0f3fc,
            0x939db4,
        )),
        _ => None,
    };
    if let Some((background, sidebar, surface, raised, hover, sunken, border, foreground, muted)) =
        surfaces
    {
        palette.background = rgb(background).into();
        palette.sidebar = rgb(sidebar).into();
        palette.surface = rgb(surface).into();
        palette.raised = rgb(raised).into();
        palette.surface_hover = rgb(hover).into();
        palette.sunken = rgb(sunken).into();
        palette.border = rgb(border).into();
        palette.card = rgb(surface).into();
        palette.foreground = rgb(foreground).into();
        palette.muted_foreground = rgb(muted).into();
    }
    // Content panels and secondary surfaces show the bands softly; raised
    // layers stay nearly opaque. The background and pressed tones stay opaque.
    if borealis::is_borealis(&appearance.background) {
        palette.sidebar = translucent(palette.sidebar, borealis::SIDEBAR_ALPHA);
        palette.surface = translucent(palette.surface, borealis::SURFACE_ALPHA);
        palette.surface_hover = translucent(palette.surface_hover, borealis::HOVER_ALPHA);
        palette.sunken = translucent(palette.sunken, borealis::SUNKEN_ALPHA);
        palette.raised = translucent(palette.raised, borealis::RAISED_ALPHA);
        palette.card = translucent(palette.card, borealis::CARD_ALPHA);
    }
    let accent = &appearance.palette;
    palette.accent = color(&accent.accent_strong, palette.accent);
    palette.accent_hover = color(&accent.accent_hover, palette.accent_hover);
    palette.accent_foreground = color(&accent.accent_contrast, palette.accent_foreground);
    palette.ring = color(&accent.accent, palette.ring);
    palette.accent_soft = Hsla {
        a: 0.13,
        ..palette.ring
    };
    palette.selection = Hsla {
        a: 0.35,
        ..palette.ring
    };
    palette.caret = palette.foreground;
    palette
}

/// Update only when persisted inputs change; this never schedules another frame.
pub fn apply(appearance: Option<&crate::api::Appearance>, cx: &mut App) {
    let palette = resolved(appearance);
    if theme(cx) != &palette {
        cx.set_global(palette);
    }
}

/// Installs the default palette. Call once after `gpui_kit::base::init`.
pub fn init(cx: &mut App) {
    cx.set_global(AuroraTheme::default());
}

/// Reads the installed palette.
pub fn theme(cx: &App) -> &AuroraTheme {
    cx.global::<AuroraTheme>()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn appearance(theme: &str, background: &str) -> crate::api::Appearance {
        serde_json::from_value(serde_json::json!({
            "auroraMotionSpeed": 50,
            "background": background,
            "theme": theme,
            "accent": { "type": "curated", "id": "violet" },
            "palette": {
                "accent": "#8b80ff", "accentStrong": "#6f5df2", "accentHover": "#6f63e8",
                "accentPressed": "#5b4ad6", "accentContrast": "#ffffff",
                "accentSoft": "rgba(139, 128, 255, 0.14)",
                "accentOutline": "rgba(139, 128, 255, 0.55)"
            },
            "themes": [],
            "accents": []
        }))
        .unwrap()
    }

    fn alphas(palette: &AuroraTheme) -> [f32; 6] {
        [
            palette.background.a,
            palette.sidebar.a,
            palette.surface.a,
            palette.surface_hover.a,
            palette.sunken.a,
            palette.raised.a,
        ]
    }

    #[test]
    fn simple_background_keeps_every_surface_opaque() {
        for theme in ["aurora-dark", "midnight", "oled"] {
            let palette = resolved(Some(&appearance(theme, "simple")));
            assert_eq!(alphas(&palette), [1.0; 6], "{theme}");
        }
        assert_eq!(alphas(&resolved(None)), [1.0; 6]);
    }

    #[test]
    fn borealis_makes_the_slint_surfaces_translucent() {
        let byte = |alpha: f32| (alpha * 255.0).round() as u8;
        for theme in ["aurora-dark", "midnight", "oled"] {
            let palette = resolved(Some(&appearance(theme, "borealis")));
            // Background stays opaque under the layer; border, text and the
            // accent family are never translucent.
            assert_eq!(palette.background.a, 1.0, "{theme}");
            assert_eq!(palette.border.a, 1.0);
            assert_eq!(palette.foreground.a, 1.0);
            assert_eq!(palette.accent.a, 1.0);
            assert_eq!(byte(palette.sidebar.a), borealis::SIDEBAR_ALPHA);
            assert_eq!(byte(palette.surface.a), borealis::SURFACE_ALPHA);
            assert_eq!(byte(palette.surface_hover.a), borealis::HOVER_ALPHA);
            assert_eq!(byte(palette.sunken.a), borealis::SUNKEN_ALPHA);
            assert_eq!(byte(palette.raised.a), borealis::RAISED_ALPHA);
            assert_eq!(byte(palette.card.a), borealis::CARD_ALPHA);
        }
    }

    #[test]
    fn borealis_keeps_each_themes_tones() {
        // Only the alpha differs from the simple background.
        for theme in ["aurora-dark", "midnight", "oled"] {
            let plain = resolved(Some(&appearance(theme, "simple")));
            let stylized = resolved(Some(&appearance(theme, "borealis")));
            for (plain, stylized) in [
                (plain.sidebar, stylized.sidebar),
                (plain.surface, stylized.surface),
                (plain.surface_hover, stylized.surface_hover),
                (plain.sunken, stylized.sunken),
                (plain.raised, stylized.raised),
            ] {
                assert_eq!(
                    Hsla { a: 1.0, ..stylized },
                    plain,
                    "{theme} changed tone, not just alpha"
                );
            }
        }
    }

    #[test]
    fn each_theme_has_its_slint_raised_and_sunken_tones() {
        let tone = |theme: &str| {
            let palette = resolved(Some(&appearance(theme, "simple")));
            (palette.raised, palette.sunken)
        };
        assert_eq!(
            tone("aurora-dark"),
            (rgb(0x1d1f27).into(), rgb(0x0d0e13).into())
        );
        assert_eq!(
            tone("midnight"),
            (rgb(0x191e2b).into(), rgb(0x0a0d14).into())
        );
        assert_eq!(tone("oled"), (rgb(0x141419).into(), rgb(0x000000).into()));
    }
}
