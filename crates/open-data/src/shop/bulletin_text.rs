//! What a bulletin's summary names: trouble codes, and topics.

use anyhow::Result;
use rusqlite::params;
use wenmar_shopdata::schema;

use crate::shop::build::ShapeContext;

/// The topic of a bulletin about a maintenance schedule or interval.
pub const MAINTENANCE: &str = "maintenance";

/// A summary that holds one of these, in either case, is about
/// maintenance.
pub const MAINTENANCE_PHRASES: [&str; 9] = [
    "maintenance schedule",
    "scheduled maintenance",
    "maintenance interval",
    "service interval",
    "service schedule",
    "replacement interval",
    "drain interval",
    "oil life monitor",
    "oil-life monitor",
];

/// A summary that names a timing belt is about maintenance only when it
/// also holds one of these: most timing belt bulletins are about a noise
/// or a repair.
pub const TIMING_BELT: &str = "timing belt";
pub const TIMING_BELT_WITH: [&str; 3] = ["interval", "schedule", "maintenance"];

/// A summary that holds none of these words, in either case, is not
/// trusted to mean a trouble code by a `B`, `C` or `U` number.
pub const CODE_WORDS: [&str; 3] = ["DTC", "CODE", "FAULT"];

/// A summary with none of [`CODE_WORDS`] that holds this word is not
/// trusted to mean a trouble code by a `P` number either: campaign
/// numbers look the same.
pub const CAMPAIGN_WORD: &str = "CAMPAIGN";

fn is_hex(byte: u8) -> bool {
    byte.is_ascii_digit() || (b'A'..=b'F').contains(&byte)
}

/// The trouble codes a summary names, each once, sorted.
///
/// A code is a capital `P`, `B`, `C` or `U`, then one of `0` to `3`, then
/// three of `0` to `9` and `A` to `F`. The character before it is not a
/// letter or a digit. After it comes the end, a character that is not a
/// letter or a digit, or exactly two more of `0` to `9` and `A` to `F`
/// (the failure type some makers append, as in `P029900`) and then the
/// same. A number whose nearest character before it, blanks aside, is `#`
/// is not a code. See [`CODE_WORDS`] and [`CAMPAIGN_WORD`] for the
/// summaries in which a number is not counted.
pub fn codes(summary: &str) -> Vec<String> {
    let upper = summary.to_ascii_uppercase();
    let has_code_word = CODE_WORDS.iter().any(|word| upper.contains(word));
    let has_campaign = upper.contains(CAMPAIGN_WORD);
    let bytes = summary.as_bytes();
    let mut found: Vec<String> = Vec::new();
    let mut index = 0;
    while index + 5 <= bytes.len() {
        let letter = bytes[index];
        let starts = matches!(letter, b'P' | b'B' | b'C' | b'U')
            && (index == 0 || !bytes[index - 1].is_ascii_alphanumeric())
            && (b'0'..=b'3').contains(&bytes[index + 1])
            && bytes[index + 2..index + 5].iter().all(|byte| is_hex(*byte));
        if !starts {
            index += 1;
            continue;
        }
        let after = &bytes[index + 5..];
        let ends = |rest: &[u8]| {
            rest.first()
                .is_none_or(|byte| !byte.is_ascii_alphanumeric())
        };
        let whole = ends(after)
            || (after.len() >= 2
                && after[..2].iter().all(|byte| is_hex(*byte))
                && ends(&after[2..]));
        let after_hash = bytes[..index]
            .iter()
            .rev()
            .find(|byte| !byte.is_ascii_whitespace())
            .is_some_and(|byte| *byte == b'#');
        let trusted = has_code_word || (letter == b'P' && !has_campaign);
        if whole && !after_hash && trusted {
            // Five ASCII bytes, so this is a character boundary.
            let code = summary[index..index + 5].to_owned();
            if !found.contains(&code) {
                found.push(code);
            }
        }
        index += 5;
    }
    found.sort_unstable();
    found
}

/// The topics a summary is about.
pub fn topics(summary: &str) -> Vec<&'static str> {
    let lower = summary.to_lowercase();
    let maintenance = MAINTENANCE_PHRASES
        .iter()
        .any(|phrase| lower.contains(phrase))
        || (lower.contains(TIMING_BELT)
            && TIMING_BELT_WITH.iter().any(|word| lower.contains(word)));
    if maintenance {
        vec![MAINTENANCE]
    } else {
        Vec::new()
    }
}

