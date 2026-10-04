//! `/guides`: five short pages that answer what people ask about VINs, and
//! a list of them.
//!
//! Each is written once, as a [`Doc`], and shown as HTML and as Markdown.
//! What they say is what the decoder does: the year chart, the weights and
//! the letter values here are checked against `wenmar-vin` in the tests,
//! and the worked example is computed, not typed.

use axum::extract::rejection::PathRejection;
use axum::extract::{Path, State};
use axum::response::Response;

use shop_math::parts_matrix::margin_to_markup;
use shop_math::targets::PARTS;

use crate::site::markdown::{self, Doc, Format, Table};
use crate::site::pages::{render, section, with_code, with_links, with_table};
use crate::site::tools::target::source_links;
use crate::site::{self, Page, jsonld, seo};
use crate::state::AppState;

/// The VIN the guides take apart: a 2023 Hyundai Kona. It is the example
/// on `/docs` too.
pub const EXAMPLE: &str = "KM8K2CAB4PU001140";

/// The 30 characters that stand for a model year in position 10, in year
/// order: the first is 1980 and 2010.
pub const YEAR_CODES: &str = "ABCDEFGHJKLMNPRSTVWXY123456789";

/// The weight of each of the 17 positions in the check digit.
pub const CHECK_WEIGHTS: [u32; 17] = [8, 7, 6, 5, 4, 3, 2, 10, 0, 9, 8, 7, 6, 5, 4, 3, 2];

/// The value of each letter in the check digit.
pub const LETTER_VALUES: [(u32, &str); 9] = [
    (1, "AJ"),
    (2, "BKS"),
    (3, "CLT"),
    (4, "DMU"),
    (5, "ENV"),
    (6, "FW"),
    (7, "GPX"),
    (8, "HY"),
    (9, "RZ"),
];

/// The value of one character of a VIN in the check digit: a digit is
/// itself, a letter is looked up.
pub fn check_value(character: char) -> u32 {
    match character.to_digit(10) {
        Some(digit) => digit,
        None => LETTER_VALUES
            .iter()
            .find(|(_, letters)| letters.contains(character))
            .map_or(0, |(value, _)| *value),
    }
}

/// The check digit a VIN should carry, by the tables above.
pub fn check_digit_of(vin: &str) -> char {
    let sum: u32 = vin
        .chars()
        .zip(CHECK_WEIGHTS)
        .map(|(character, weight)| check_value(character) * weight)
        .sum();
    match sum % 11 {
        10 => 'X',
        digit => char::from_digit(digit, 10).unwrap_or('0'),
    }
}

fn cells(row: &[&str]) -> Vec<String> {
    row.iter().map(|cell| (*cell).to_owned()).collect()
}

fn list(numbers: &[u32]) -> String {
    numbers
        .iter()
        .map(u32::to_string)
        .collect::<Vec<_>>()
        .join(", ")
}

/// The check digit of [`EXAMPLE`], step by step, in words.
fn worked_example() -> String {
    let values: Vec<u32> = EXAMPLE.chars().map(check_value).collect();
    let products: Vec<u32> = values
        .iter()
        .zip(CHECK_WEIGHTS)
        .map(|(value, weight)| value * weight)
        .collect();
    let sum: u32 = products.iter().sum();
    format!(
        "For {EXAMPLE} the values are {}. Multiplied by the weights they are {}, which add up to {sum}. {sum} divided by 11 is {} with {} left over, so the check digit is {}. Position 9 of {EXAMPLE} is {}, so the check passes.",
        list(&values),
        list(&products),
        sum / 11,
        sum % 11,
        check_digit_of(EXAMPLE),
        EXAMPLE.chars().nth(8).unwrap_or(' ')
    )
}

