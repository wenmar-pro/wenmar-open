//! The datasets this site serves and the licence of each.
//!
//! The list is written once, here. `/data`, `/data.md` and `llms-full.txt`
//! show it, and `NOTICE.md` in the repository holds the same rows: a test
//! below fails when they differ.

use crate::site::markdown::Table;

/// One dataset, and what its licence asks of anyone who reuses it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Licence {
    /// What the data is, in a few words.
    pub dataset: &'static str,
    /// Who publishes it, and where.
    pub source: &'static str,
    /// The licence, as a reader should see it.
    pub licence: &'static str,
    /// The licence as the shop file's `datasets` value names it:
    /// `public-domain`, `ogl-canada`, `cc-by-4.0` or `mit`.
    pub licence_id: &'static str,
    /// What must be said by anyone who republishes the data.
    pub attribution: &'static str,
}

pub const CAPTION: &str = "Datasets and their licences";
pub const HEAD: [&str; 4] = ["Dataset", "Source", "Licence", "Required attribution"];
/// The line in `NOTICE.md` that ends the table there.
pub const MARKER: &str = "<!-- Shop-data plans add one row per dataset above this line, and the same row in crates/open-server/src/site/licences.rs. -->";

pub const LICENCES: &[Licence] = &[
    Licence {
        dataset: "VIN decoding data and vehicle catalog",
        source: "NHTSA vPIC, the Product Information Catalog and Vehicle Listing, https://vpic.nhtsa.dot.gov/",
        licence: "Public domain (United States government work)",
        licence_id: "public-domain",
        attribution: "None required. This site credits NHTSA.",
    },
    Licence {
        dataset: "Popular makes, aliases, trim spellings and added trims",
        source: "Written by hand for this project: data/catalog/makes.yaml, data/catalog/names.yaml and data/presets.yaml",
        licence: "MIT",
        licence_id: "mit",
        attribution: "Keep the copyright notice in LICENSE.",
    },
    Licence {
        dataset: "Canadian sales tax and tire fee rates",
        source: "Government and stewardship pages, each named on the calculator page, collected in data/rates/canada.toml",
        licence: "MIT for the collection. A rate is a fact.",
        licence_id: "mit",
        attribution: "Keep the copyright notice in LICENSE.",
    },
    // ---- Shop-data plans add one row per dataset below this line. ----
    // In the order of the roadmap's dataset ids: recalls_us, recalls_ca,
    // investigations, recall_reports, ratings, bulletins, complaints, codes,
    // fuel_us, fuel_ca, specs, oil_grade, fluids, tires, manuals, jobs, labor,
    // vin_guides.
    // The same row, cell for cell, goes above the marker in NOTICE.md.
    Licence {
        dataset: "Vehicle recall campaigns, United States",
        source: "NHTSA recall flat files, https://static.nhtsa.gov/odi/ffdd/rcl/",
        licence: "Public domain (United States government work)",
        licence_id: "public-domain",
        attribution: "None required. This site credits NHTSA.",
    },
    Licence {
        dataset: "Vehicle recall campaigns, Canada",
        source: "Transport Canada, Vehicle Recalls Database, https://open.canada.ca/data/en/dataset/1ec92326-47ef-4110-b7ca-959fab03f96d",
        licence: "Open Government Licence – Canada",
        licence_id: "ogl-canada",
        attribution: "Contains information licensed under the Open Government Licence – Canada.",
    },
    Licence {
        dataset: "Vehicle defect investigations, United States",
        source: "NHTSA investigations flat file, https://static.nhtsa.gov/odi/ffdd/inv/",
        licence: "Public domain (United States government work)",
        licence_id: "public-domain",
        attribution: "None required. This site credits NHTSA.",
    },
    Licence {
        dataset: "Recall remedy progress, United States",
        source: "NHTSA recall quarterly and annual report flat files, https://static.nhtsa.gov/odi/ffdd/rcl/",
        licence: "Public domain (United States government work)",
        licence_id: "public-domain",
        attribution: "None required. This site credits NHTSA.",
    },
    Licence {
        dataset: "Crash test ratings, United States",
        source: "NHTSA New Car Assessment Program, Safercar data file, https://static.nhtsa.gov/nhtsa/downloads/Safercar/Safercar_data.csv",
        licence: "Public domain (United States government work)",
        licence_id: "public-domain",
        attribution: "None required. This site credits NHTSA.",
    },
    Licence {
        dataset: "Manufacturer communications: service bulletins, service campaigns, warranty extensions and over-the-air updates. The index and summaries NHTSA publishes; hosted service only",
        source: "NHTSA manufacturer communications flat files, https://static.nhtsa.gov/odi/ffdd/tsbs/",
        licence: "Public domain (United States government work). The documents belong to the manufacturers and are not held here.",
        licence_id: "public-domain",
        attribution: "None required. This site credits NHTSA.",
    },
    Licence {
        dataset: "Owner complaints to NHTSA, counts by vehicle and component only",
        source: "NHTSA consumer complaints flat file, https://static.nhtsa.gov/odi/ffdd/cmpl/",
        licence: "Public domain (United States government work)",
        licence_id: "public-domain",
        attribution: "None required. This site credits NHTSA.",
    },
    Licence {
        dataset: "Trouble code descriptions, generic OBD-II codes only",
        source: "Written by hand for this project, in its own words: data/codes/. A code number is a fact; no text is taken from SAE J2012 or any other list",
        licence: "MIT",
        licence_id: "mit",
        attribution: "Keep the copyright notice in LICENSE.",
    },
    Licence {
        dataset: "Job list: the repair jobs a labour time can be about",
        source: "Written by hand for this project, in its own words: data/jobs.yaml",
        licence: "MIT",
        licence_id: "mit",
        attribution: "Keep the copyright notice in LICENSE.",
    },
    Licence {
        dataset: "Labour estimates: the hours a repair job takes",
        source: "Wenmar Pro. Written by people who do the work, in data/labor/, and later measured from repair orders of shops that opt in. Never taken from a labour guide",
        licence: "Creative Commons Attribution 4.0 International (CC BY 4.0)",
        licence_id: "cc-by-4.0",
        attribution: "Labour estimates by Wenmar Pro, CC BY 4.0",
    },
    Licence {
        dataset: "Index of the VIN guides manufacturers file under 49 CFR Part 565",
        source: "NHTSA vPIC, the GetParts list of manufacturer filings, https://vpic.nhtsa.dot.gov/api/",
        licence: "Public domain (United States government work) for the index. The letters it links to belong to the manufacturers and are not copied.",
        licence_id: "public-domain",
        attribution: "None required. This site credits NHTSA and links only to its copies.",
    },
    Licence {
        dataset: "Fuel consumption ratings, United States",
        source: "EPA fuel economy data, https://www.fueleconomy.gov/feg/download.shtml",
        licence: "Public domain (United States government work)",
        licence_id: "public-domain",
        attribution: "None required. This site credits EPA.",
    },
    Licence {
        dataset: "Fuel consumption ratings, Canada",
        source: "Natural Resources Canada, Fuel consumption ratings, https://open.canada.ca/data/en/dataset/98f1a129-f628-4ce4-b24d-6f16bf24dd64",
        licence: "Open Government Licence – Canada",
        licence_id: "ogl-canada",
        attribution: "Contains information licensed under the Open Government Licence – Canada.",
    },
    Licence {
        dataset: "Vehicle specifications and emission standards",
        source: "EPA fuel economy data, emissions file and Fuel Economy Guide files, https://www.fueleconomy.gov/feg/download.shtml",
        licence: "Public domain (United States government work)",
        licence_id: "public-domain",
        attribution: "None required. This site credits EPA.",
    },
    Licence {
        dataset: "Engine oil viscosity grade, as certified for fuel economy",
        source: "EPA Fuel Economy Guide files, https://www.fueleconomy.gov/feg/download.shtml",
        licence: "Public domain (United States government work)",
        licence_id: "public-domain",
        attribution: "None required. This site credits EPA.",
    },
    Licence {
        dataset: "Fluid specifications and capacities, read by hand",
        source: "Owner manuals and under-hood labels of the makers, each named in its record, collected in data/fluids/",
        licence: "MIT for the collection. A figure is a fact.",
        licence_id: "mit",
        attribution: "Keep the copyright notice in LICENSE.",
    },
    Licence {
        dataset: "Original-equipment tire sizes, pressures and wheel nut torque, read by hand",
        source: "Door placards and owner manuals of the makers, each named in its record, collected in data/tires/",
        licence: "MIT for the collection. A figure is a fact.",
        licence_id: "mit",
        attribution: "Keep the copyright notice in LICENSE.",
    },
    Licence {
        dataset: "Links to owner manuals and maintenance schedules on the makers sites",
        source: "Found and opened by a person, each with the day it was checked, collected in data/fluids/ and data/catalog/makes.yaml",
        licence: "MIT for the collection. An address is a fact. The documents belong to the makers and are not copied.",
        licence_id: "mit",
        attribution: "Keep the copyright notice in LICENSE.",
    },
];

