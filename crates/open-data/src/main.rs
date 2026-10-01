use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};
use open_data::build::{BuildInfo, build};
use open_data::catalog::curated::Curated;
use open_data::{catalog_parity, fetch, inspect, parity};
use wenmar_vehicles::sqlite::SqliteSource;
use wenmar_vehicles::{Catalog, Scope};
use wenmar_vin::sqlite::SqliteData;
use wenmar_vin::{DecodeOptions, Decoder};

/// Builds the Wenmar Open data file from NHTSA's vPIC release.
#[derive(Parser)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Download a vPIC plain-text dump and extract it.
    Fetch {
        /// Release such as 2026_09. Defaults to the newest on NHTSA's page.
        #[arg(long)]
        release: Option<String>,
        /// Where to put the files.
        #[arg(long, default_value = "data/build")]
        dir: PathBuf,
    },
    /// Build a data file from an extracted dump.
    Build {
        /// The extracted .sql file.
        #[arg(long)]
        dump: PathBuf,
        /// The data file to create.
        #[arg(long)]
        out: PathBuf,
        /// Data version such as 2026.09. Defaults to the one in the dump's file name.
        #[arg(long)]
        data_version: Option<String>,
        /// Directory holding catalog/makes.yaml, catalog/names.yaml and presets.yaml.
        #[arg(long, default_value = "data")]
        curated: PathBuf,
    },
    /// Decode one VIN against a data file and print the JSON.
    Decode {
        /// The data file.
        #[arg(long)]
        data: PathBuf,
        vin: String,
    },
    /// Compare decodes with NHTSA's recorded answers and check the baseline.
    Parity {
        /// The data file.
        #[arg(long)]
        data: PathBuf,
        /// Directory holding nhtsa.json and baseline.json.
        #[arg(long, default_value = "data/corpus")]
        corpus: PathBuf,
        /// Write the result as the new baseline instead of checking it.
        #[arg(long)]
        update_baseline: bool,
    },
    /// Print the newest vPIC release on NHTSA's downloads page, such as 2026_09.
    Latest,
    /// Look something up in a data file's vehicle catalog and print JSON.
    Catalog {
        /// The data file.
        #[arg(long)]
        data: PathBuf,
        /// light (cars, MPVs and trucks), all, or a vPIC vehicle type id.
        #[arg(long, default_value = "light")]
        scope: String,
        #[command(subcommand)]
        query: inspect::Query,
    },
    /// Compare the catalog's models with NHTSA's recorded answers and check the baseline.
    CatalogParity {
        /// The data file.
        #[arg(long)]
        data: PathBuf,
        /// Directory holding nhtsa-models.json and baseline.json.
        #[arg(long, default_value = "data/catalog")]
        fixtures: PathBuf,
        /// Write the result as the new baseline instead of checking it.
        #[arg(long)]
        update_baseline: bool,
    },
}

/// The current UTC time as `YYYY-MM-DD HH:MM:SS`.
fn now() -> String {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs());
    let (days, rest) = (seconds / 86_400, seconds % 86_400);
    // Civil date from a day count (Howard Hinnant's algorithm).
    let shifted = days + 719_468;
    let era = shifted / 146_097;
    let day_of_era = shifted % 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_index = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_index + 2) / 5 + 1;
    let month = if month_index < 10 {
        month_index + 3
    } else {
        month_index - 9
    };
    let year = year_of_era + era * 400 + u64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02} {:02}:{:02}:{:02}",
        rest / 3_600,
        rest % 3_600 / 60,
        rest % 60
    )
}