fn how_to_read(base: &str) -> Doc {
    Doc {
        title: "How to read a VIN".to_owned(),
        intro: format!(
            "A vehicle identification number, or VIN, has 17 characters. Each position has a job, and the layout has been the same since the 1981 model year. This page goes through the positions in order, using {EXAMPLE}, a 2023 Hyundai Kona, as the example."
        ),
        sections: vec![
            with_table(
                section(
                    "The 17 positions",
                    &[
                        "A VIN uses the digits 0 to 9 and every capital letter except I, O and Q, which look too much like 1 and 0.",
                    ],
                ),
                Table {
                    caption: format!("The positions of {EXAMPLE}"),
                    head: cells(&["Position", "Example", "What it is"]),
                    rows: vec![
                        cells(&[
                            "1 to 3",
                            "KM8",
                            "Who built the vehicle: the manufacturer code.",
                        ]),
                        cells(&[
                            "4 to 8",
                            "K2CAB",
                            "What the vehicle is. The manufacturer chooses the meaning: usually the model, the body, the restraints and the engine.",
                        ]),
                        cells(&[
                            "9",
                            "4",
                            "The check digit, worked out from the other 16 characters.",
                        ]),
                        cells(&["10", "P", "The model year. P is 2023."]),
                        cells(&["11", "U", "The plant the vehicle was built in."]),
                        cells(&["12 to 17", "001140", "The serial number of this vehicle."]),
                    ],
                    prose: true,
                },
            ),
            with_links(
                section(
                    "Positions 1 to 3: the manufacturer",
                    &[
                        "The first character is a region, the first two are a country, and all three name one manufacturer and often one kind of vehicle. KM8 is Hyundai, for multipurpose vehicles built in South Korea. A small manufacturer shares its first three characters with others: its third character is 9, and positions 12 to 14 finish its code.",
                    ],
                ),
                &[
                    (
                        "What the first three characters of a VIN mean",
                        format!("{base}/guides/wmi"),
                    ),
                    ("Manufacturer code KM8", format!("{base}/wmi/KM8")),
                ],
            ),
            with_links(
                section(
                    "Positions 4 to 8: the vehicle",
                    &[
                        "These five characters describe the vehicle, and each manufacturer decides how. One puts the model in positions 4 and 5 and the engine in position 8; another does it differently, and the meaning can change from one model year to the next. That is why decoding them needs the manufacturer's own tables, which it files with NHTSA.",
                        "For many vehicles sold in North America position 8 is the engine. A model-year page on this site lists the eighth character for each engine where the data settles it.",
                    ],
                ),
                &[(
                    "Every make, with its models by year",
                    format!("{base}/makes"),
                )],
            ),
            with_links(
                section(
                    "Position 9: the check digit",
                    &[
                        "The ninth character is worked out from the other sixteen, so it catches most typing mistakes. It is required on vehicles built for the United States and Canada. Many vehicles built for other markets do not use it, so a VIN that fails the check is not always wrong.",
                    ],
                ),
                &[(
                    "What the VIN check digit is",
                    format!("{base}/guides/check-digit"),
                )],
            ),
            with_links(
                section(
                    "Position 10: the model year",
                    &[
                        "One character stands for the model year, in a cycle of 30 years: P was 1993 and is 2023. On cars, multipurpose vehicles and light trucks, position 7 says which: a digit there means 1980 to 2009 and a letter means 2010 to 2039. In the example position 7 is A, so P is 2023.",
                    ],
                ),
                &[(
                    "How a VIN encodes the model year",
                    format!("{base}/guides/model-year"),
                )],
            ),
            section(
                "Position 11: the plant",
                &[
                    "One character for the factory. Each manufacturer has its own list, so U at one manufacturer is not U at another.",
                ],
            ),
            section(
                "Positions 12 to 17: the serial number",
                &[
                    "The last six characters are what makes one VIN different from the next. A small manufacturer has only positions 15 to 17 for the serial number, because 12 to 14 belong to its manufacturer code.",
                ],
            ),
            with_links(
                section(
                    "Decode one",
                    &[
                        "Type a VIN into the box on the first page to see every field this site has for it.",
                    ],
                ),
                &[("Decode a VIN", format!("{base}/"))],
            ),
        ],
    }
}

