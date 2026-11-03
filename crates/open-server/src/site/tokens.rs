//! Brand colours and type.
//!
//! These are the Wenmar web palette, shared with wenmarpro.com, Wenmar Find and
//! Wenmar Jobs (2026-11-03; Wenmar Pro's brand guide, "Web palette"): porcelain
//! ground, white cards, scarlet for actions. Wenmar Pro's own token crate is
//! private and this repository is public, so the values are repeated here
//! instead of depended on. When the brand changes, change them here.

/// The red of the logo mark and the wordmark's "Open", and nothing else. Large
/// bold text: 3 to 1 is enough on every ground.
pub const BRAND: &str = "#e50914";
/// Scarlet: the main action on a page, and the focus outline. White text on it
/// passes AA.
pub const ACCENT: &str = "#db222a";
/// The accent under the pointer, in both schemes: darker, so white text on it
/// still passes.
pub const ACCENT_HOVER: &str = "#b81b22";

/// A set of surface, border and text colours.
pub struct Palette {
    pub page: &'static str,
    pub surface: &'static str,
    pub inset: &'static str,
    pub border: &'static str,
    pub border_strong: &'static str,
    /// The edge of a text field or a picker. At least 3 to 1 against the
    /// page and the card, which a field needs to be seen as a field.
    pub control: &'static str,
    pub heading: &'static str,
    pub body: &'static str,
    pub secondary: &'static str,
    /// A warning's text and its ground: the brand's amber status pair.
    pub warn_text: &'static str,
    pub warn_bg: &'static str,
}

pub const LIGHT: Palette = Palette {
    page: "#f4f4ed",
    surface: "#ffffff",
    inset: "#eaeae1",
    border: "#deded4",
    border_strong: "#8b959e",
    control: "#78828b",
    heading: "#050609",
    body: "#050609",
    secondary: "#4a5661",
    warn_text: "#8a4b00",
    warn_bg: "#fdf1dc",
};

pub const DARK: Palette = Palette {
    page: "#050609",
    surface: "#101217",
    inset: "#1c1f25",
    border: "#262a31",
    border_strong: "#4a525c",
    control: "#5c636e",
    heading: "#f4f4ed",
    body: "#f4f4ed",
    secondary: "#b4bdc6",
    warn_text: "#fbbf24",
    warn_bg: "#2a1c05",
};

/// The brand faces are served by this site (`assets/fonts/`). After each
/// comes what is used while it loads or if it does not: for the sans, a
/// face made of the visitor's own Arial with DM Sans's proportions, so the
/// text does not move when the brand face arrives.
pub const FONT_SANS: &str =
    "\"DM Sans\", \"DM Sans Fallback\", system-ui, -apple-system, \"Segoe UI\", Roboto, sans-serif";
pub const FONT_MONO: &str =
    "\"JetBrains Mono\", ui-monospace, SFMono-Regular, Menlo, Consolas, monospace";

/// Sizes in pixels. The product's body text is 14; a public page read on a
/// phone at a service counter uses 16, which also stops phones zooming into
/// form fields.
pub const TEXT_BODY: u8 = 16;
pub const TEXT_SMALL: u8 = 14;
pub const TEXT_SECTION: u8 = 18;
pub const TEXT_TITLE: u8 = 24;
pub const WEIGHT_MEDIUM: u16 = 500;
pub const WEIGHT_SEMIBOLD: u16 = 600;
/// The wordmark. At 20 pixels and bold it is large text, for which red on
/// the page colour is enough contrast.
pub const TEXT_WORDMARK: u8 = 20;

pub const RADIUS: u8 = 8;
/// The smallest height and width of anything that can be tapped.
pub const TOUCH: u8 = 44;