/// Fills `bulletin_code` and `bulletin_topic` from the summaries in
/// `bulletin`. Runs after the bulletin shape step.
pub fn shape(context: &mut ShapeContext<'_>) -> Result<()> {
    let transaction = context.transaction;
    transaction.execute_batch(schema::BULLETIN_TEXT)?;
    let mut select =
        transaction.prepare("SELECT id, summary FROM bulletin WHERE summary IS NOT NULL")?;
    let mut code =
        transaction.prepare("INSERT INTO bulletin_code (bulletin_id, code) VALUES (?1, ?2)")?;
    let mut topic =
        transaction.prepare("INSERT INTO bulletin_topic (bulletin_id, topic) VALUES (?1, ?2)")?;
    let mut rows = select.query([])?;
    while let Some(row) = rows.next()? {
        let (id, summary): (i64, String) = (row.get(0)?, row.get(1)?);
        for found in codes(&summary) {
            code.execute(params![id, found])?;
        }
        for found in topics(&summary) {
            topic.execute(params![id, found])?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_are_found_in_a_summary() {
        assert_eq!(
            codes("MIL ON WITH DTC P0326. Also P0300/P0301, (P0420) and p0171."),
            ["P0300", "P0301", "P0326", "P0420"]
        );
        // The failure type some makers append.
        assert_eq!(
            codes("Fault memory entry P029900, U010087 and B13034A; C1113-07; B128A:12"),
            ["B128A", "B1303", "C1113", "P0299", "U0100"]
        );
        // Each once, sorted.
        assert_eq!(codes("P0456 P0442 P0456"), ["P0442", "P0456"]);
        assert_eq!(codes("P0A80 and P3FFF"), ["P0A80", "P3FFF"]);
        // Not codes: part of a longer word or number, a first digit over 3,
        // lowercase, too short.
        for text in [
            "XP0420",
            "P04201",
            "P0420A",
            "P4420",
            "P042",
            "AP0420 code",
            "p0420",
            "P0G20",
        ] {
            assert!(codes(text).is_empty(), "{text}");
        }
    }

    #[test]
    fn a_number_is_not_counted_where_it_is_likely_something_else() {
        // B, C and U numbers need a word that says a code is meant.
        assert!(codes("TORSION BAR FRONT SUSPENSION ON C1500 MODELS.").is_empty());
        assert!(codes("AUTOMATIC TRANSAXLE (U240E). REPAIR MANUAL CORRECTION.").is_empty());
        assert!(codes("VAN HOOL C2045 COACH MODEL").is_empty());
        assert_eq!(codes("ABS light on, DTC C0035"), ["C0035"]);
        assert_eq!(codes("Fault U0100 stored"), ["U0100"]);
        assert_eq!(codes("trouble code B1650"), ["B1650"]);
        // P numbers are counted without one.
        assert_eq!(codes("MIL ON P0840, LACK OF POWER."), ["P0840"]);
        // Unless the summary is about a campaign, or the number follows #.
        assert!(
            codes("Nissan has assigned identification number P1A01 to this campaign.").is_empty()
        );
        assert!(codes("eligible for reimbursement under Campaign ID # P3104.").is_empty());
        assert_eq!(codes("Campaign ID #P3104 covers DTC P0420"), ["P0420"]);
        assert_eq!(codes("Campaign 21S01: DTC P0300 may set"), ["P0300"]);
        assert!(codes("").is_empty());
        // Text outside ASCII does not break the scan.
        assert_eq!(codes("\u{2022} P0171 \u{2013} lean \u{e9}"), ["P0171"]);
    }

    #[test]
    fn a_summary_about_a_schedule_or_an_interval_is_maintenance() {
        for text in [
            "2015 Volkswagen Beetle Maintenance Schedule For USA",
            "SCHEDULED MAINTENANCE CHECKLIST UPDATE",
            "Fluid Maintenance Service Interval.",
            "New engine oil and filter replacement interval",
            "Maximum Oil Drain Intervals",
            "UNABLE TO RESET ENGINE OIL LIFE MONITOR.",
            "follow Intelligent Oil-Life Monitor messages",
            "TIMING BELT REPLACEMENT INTERVAL",
            "Timing belt added to the maintenance table",
        ] {
            assert_eq!(topics(text), [MAINTENANCE], "{text}");
        }
        for text in [
            "",
            "THE TIMING BELT TENSIONED TOO TIGHT CAUSING A WHINE",
            "excessive oil consumption between oil changes",
            "Maintenance reminder light reset",
        ] {
            assert!(topics(text).is_empty(), "{text}");
        }
    }
}