fn where_to_find(base: &str) -> Doc {
    Doc {
        title: "Where to find the VIN on a vehicle".to_owned(),
        intro: "The VIN is on the vehicle in at least two places and on most of its papers. If two of them disagree, go by the one stamped or riveted to the vehicle.".to_owned(),
        sections: vec![
            section(
                "On a car or a light truck",
                &[
                    "At the bottom of the windshield on the driver's side, on a small plate on top of the dash. It is read from outside, standing by the driver's door and looking down through the glass. Vehicles built for the United States and Canada have it there.",
                    "On the driver's door jamb or the edge of the driver's door, on the label that also gives the date of manufacture and the weight ratings.",
                    "Stamped into the body or the frame. Common places are the firewall at the back of the engine bay, the top of a front strut tower, the floor beside a front seat, and a frame rail on a truck.",
                ],
            ),
            section(
                "On the papers",
                &["On the ownership or registration, on the insurance card, and on the title where one is issued. A bill of sale and most service invoices carry it too."],
            ),
            section(
                "On a motorcycle, a trailer or a heavy truck",
                &[
                    "A motorcycle has it stamped on the steering neck, behind the headlight, and often on a label on the frame.",
                    "A trailer has it on a plate or label near the front on the left side, usually on the tongue or the frame.",
                    "A heavy truck has it on the label on the driver's door jamb and stamped on a frame rail.",
                ],
            ),
            with_links(
                section(
                    "Reading it without mistakes",
                    &[
                        "A VIN never contains the letters I, O or Q. If you read one of those, it is the digit 1 or 0.",
                        "The pairs most often mixed up are S and 5, B and 8, Z and 2, G and 6, D and 0, U and V, and L and 1. When a VIN typed here fails its check digit, the result page offers the VINs that pass the check and differ by one such swap.",
                        "A VIN from before the 1981 model year can be shorter than 17 characters and follows its manufacturer's own layout. This site decodes 17-character VINs only.",
                    ],
                ),
                &[
                    ("How to read a VIN", format!("{base}/guides/how-to-read-a-vin")),
                    ("Decode a VIN", format!("{base}/")),
                ],
            ),
        ],
    }
}

fn wmi(base: &str) -> Doc {
    Doc {
        title: "What the first three characters of a VIN mean".to_owned(),
        intro: "The first three characters of a VIN are the World Manufacturer Identifier, or WMI. They say who built the vehicle. This site has a page for every one NHTSA has on file.".to_owned(),
        sections: vec![
            with_links(
                section(
                    "What each character says",
                    &[
                        "The first character is a region, and with the second it is a country. 1, 4 and 5 are the United States, 2 is Canada, J is Japan and W is Germany. Other countries share a first character: KL to KR is South Korea, 3A to 3W is Mexico, and SA to SM is the United Kingdom.",
                        "The third character, with the first two, names one manufacturer and often one kind of vehicle. Honda's cars built in the United States start with 1HG. Hyundai's multipurpose vehicles built in South Korea start with KM8.",
                        "The country in a manufacturer code is where the code was registered, which is usually where the vehicle was built. The plant in position 11 is the more exact answer.",
                    ],
                ),
                &[("Manufacturer code KM8", format!("{base}/wmi/KM8"))],
            ),
            section(
                "Small manufacturers use six characters",
                &["A manufacturer that builds few vehicles shares its first three characters with others. Its third character is 9, and positions 12, 13 and 14 of the VIN finish the code. On this site such a code is written as six characters: 1P9618 means a VIN that starts with 1P9 and has 618 in positions 12 to 14."],
            ),
            with_links(
                section(
                    "One manufacturer, many codes",
                    &["A large manufacturer has many codes: one for each country, division and kind of vehicle it builds. A make's page on this site lists the codes its vehicles use."],
                ),
                &[("Every make", format!("{base}/makes"))],
            ),
            with_code(
                section(
                    "Looking one up",
                    &["Decode a whole VIN on the first page, or put the code after /wmi/ in this site's address. The same page as Markdown, for a program:"],
                ),
                format!("curl {base}/wmi/KM8.md"),
            ),
        ],
    }
}