fn main() -> Result<()> {
    match Cli::parse().command {
        Command::Fetch { release, dir } => {
            let release = match release {
                Some(release) => release,
                None => fetch::latest_release(&fetch::downloads_page()?)
                    .context("no plain-text release found on NHTSA's downloads page")?,
            };
            let path = fetch::download(&release, &dir)?;
            println!("{}", path.display());
        }
        Command::Build {
            dump,
            out,
            data_version,
            curated,
        } => {
            let stem = dump
                .file_stem()
                .and_then(|stem| stem.to_str())
                .unwrap_or_default()
                .to_owned();
            let release = stem.strip_prefix("vPICList_lite_").unwrap_or_default();
            let Some(data_version) = data_version.or_else(|| fetch::data_version(release)) else {
                bail!(
                    "cannot tell the data version from {}; pass --data-version",
                    dump.display()
                );
            };
            let info = BuildInfo {
                data_version,
                vpic_release: stem,
                built_at: now(),
            };
            let curated = Curated::load(&curated)?;
            let reader = BufReader::new(
                File::open(&dump).with_context(|| format!("opening {}", dump.display()))?,
            );
            let summary = build(reader, &out, &info, &curated)?;
            println!(
                "{}: {} manufacturer codes, {} schema links, {} patterns, {} specification rows, {} engine-model rows",
                out.display(),
                summary.manufacturers,
                summary.schema_links,
                summary.patterns,
                summary.spec_rows,
                summary.engine_rows
            );
            println!(
                "catalog: {} makes, {} models, {} model-years ({} cars, MPVs and trucks), {} details, {} submodels, {} engines, {} combinations",
                summary.catalog.makes,
                summary.catalog.models,
                summary.catalog.vehicles,
                summary.catalog.light_vehicles,
                summary.catalog.details,
                summary.catalog.submodels,
                summary.catalog.engines,
                summary.catalog.cells
            );
            if summary.catalog.capped != 0 {
                eprintln!(
                    "warning: {} models had too many combinations and got no submodels or engines",
                    summary.catalog.capped
                );
            }
            for entry in &summary.catalog.unmatched {
                eprintln!("warning: nothing in vPIC matches {entry}");
            }
        }
        Command::Decode { data, vin } => {
            let source = SqliteData::open(&data).map_err(|error| anyhow::anyhow!("{error}"))?;
            match Decoder::new(source).decode(&vin, DecodeOptions::default()) {
                Ok(decoded) => println!("{}", serde_json::to_string_pretty(&decoded)?),
                Err(error) => bail!("{error}"),
            }
        }
        Command::Parity {
            data,
            corpus,
            update_baseline,
        } => {
            let fixtures_path = corpus.join("nhtsa.json");
            let baseline_path = corpus.join("baseline.json");
            let fixtures: parity::Fixtures = serde_json::from_reader(BufReader::new(
                File::open(&fixtures_path)
                    .with_context(|| format!("opening {}", fixtures_path.display()))?,
            ))
            .with_context(|| format!("reading {}", fixtures_path.display()))?;
            let source = SqliteData::open(&data).map_err(|error| anyhow::anyhow!("{error}"))?;
            let report = parity::run(source, &fixtures);
            print!("{}", parity::table(&report));
            if update_baseline {
                let mut text = serde_json::to_string_pretty(&report)?;
                text.push('\n');
                std::fs::write(&baseline_path, text)
                    .with_context(|| format!("writing {}", baseline_path.display()))?;
                println!("baseline written to {}", baseline_path.display());
            } else {
                let baseline: parity::Report = serde_json::from_reader(BufReader::new(
                    File::open(&baseline_path)
                        .with_context(|| format!("opening {}", baseline_path.display()))?,
                ))
                .with_context(|| format!("reading {}", baseline_path.display()))?;
                let problems = parity::regressions(&report, &baseline);
                if !problems.is_empty() {
                    for problem in &problems {
                        eprintln!("regression: {problem}");
                    }
                    bail!("agreement with NHTSA fell below the baseline");
                }
                println!("agreement with NHTSA is at or above the baseline");
            }
        }
        Command::Latest => {
            let release = fetch::latest_release(&fetch::downloads_page()?)
                .context("no plain-text release found on NHTSA's downloads page")?;
            println!("{release}");
        }
        Command::Catalog { data, scope, query } => {
            let scope = Scope::parse(&scope).with_context(|| {
                format!("{scope} is not a scope; use light, all or a vehicle type id")
            })?;
            println!(
                "{}",
                serde_json::to_string_pretty(&inspect::run(&data, scope, &query)?)?
            );
        }
        Command::CatalogParity {
            data,
            fixtures,
            update_baseline,
        } => {
            let fixtures_path = fixtures.join("nhtsa-models.json");
            let baseline_path = fixtures.join("baseline.json");
            let recorded: catalog_parity::Fixtures = serde_json::from_reader(BufReader::new(
                File::open(&fixtures_path)
                    .with_context(|| format!("opening {}", fixtures_path.display()))?,
            ))
            .with_context(|| format!("reading {}", fixtures_path.display()))?;
            let source = SqliteSource::open(&data).map_err(|error| anyhow::anyhow!("{error}"))?;
            let catalog = Catalog::new(source)?;
            let report = catalog_parity::run(&catalog, &recorded)?;
            print!("{}", catalog_parity::table(&report));
            if update_baseline {
                let mut text = serde_json::to_string_pretty(&report)?;
                text.push('\n');
                std::fs::write(&baseline_path, text)
                    .with_context(|| format!("writing {}", baseline_path.display()))?;
                println!("baseline written to {}", baseline_path.display());
            } else {
                let baseline: catalog_parity::Report = serde_json::from_reader(BufReader::new(
                    File::open(&baseline_path)
                        .with_context(|| format!("opening {}", baseline_path.display()))?,
                ))
                .with_context(|| format!("reading {}", baseline_path.display()))?;
                let open_from = recorded.open_from();
                let newer = catalog_parity::newer(&report, &baseline, open_from);
                if let Some(note) = catalog_parity::newer_note(&newer, open_from) {
                    println!("{note}");
                }
                let problems = catalog_parity::regressions(&report, &baseline, open_from);
                if !problems.is_empty() {
                    for problem in &problems {
                        eprintln!("regression: {problem}");
                    }
                    bail!("the catalog's agreement with NHTSA fell below the baseline");
                }
                println!("the catalog agrees with NHTSA at or above the baseline");
            }
        }
    }
    Ok(())
}
