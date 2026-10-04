//! A figure shown beside its gross profit target.
//!
//! The range, the usual target and the pages that state them are read from
//! `shop_math::targets`. Nothing here, and no template, writes a target or
//! a source of its own.

use shop_math::Percent;
use shop_math::targets::{Source, Standing, Target};

/// What the page shows where a result has a target: the figure, the range,
/// where the figure stands, the usual target, and where each comes from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetView {
    /// What the figure is called on this page: `Blended margin`.
    pub label: String,
    pub figure: String,
    /// `below`, `inside` or `above` the range. It is stated, never coloured.
    pub standing: Standing,
    /// What the target is a target for: `parts gross profit`.
    pub of: &'static str,
    pub low: String,
    pub high: String,
    pub usual: String,
    pub range_sources: &'static [Source],
    pub usual_sources: &'static [Source],
}

impl TargetView {
    pub fn new(label: &str, figure: Percent, of: &'static str, target: &Target) -> TargetView {
        TargetView {
            label: label.to_owned(),
            figure: figure.to_string(),
            standing: target.standing(figure),
            of,
            low: target.range.low.to_string(),
            high: target.range.high.to_string(),
            usual: target.usual.to_string(),
            range_sources: target.range_sources,
            usual_sources: target.usual_sources,
        }
    }

    /// The first sentences of the block, as plain text. A Markdown version
    /// uses them for its worked example.
    pub fn sentence(&self) -> String {
        format!(
            "{}: {}. That is {} the typical range of {} to {} for {}. The usual target is {}.",
            self.label, self.figure, self.standing, self.low, self.high, self.of, self.usual
        )
    }
}

/// The pages that state a target, as link text and address, for a page of
/// prose: the range's sources, then the usual target's.
pub fn source_links(target: &Target) -> Vec<(String, String)> {
    let mut links: Vec<(String, String)> = Vec::new();
    for source in target.range_sources.iter().chain(target.usual_sources) {
        let link = (
            format!("{}: {}", source.name, source.title),
            source.url.to_owned(),
        );
        if !links.contains(&link) {
            links.push(link);
        }
    }
    links
}

#[cfg(test)]
mod tests {
    use shop_math::targets::{LABOR, OVERALL, PARTS};

    use super::*;

    #[test]
    fn a_figure_is_shown_with_its_range_its_standing_and_the_usual_target() {
        let inside = TargetView::new(
            "Blended margin",
            Percent::from_thousandths(43_662),
            "parts gross profit",
            &PARTS,
        );
        assert_eq!(
            inside.sentence(),
            "Blended margin: 43.662%. That is inside the typical range of 40% to 50% for parts gross profit. The usual target is 50%."
        );
        let below = TargetView::new(
            "Blended margin",
            Percent::whole(35),
            "parts gross profit",
            &PARTS,
        );
        assert!(below.sentence().contains("That is below the typical range"));
        let above = TargetView::new(
            "Blended margin",
            Percent::whole(60),
            "parts gross profit",
            &PARTS,
        );
        assert!(above.sentence().contains("That is above the typical range"));
        // Both ends of a range are inside it.
        for edge in [40, 50] {
            let view =
                TargetView::new("Margin", Percent::whole(edge), "parts gross profit", &PARTS);
            assert_eq!(view.standing, Standing::Inside, "{edge}");
        }
    }

    #[test]
    fn a_view_carries_the_sources_of_its_own_target_and_no_others() {
        for target in [&LABOR, &PARTS, &OVERALL] {
            let view = TargetView::new("Figure", Percent::whole(50), "gross profit", target);
            assert_eq!(view.range_sources, target.range_sources);
            assert_eq!(view.usual_sources, target.usual_sources);
            assert!(!view.range_sources.is_empty() && !view.usual_sources.is_empty());
            let links = source_links(target);
            assert_eq!(
                links.len(),
                target.range_sources.len() + target.usual_sources.len()
            );
            for (text, address) in &links {
                assert!(address.starts_with("https://"), "{address}");
                assert!(text.contains(": "), "{text}");
            }
        }
    }
}