fn model_year(base: &str) -> Doc {
    let rows = YEAR_CODES
        .chars()
        .zip(0u16..)
        .map(|(code, index)| {
            vec![
                code.to_string(),
                (1980 + index).to_string(),
                (2010 + index).to_string(),
            ]
        })
        .collect();
    Doc {
        title: "How a VIN encodes the model year".to_owned(),
        intro: "The tenth character of a VIN is the model year. There are 30 codes, so each one comes round again after 30 years: the character that meant 1993 means 2023.".to_owned(),
        sections: vec![
            with_table(
                section(
                    "The chart",
                    &["The codes run through the letters, leaving out I, O, Q, U and Z, and then through the digits 1 to 9. Zero is never a year."],
                ),
                Table {
                    caption: "Model year by tenth character".to_owned(),
                    head: cells(&["Character", "1980 to 2009", "2010 to 2039"]),
                    rows,
                    prose: false,
                },
            ),
            section(
                "Which of the two years",
                &[
                    "On cars, multipurpose vehicles and light trucks, position 7 settles it. A digit in position 7 means the earlier cycle, 1980 to 2009. A letter means the later one, 2010 to 2039.",
                    &format!("{EXAMPLE} has P in position 10 and A in position 7, so it is a 2023. 1HGCM82633A004352 has 3 in position 10 and 2 in position 7, so it is a 2003."),
                    "Heavy trucks, buses, trailers and motorcycles do not follow the position 7 rule. For them this site tries both years against what the manufacturer filed and keeps the one that fits, preferring the later.",
                    "A model year is never more than two years ahead of the calendar. A code whose later year would be further ahead than that is read as the earlier year.",
                    "The model year is not the year the vehicle was built. A 2023 model is often built in 2022.",
                ],
            ),
            with_links(
                section(
                    "When position 10 is not a year",
                    &["Some vehicles built for markets outside North America do not put the model year in position 10. If the character there is not one of the 30 codes, this site says the model year is unknown instead of guessing."],
                ),
                &[
                    ("How to read a VIN", format!("{base}/guides/how-to-read-a-vin")),
                    ("Decode a VIN", format!("{base}/")),
                ],
            ),
        ],
    }
}

fn check_digit(base: &str) -> Doc {
    let letters = LETTER_VALUES
        .iter()
        .map(|(value, letters)| {
            let spelled: Vec<String> = letters.chars().map(String::from).collect();
            vec![value.to_string(), spelled.join(", ")]
        })
        .collect();
    let weights = CHECK_WEIGHTS
        .iter()
        .zip(1u32..)
        .map(|(weight, position)| vec![position.to_string(), weight.to_string()])
        .collect();
    Doc {
        title: "What the VIN check digit is".to_owned(),
        intro: "The ninth character of a VIN is a check digit. It is worked out from the other sixteen characters, so a VIN with one character mistyped almost never passes the check.".to_owned(),
        sections: vec![
            section(
                "How it is worked out",
                &["Every character has a value: a digit is its own value, and a letter has the value in the first table below. Each value is multiplied by the weight of its position, in the second table. The 17 results are added up and divided by 11. The remainder is the check digit, and a remainder of 10 is written X."],
            ),
            with_table(
                section("Letter values", &["I, O and Q have no value, because a VIN never contains them."]),
                Table {
                    caption: "The value of each letter".to_owned(),
                    head: cells(&["Value", "Letters"]),
                    rows: letters,
                    prose: false,
                },
            ),
            with_table(
                section(
                    "Position weights",
                    &["Position 9 is the check digit itself, so its weight is 0."],
                ),
                Table {
                    caption: "The weight of each position".to_owned(),
                    head: cells(&["Position", "Weight"]),
                    rows: weights,
                    prose: false,
                },
            ),
            section("A worked example", &[&worked_example()]),
            with_links(
                section(
                    "When the check fails",
                    &[
                        "A wrong check digit usually means a character was misread. This site still decodes the VIN, says what position 9 should be, and offers the VINs that pass the check and differ by one easily confused character.",
                        "It can also mean nothing is wrong. The check digit is required on vehicles built for the United States and Canada. Many vehicles built for other markets carry any character in position 9, and their VINs fail the check while being genuine. That is why the API reports a wrong check digit as a warning and not as an error.",
                    ],
                ),
                &[
                    ("Where to find the VIN on a vehicle", format!("{base}/guides/where-to-find-the-vin")),
                    ("Decode a VIN", format!("{base}/")),
                ],
            ),
        ],
    }
}

