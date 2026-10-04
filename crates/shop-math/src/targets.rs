//! The gross profit targets, and where each comes from.
//!
//! This is the only place a target is written. A page shows a range, a
//! usual target and their sources by reading them from here.
//!
//! The ranges are what the source calls typical for a general repair shop,
//! and the usual targets are what the sources say shops aim for. They are
//! not a survey of what shops earn.

use std::fmt;

use crate::percent::Percent;

/// Where a figure stands against a range.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Standing {
    Below,
    Inside,
    Above,
}

/// The standing as a word for a sentence: `below`, `inside` or `above`.
impl fmt::Display for Standing {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Standing::Below => "below",
            Standing::Inside => "inside",
            Standing::Above => "above",
        })
    }
}

/// A range of percentages. Both ends are inside it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Range {
    pub low: Percent,
    pub high: Percent,
}

impl Range {
    pub fn standing(&self, figure: Percent) -> Standing {
        if figure < self.low {
            Standing::Below
        } else if figure > self.high {
            Standing::Above
        } else {
            Standing::Inside
        }
    }
}

/// A public page that states a figure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Source {
    /// The company whose page it is.
    pub name: &'static str,
    /// The title of the page.
    pub title: &'static str,
    pub url: &'static str,
}

/// A gross profit target: the typical range, the usual target, and the
/// pages that state each.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Target {
    pub range: Range,
    pub usual: Percent,
    pub range_sources: &'static [Source],
    pub usual_sources: &'static [Source],
}

impl Target {
    /// Where a figure stands against the typical range.
    pub fn standing(&self, figure: Percent) -> Standing {
        self.range.standing(figure)
    }
}

const WICKEDFILE: Source = Source {
    name: "WickedFile",
    title: "Parts vs. Labor Margin: Why Most Auto Repair Shops Are Getting It Backwards",
    url: "https://www.wickedfile.com/blogs/parts-vs-labor-margin-auto-repair/",
};

const ELITE_SIX_STRATEGIES: Source = Source {
    name: "Elite Worldwide",
    title: "Six Strategies to Keep Your Auto Repair Shop Profitable",
    url: "https://eliteworldwide.com/info-center/six-strategies-to-keep-your-auto-repair-shop-profitable/",
};

const ELITE_COMPETITIVE: Source = Source {
    name: "Elite Worldwide",
    title: "Keeping Your Shop Competitive and Profitable",
    url: "https://eliteworldwide.com/info-center/keeping-your-shop-competitive-and-profitable/",
};

const PARTSTECH: Source = Source {
    name: "PartsTech",
    title: "5 Keys to Maximizing Your Shop's Profits",
    url: "https://partstech.com/resource/blog/the-keys-to-maximizing-your-shops-profits/",
};

/// Gross profit on labor: labor sales less what the technicians cost.
pub const LABOR: Target = Target {
    range: Range {
        low: Percent::whole(60),
        high: Percent::whole(75),
    },
    usual: Percent::whole(70),
    range_sources: &[WICKEDFILE],
    usual_sources: &[ELITE_SIX_STRATEGIES, PARTSTECH],
};

/// Gross profit on parts: parts sales less what the shop paid for them.
pub const PARTS: Target = Target {
    range: Range {
        low: Percent::whole(40),
        high: Percent::whole(50),
    },
    usual: Percent::whole(50),
    range_sources: &[WICKEDFILE],
    usual_sources: &[ELITE_COMPETITIVE],
};

/// Gross profit across labor, parts and sublet together.
pub const OVERALL: Target = Target {
    range: Range {
        low: Percent::whole(50),
        high: Percent::whole(60),
    },
    usual: Percent::whole(60),
    range_sources: &[WICKEDFILE],
    usual_sources: &[PARTSTECH],
};
