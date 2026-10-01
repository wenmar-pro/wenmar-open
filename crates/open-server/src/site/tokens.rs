//! Brand colours and type.
//!
//! These are the values of Wenmar Pro's design tokens, copied on 2026-10-01.
//! Wenmar Pro's own token crate is private and this repository is public, so
//! the values are repeated here instead of depended on. When the brand
//! changes, change them here.

/// The one brand colour. It marks the single most important action on a
/// page and nothing else.
pub const BRAND: &str = "#e50914";
pub const BRAND_HOVER: &str = "#c20d12";

/// A set of surface, border and text colours.
pub struct Palette {
    pub page: &'static str,
    pub surface: &'static str,
    pub inset: &'static str,
    pub border: &'static str,
    pub border_strong: &'static str,
    pub heading: &'static str,
    pub body: &'static str,
    pub secondary: &'static str,
}

pub const LIGHT: Palette = Palette {
    page: "#f8f7f4",
    surface: "#ffffff",
    inset: "#f1f5f9",
    border: "#e2e8f0",
    border_strong: "#cbd5e1",
    heading: "#0f172a",
    body: "#0f172a",
    secondary: "#334155",
};

pub const DARK: Palette = Palette {
    page: "#0e0e10",
    surface: "#18181b",
    inset: "#27272a",
    border: "#27272a",
    border_strong: "#3f3f46",
    heading: "#fafafa",
    body: "#d4d4d8",
    secondary: "#a1a1aa",
};

/// No font file is downloaded: the brand faces are used when the visitor
/// has them, and the system's own otherwise.
pub const FONT_SANS: &str =
    "\"DM Sans\", system-ui, -apple-system, \"Segoe UI\", Roboto, sans-serif";
pub const FONT_MONO: &str =
    "\"JetBrains Mono\", ui-monospace, SFMono-Regular, Menlo, Consolas, monospace";

/// Sizes in pixels. The product's body text is 14; a public page read on a
/// phone at a service counter uses 16, which also stops phones zooming into
/// form fields.
pub const TEXT_BODY: u8 = 16;
pub const TEXT_SMALL: u8 = 14;
pub const TEXT_SECTION: u8 = 18;
pub const TEXT_TITLE: u8 = 24;
pub const WEIGHT_SEMIBOLD: u16 = 600;

pub const RADIUS: u8 = 8;
/// The smallest height and width of anything that can be tapped.
pub const TOUCH: u8 = 44;

fn palette(palette: &Palette) -> String {
    format!(
        "--page:{};--surface:{};--inset:{};--border:{};--border-strong:{};--heading:{};--body:{};--secondary:{};",
        palette.page,
        palette.surface,
        palette.inset,
        palette.border,
        palette.border_strong,
        palette.heading,
        palette.body,
        palette.secondary
    )
}

/// The tokens as CSS custom properties, light by default and dark when the
/// visitor's system asks for it.
pub fn css() -> String {
    format!(
        ":root{{color-scheme:light dark;--brand:{BRAND};--brand-hover:{BRAND_HOVER};--sans:{FONT_SANS};--mono:{FONT_MONO};--text:{TEXT_BODY}px;--small:{TEXT_SMALL}px;--section:{TEXT_SECTION}px;--title:{TEXT_TITLE}px;--semibold:{WEIGHT_SEMIBOLD};--radius:{RADIUS}px;--touch:{TOUCH}px;{}}}\n@media (prefers-color-scheme:dark){{:root{{{}}}}}\n",
        palette(&LIGHT),
        palette(&DARK)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_css_carries_the_brand_values() {
        let css = css();
        assert!(css.contains("--brand:#e50914;"), "{css}");
        assert!(css.contains("--page:#f8f7f4;"), "{css}");
        assert!(
            css.contains("@media (prefers-color-scheme:dark){:root{--page:#0e0e10;"),
            "{css}"
        );
        assert!(css.contains("\"DM Sans\", system-ui"), "{css}");
        assert!(css.contains("--touch:44px;"), "{css}");
    }

    /// Relative luminance and contrast, as WCAG 2 defines them.
    fn luminance(hex: &str) -> f64 {
        let channel = |index: usize| {
            let value = f64::from(u8::from_str_radix(&hex[index..index + 2], 16).unwrap()) / 255.0;
            if value <= 0.03928 {
                value / 12.92
            } else {
                ((value + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * channel(1) + 0.7152 * channel(3) + 0.0722 * channel(5)
    }

    fn contrast(one: &str, other: &str) -> f64 {
        let (one, other) = (luminance(one), luminance(other));
        (one.max(other) + 0.05) / (one.min(other) + 0.05)
    }

    #[test]
    fn text_colours_are_readable_on_their_backgrounds() {
        for palette in [&LIGHT, &DARK] {
            for text in [palette.heading, palette.body, palette.secondary] {
                for background in [palette.page, palette.surface, palette.inset] {
                    assert!(
                        contrast(text, background) >= 4.5,
                        "{text} on {background} is {:.2}",
                        contrast(text, background)
                    );
                }
            }
        }
        // White text on the brand button.
        assert!(contrast("#ffffff", BRAND) >= 4.5);
        assert!(contrast("#ffffff", BRAND_HOVER) >= 4.5);
    }
}