/// Who says what shops aim for: the other page below when one source gives
/// the target, and the others when more do.
fn who_gives_the_target(sources: usize) -> &'static str {
    if sources == 1 {
        "the other says"
    } else {
        "the others say"
    }
}

fn parts_matrix(base: &str) -> Doc {
    let calculator = format!("{base}/tools/parts-matrix");
    let sources = source_links(&PARTS);
    let sources: Vec<(&str, String)> = sources
        .iter()
        .map(|(text, address)| (text.as_str(), address.clone()))
        .collect();
    let usual_markup = margin_to_markup(PARTS.usual)
        .map(|markup| format!(" A margin of {} takes a markup of {markup}.", PARTS.usual))
        .unwrap_or_default();
    Doc {
        title: "How to build a parts matrix".to_owned(),
        intro: "A parts matrix is a table that says how much to add to a part's cost to get its selling price, with a different percent for each range of cost. This page explains how to build one and how to check that it makes the margin the shop needs. It names no matrix as the right one.".to_owned(),
        sections: vec![
            section(
                "Markup and margin are not the same number",
                &[
                    "Markup is the profit on a part as a percent of what the shop paid for it. Margin, or gross profit, is the same profit as a percent of what the customer paid. A part that costs 40.00 and sells for 50.00 has a profit of 10.00: a markup of 25% and a margin of 20%.",
                    "The two are confused because both are called the percent on parts. A matrix is usually written in markup, because that is what is applied to a cost. A target is usually given in margin, because that is what shows on a profit and loss statement. The margin is always the smaller number, and the gap grows as the percent does.",
                    "To turn one into the other: margin is markup divided by one plus markup, and markup is margin divided by one minus margin, with both written as fractions.",
                ],
            ),
            section(
                "Why cheap parts carry a higher markup",
                &[
                    "Handling a part costs about the same whatever its price. Someone looks it up, orders it, receives it, checks it, and returns it if it is wrong. On a part that costs a few dollars, a flat percent does not pay for that work. On a part that costs hundreds, the same percent can price the shop out of the job.",
                    "So a matrix slides: a high markup on the cheapest parts, falling in steps as the cost rises. The steps are the rows of the matrix. How steep the slide should be depends on what the shop sells: a shop that sells many cheap parts makes most of its parts profit in the first rows.",
                ],
            ),
            section(
                "Check the price against the list price",
                &[
                    "A matrix works from the shop's cost and knows nothing about the price a customer can find elsewhere. Before a matrix is used, compare what it gives with the part's list price for a sample of common parts. Where the matrix price is well above list, the row's markup is too high for that range of cost or that kind of part. Many shops cap the price at list, or keep a second, flatter matrix for dealer parts and tires.",
                ],
            ),
            section(
                "Test the matrix on last month's invoices",
                &[
                    "The margin a matrix makes is not the margin of any one row. It depends on how the shop's parts spend is spread over the rows. Take last month's parts invoices, add up what was spent in each range of cost, and work out each range's share of the total.",
                    "With those shares, the blended margin is total profit divided by total sales across the rows. That is the number to compare with the target, and it is what the share column of the calculator shows. If it is low, raise the rows where most of the spend is, not the rows that look low.",
                ],
            ),
            with_links(
                section(
                    "What to aim for",
                    &[
                        format!(
                            "For a general repair shop, the typical range for gross profit on parts is {} to {}, and the usual target is {}.{usual_markup} A tire shop or a heavy-duty shop runs different numbers.",
                            PARTS.range.low, PARTS.range.high, PARTS.usual
                        )
                        .as_str(),
                        format!(
                            "The range is what the first page below calls typical, and the target is what {} shops aim for. They are not a survey of what shops earn, and this site does not say what any shop should charge.",
                            who_gives_the_target(PARTS.usual_sources.len())
                        )
                        .as_str(),
                    ],
                ),
                &sources,
            ),
            section(
                "Check your own rules",
                &[
                    "Some places regulate how charges are presented to a customer: what an estimate must show, and whether a part's price must be stated apart from labor. A matrix sets a price, not how it is shown. Check the rules where the shop is.",
                ],
            ),
            with_links(
                section(
                    "Try one",
                    &[
                        "The calculator prices a part under a matrix of up to eight rows and shows the blended margin on a mix of parts spend. Its examples are illustrations, not recommendations.",
                    ],
                ),
                &[("Parts markup matrix calculator", calculator)],
            ),
        ],
    }
}