/// The row as `NOTICE.md` and the Markdown page write it.
pub fn markdown_row(licence: &Licence) -> String {
    format!(
        "| {} | {} | {} | {} |",
        licence.dataset, licence.source, licence.licence, licence.attribution
    )
}

/// The list as a table for a page of prose.
pub fn table() -> Table {
    let cells = |licence: &Licence| -> Vec<String> {
        [
            licence.dataset,
            licence.source,
            licence.licence,
            licence.attribution,
        ]
        .iter()
        .map(|cell| (*cell).to_owned())
        .collect()
    };
    Table {
        caption: CAPTION.to_owned(),
        head: HEAD.iter().map(|cell| (*cell).to_owned()).collect(),
        rows: LICENCES.iter().map(cells).collect(),
        prose: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_first_rows_are_the_data_served_today() {
        let datasets: Vec<&str> = LICENCES
            .iter()
            .take(3)
            .map(|licence| licence.dataset)
            .collect();
        assert_eq!(
            datasets,
            [
                "VIN decoding data and vehicle catalog",
                "Popular makes, aliases, trim spellings and added trims",
                "Canadian sales tax and tire fee rates"
            ]
        );
        assert_eq!(
            LICENCES.first().map(|licence| licence.licence_id),
            Some("public-domain")
        );
        assert_eq!(table().rows.len(), LICENCES.len());
    }

    #[test]
    fn every_cell_is_plain_text_and_every_licence_is_one_the_shop_file_names() {
        for licence in LICENCES {
            for cell in [
                licence.dataset,
                licence.source,
                licence.licence,
                licence.attribution,
            ] {
                assert!(!cell.trim().is_empty() && cell.trim() == cell, "{cell:?}");
                assert!(!cell.contains(['|', '\n', '\'', '&', '<', '>']), "{cell:?}");
            }
            assert!(
                ["public-domain", "ogl-canada", "cc-by-4.0", "mit"].contains(&licence.licence_id),
                "{}",
                licence.licence_id
            );
        }
        let mut names: Vec<&str> = LICENCES.iter().map(|licence| licence.dataset).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), LICENCES.len(), "a dataset is listed twice");
    }

    #[test]
    fn notice_md_holds_the_same_table() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../NOTICE.md");
        let notice = std::fs::read_to_string(&path).unwrap();
        let heading = format!("## {CAPTION}\n");
        let section = notice
            .split(heading.as_str())
            .nth(1)
            .unwrap_or_else(|| panic!("NOTICE.md has no section {CAPTION:?}"));
        // From the header row to the marker: the header, the rule, one line
        // for each row in order, then the marker, and nothing between them.
        let mut expected = format!("| {} |\n| --- | --- | --- | --- |\n", HEAD.join(" | "));
        for licence in LICENCES {
            expected.push_str(&markdown_row(licence));
            expected.push('\n');
        }
        expected.push_str(MARKER);
        let table = section.find("| Dataset").map_or("", |at| &section[at..]);
        assert!(
            table.starts_with(&expected),
            "NOTICE.md and licences.rs differ.\nlicences.rs gives:\n{expected}\nNOTICE.md has:\n{table:.1500}"
        );
    }
}