fn palette(palette: &Palette) -> String {
    format!(
        "--page:{};--surface:{};--inset:{};--border:{};--border-strong:{};--control:{};--heading:{};--body:{};--secondary:{};--warn-text:{};--warn-bg:{};",
        palette.page,
        palette.surface,
        palette.inset,
        palette.border,
        palette.border_strong,
        palette.control,
        palette.heading,
        palette.body,
        palette.secondary,
        palette.warn_text,
        palette.warn_bg
    )
}

/// The tokens as CSS custom properties, light by default and dark when the
/// visitor's system asks for it.
pub fn css() -> String {
    format!(
        ":root{{color-scheme:light dark;--brand:{BRAND};--accent:{ACCENT};--accent-hover:{ACCENT_HOVER};--sans:{FONT_SANS};--mono:{FONT_MONO};--text:{TEXT_BODY}px;--small:{TEXT_SMALL}px;--section:{TEXT_SECTION}px;--title:{TEXT_TITLE}px;--wordmark:{TEXT_WORDMARK}px;--medium:{WEIGHT_MEDIUM};--semibold:{WEIGHT_SEMIBOLD};--radius:{RADIUS}px;--touch:{TOUCH}px;{}}}\n@media (prefers-color-scheme:dark){{:root{{{}}}}}\n",
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
        assert!(css.contains("--accent:#db222a;"), "{css}");
        assert!(css.contains("--page:#f4f4ed;"), "{css}");
        assert!(
            css.contains("@media (prefers-color-scheme:dark){:root{--page:#050609;"),
            "{css}"
        );
        assert!(
            css.contains("\"DM Sans\", \"DM Sans Fallback\", system-ui"),
            "{css}"
        );
        assert!(css.contains("--touch:44px;"), "{css}");
        assert!(css.contains("--control:#78828b;"), "{css}");
        assert!(css.contains("--control:#5c636e;"), "{css}");
        assert!(css.contains("--wordmark:20px;"), "{css}");
    }

    #[test]
    fn every_pair_of_text_and_ground_meets_wcag_aa() {
        for (scheme, palette) in [("light", &LIGHT), ("dark", &DARK)] {
            // Text of ordinary size: 4.5 to 1.
            for text in [palette.heading, palette.body, palette.secondary] {
                for ground in [palette.page, palette.surface, palette.inset] {
                    assert!(
                        contrast(text, ground) >= 4.5,
                        "{scheme}: {text} on {ground} is {:.2}",
                        contrast(text, ground)
                    );
                }
            }
            assert!(
                contrast(palette.warn_text, palette.warn_bg) >= 4.5,
                "{scheme}: a warning is {:.2}",
                contrast(palette.warn_text, palette.warn_bg)
            );
            // The edge of a field: 3 to 1 against what it sits on.
            for ground in [palette.page, palette.surface] {
                assert!(
                    contrast(palette.control, ground) >= 3.0,
                    "{scheme}: a field's edge on {ground} is {:.2}",
                    contrast(palette.control, ground)
                );
            }
            // The focus outline, and the red half of the wordmark, which is
            // large text: 3 to 1.
            for ground in [palette.page, palette.surface, palette.inset] {
                assert!(
                    contrast(BRAND, ground) >= 3.0,
                    "{scheme}: red on {ground} is {:.2}",
                    contrast(BRAND, ground)
                );
            }
        }
        // White text on the main button, and the focus outline on every ground.
        assert!(contrast("#ffffff", ACCENT) >= 4.5);
        assert!(contrast("#ffffff", ACCENT_HOVER) >= 4.5);
        for palette in [&LIGHT, &DARK] {
            for ground in [palette.page, palette.surface] {
                assert!(
                    contrast(ACCENT, ground) >= 3.0,
                    "the focus outline on {ground}"
                );
            }
        }
    }

    #[test]
    fn the_layout_names_the_page_colours_of_both_schemes() {
        let layout = include_str!("../../templates/base.html");
        for palette in [&LIGHT, &DARK] {
            assert!(
                layout.contains(&format!("content=\"{}\"", palette.page)),
                "{}",
                palette.page
            );
        }
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
}