/// What a guide is about, which decides where it is listed and which
/// entry of the header it is under.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Topic {
    /// Vehicle identification numbers: under "VIN guide".
    Vin,
    /// Running a shop: under "Tools", beside the calculators.
    Shop,
}

/// One answer page.
pub struct Guide {
    /// The last part of the page's address.
    pub slug: &'static str,
    /// What the page is called in a list and in a search result.
    pub title: &'static str,
    pub description: &'static str,
    pub topic: Topic,
    doc: fn(&str) -> Doc,
}

impl Guide {
    /// The page's text, with links on the public address `base`.
    pub fn doc(&self, base: &str) -> Doc {
        (self.doc)(base)
    }

    pub fn path(&self) -> String {
        format!("/guides/{}", self.slug)
    }

    /// Whether the guide is one of the answers about VINs.
    pub fn about_vins(&self) -> bool {
        self.topic == Topic::Vin
    }
}

pub static GUIDES: [Guide; 6] = [
    Guide {
        slug: "how-to-read-a-vin",
        title: "How to read a VIN, position by position",
        description: "What each of the 17 characters of a VIN means: the manufacturer, the vehicle, the check digit, the model year, the plant and the serial number.",
        topic: Topic::Vin,
        doc: how_to_read,
    },
    Guide {
        slug: "where-to-find-the-vin",
        title: "Where to find the VIN on a vehicle",
        description: "The places a VIN is printed or stamped on a car, truck, motorcycle or trailer, where it is on the papers, and how to read it without mistakes.",
        topic: Topic::Vin,
        doc: where_to_find,
    },
    Guide {
        slug: "wmi",
        title: "What the first three characters of a VIN mean",
        description: "The first three characters of a VIN are the World Manufacturer Identifier. What each one says, and how small manufacturers use six characters.",
        topic: Topic::Vin,
        doc: wmi,
    },
    Guide {
        slug: "model-year",
        title: "VIN model year chart: the 10th character",
        description: "The tenth character of a VIN is the model year. A chart of every code from 1980 to 2039, and how the seventh character tells 1993 from 2023.",
        topic: Topic::Vin,
        doc: model_year,
    },
    Guide {
        slug: "check-digit",
        title: "VIN check digit: how position 9 is worked out",
        description: "The ninth character of a VIN is a check digit worked out from the other sixteen. The weights, the letter values and a worked example.",
        topic: Topic::Vin,
        doc: check_digit,
    },
    Guide {
        slug: "parts-matrix",
        title: "How to build a parts matrix",
        description: "How to build a parts matrix for an auto repair shop: markup against margin, why cheap parts carry more markup, and how to test it on your own invoices.",
        topic: Topic::Shop,
        doc: parts_matrix,
    },
];

const INDEX_TITLE: &str = "VIN guide: how to read, find and check a VIN";
const INDEX_DESCRIPTION: &str = "Short answers about vehicle identification numbers: what each position means, where the VIN is on a vehicle, the model year chart and the check digit.";

/// The list of the guides.
pub fn index(base: &str) -> Doc {
    let links_of = |vins: bool| -> Vec<(&'static str, String)> {
        GUIDES
            .iter()
            .filter(|guide| guide.about_vins() == vins)
            .map(|guide| (guide.title, format!("{base}{}", guide.path())))
            .collect()
    };
    let (links, shop) = (links_of(true), links_of(false));
    Doc {
        title: "VIN guide".to_owned(),
        intro: "Short answers to what people ask about vehicle identification numbers. Each page says what this site's decoder does.".to_owned(),
        sections: vec![
            with_links(
                section(
                    "The pages",
                    &["Every page has a Markdown version at the same address with .md added."],
                ),
                &links,
            ),
            with_links(
                section(
                    "For shop owners",
                    &["A guide that goes with the shop calculators."],
                ),
                &shop,
            ),
            with_links(
                section(
                    "Decode one",
                    &["Type a VIN into the box on the first page to see every field this site has for it."],
                ),
                &[("Decode a VIN", format!("{base}/"))],
            ),
        ],
    }
}

fn show(
    state: &AppState,
    path: &str,
    title: &str,
    description: &str,
    doc: Doc,
    crumbs: Vec<(String, String)>,
    format: Format,
) -> Response {
    if format == Format::Markdown {
        let canonical = format!("{}{path}", state.config().base_url);
        return markdown::response(markdown::doc(&doc), &canonical);
    }
    // A guide about running a shop is under Tools, and shows no vehicle
    // data.
    let shop = crumbs.iter().any(|(_, address)| address == "/tools");
    let page = Page::new(state, seo::title(title), description)
        .indexed(state, path)
        .with_markdown(&format!("{path}.md"))
        .in_section(if shop { "tools" } else { "guides" })
        .under(crumbs)
        .as_article();
    let page = if shop {
        page.without_vehicle_data()
    } else {
        page
    };
    let base = &state.config().base_url;
    // The list of guides is a list. Each guide is an article.
    let page = if page.crumbs.is_empty() {
        page
    } else {
        let things = vec![
            page.trail(&doc.title),
            jsonld::article(base, path, &doc.title, description),
            jsonld::organization(base),
        ];
        page.describing(things)
    };
    render(page, doc)
}

fn show_index(state: &AppState, format: Format) -> Response {
    show(
        state,
        "/guides",
        INDEX_TITLE,
        INDEX_DESCRIPTION,
        index(&state.config().base_url),
        Vec::new(),
        format,
    )
}

pub async fn index_html(State(state): State<AppState>) -> Response {
    show_index(&state, Format::Html)
}

pub async fn index_md(State(state): State<AppState>) -> Response {
    show_index(&state, Format::Markdown)
}

pub async fn guide(
    State(state): State<AppState>,
    path: Result<Path<String>, PathRejection>,
) -> Response {
    let Ok(Path(segment)) = path else {
        return site::not_found(&state);
    };
    let (slug, format) = markdown::split(&segment);
    let Some(guide) = GUIDES.iter().find(|guide| guide.slug == slug) else {
        return site::not_found(&state);
    };
    let crumb = if guide.about_vins() {
        ("VIN guide".to_owned(), "/guides".to_owned())
    } else {
        ("Tools".to_owned(), "/tools".to_owned())
    };
    show(
        &state,
        &guide.path(),
        guide.title,
        guide.description,
        guide.doc(&state.config().base_url),
        vec![crumb],
        format,
    )
}

#[cfg(test)]
mod tests {
    use wenmar_vin::{Vin, check_digit, model_year};

    use super::*;

    #[test]
    fn the_parts_guide_says_the_other_or_the_others_by_how_many_sources_give_the_target() {
        assert_eq!(who_gives_the_target(1), "the other says");
        assert_eq!(who_gives_the_target(2), "the others say");
        let text = markdown::doc(&parts_matrix("https://open.example"));
        let sentence = if PARTS.usual_sources.len() == 1 {
            "the target is what the other says shops aim for"
        } else {
            "the target is what the others say shops aim for"
        };
        assert!(text.contains(sentence), "{sentence}");
    }

    /// A VIN with `year` in position 10 and `seventh` in position 7.
    fn vin_with(year: char, seventh: char) -> Vin {
        let mut text: Vec<char> = "1HGCM82633A004352".chars().collect();
        text[6] = seventh;
        text[9] = year;
        Vin::parse(&text.into_iter().collect::<String>()).unwrap()
    }

    #[test]
    fn the_year_chart_is_what_the_decoder_reads() {
        assert_eq!(YEAR_CODES.chars().count(), 30);
        for (code, index) in YEAR_CODES.chars().zip(0u16..) {
            // A digit in position 7 is the earlier cycle, a letter the later.
            // The year 2039 is given as today, so nothing is too far ahead.
            assert_eq!(
                model_year::candidates(&vin_with(code, '2'), 2039, true).first(),
                Some(&(1980 + index)),
                "{code}"
            );
            assert_eq!(
                model_year::candidates(&vin_with(code, 'A'), 2039, true).first(),
                Some(&(2010 + index)),
                "{code}"
            );
        }
        // The characters the chart leaves out are not years.
        for code in ['U', 'Z', '0'] {
            assert!(model_year::candidates(&vin_with(code, '2'), 2039, true).is_empty());
        }
        // The two examples on the page.
        let kona = Vin::parse(EXAMPLE).unwrap();
        assert_eq!(
            model_year::candidates(&kona, 2026, true).first(),
            Some(&2023)
        );
        let accord = Vin::parse("1HGCM82633A004352").unwrap();
        assert_eq!(
            model_year::candidates(&accord, 2026, true).first(),
            Some(&2003)
        );
        // "Never more than two years ahead": W is 1998, and 2028 only once
        // the calendar reaches 2026.
        assert_eq!(
            model_year::candidates(&vin_with('W', 'A'), 2025, true),
            [1998]
        );
        assert_eq!(
            model_year::candidates(&vin_with('W', 'A'), 2026, true).first(),
            Some(&2028)
        );
    }

    #[test]
    fn the_check_digit_tables_are_what_the_decoder_uses() {
        for text in [
            EXAMPLE,
            "1HGCM82633A004352",
            "1M8GDM9AXKP042788",
            "ABCDEFGHJKLMNPRST",
            "UVWXYZ12345678901",
            "11111111111111111",
        ] {
            let vin = Vin::parse(text).unwrap();
            assert_eq!(
                check_digit_of(text),
                check_digit::check(&vin).expected,
                "{text}"
            );
        }
        // Every letter a VIN can hold has a value, and no other letter has.
        let valued: String = LETTER_VALUES.iter().map(|(_, letters)| *letters).collect();
        assert_eq!(valued.len(), 23);
        for letter in ['I', 'O', 'Q'] {
            assert!(!valued.contains(letter));
        }
    }

    #[test]
    fn the_worked_example_is_true() {
        let vin = Vin::parse(EXAMPLE).unwrap();
        let checked = check_digit::check(&vin);
        assert!(checked.valid, "the page says the check passes");
        let text = worked_example();
        assert!(
            text.contains("which add up to 257. 257 divided by 11 is 23 with 4 left over"),
            "{text}"
        );
        assert!(text.ends_with("Position 9 of KM8K2CAB4PU001140 is 4, so the check passes."));
    }

    #[test]
    fn every_guide_has_its_own_address_title_and_description() {
        for (index, guide) in GUIDES.iter().enumerate() {
            assert!(wenmar_vehicles::text::is_slug(guide.slug), "{}", guide.slug);
            assert!(guide.title.len() <= 46, "{}", guide.title);
            assert!(
                (100..=160).contains(&guide.description.len()),
                "{}: {}",
                guide.slug,
                guide.description.len()
            );
            for other in &GUIDES[index + 1..] {
                assert_ne!(guide.slug, other.slug);
                assert_ne!(guide.title, other.title);
                assert_ne!(guide.description, other.description);
            }
            // No em dashes, no arrows: the fonts are Latin subsets, and the
            // voice is plain.
            let text = markdown::doc(&guide.doc("https://open.example"));
            assert!(text.is_ascii(), "{}", guide.slug);
        }
    }
}
