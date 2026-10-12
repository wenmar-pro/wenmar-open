use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};
use open_data::build::{BuildInfo, build};
use open_data::catalog::curated::Curated;
use open_data::shop::build::{ShopBuildInfo, ShopInputs, build_shop, open_catalog};
use open_data::shop::coverage;
use open_data::shop::nhtsa_vin_guides;
use open_data::shop::{epa_guide, nrcan_fuel};
use open_data::{catalog_parity, fetch, inspect, parity, sources};
use wenmar_shopdata::ShopData;
use wenmar_shopdata::sqlite::ShopSource;
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
    /// Download a source's files. With no source: a vPIC plain-text dump, extracted.
    Fetch {
        /// vpic (the default), nhtsa-recalls, tc-recalls, nhtsa-investigations, nhtsa-recall-reports, nhtsa-ratings, nhtsa-vin-guides, epa-vehicles, epa-guide, epa-emissions or nrcan-fuel.
        source: Option<String>,
        /// vpic only. Release such as 2026_09. Defaults to the newest on NHTSA's page.
        #[arg(long)]
        release: Option<String>,
        /// The build directory. vPIC goes in it; other sources go in sources/<source>/ under it.
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
    /// Build a shop file: the datasets matched against a data file's catalog.
    BuildShop {
        /// The core data file whose catalog the datasets are matched against.
        #[arg(long)]
        catalog: PathBuf,
        /// The shop file to create.
        #[arg(long)]
        out: PathBuf,
        /// Data version such as 2026.10.12. Defaults to today's date, UTC.
        #[arg(long)]
        data_version: Option<String>,
        /// Directory holding the curated files, such as shop/models.yaml.
        #[arg(long, default_value = "data")]
        curated: PathBuf,
        /// The fetched nhtsa-recalls directory. Adds the recalls_us dataset.
        #[arg(long)]
        nhtsa_recalls: Option<PathBuf>,
        /// The fetched tc-recalls directory. Adds the recalls_ca dataset.
        #[arg(long)]
        tc_recalls: Option<PathBuf>,
        /// The fetched nhtsa-investigations directory. Adds the investigations dataset.
        #[arg(long)]
        nhtsa_investigations: Option<PathBuf>,
        /// The fetched nhtsa-recall-reports directory. Adds the recall_reports dataset.
        #[arg(long)]
        nhtsa_recall_reports: Option<PathBuf>,
        /// The fetched nhtsa-ratings directory. Adds the ratings dataset.
        #[arg(long)]
        nhtsa_ratings: Option<PathBuf>,
        /// The fetched nhtsa-vin-guides directory. Adds the vin_guides dataset.
        #[arg(long)]
        nhtsa_vin_guides: Option<PathBuf>,
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
    /// Report how much of a shop file matched the catalog, and check the minimums.
    Coverage {
        /// The shop file.
        #[arg(long)]
        shop: PathBuf,
        /// The core data file the shop file was built against.
        #[arg(long)]
        catalog: PathBuf,
        /// The file of minimum matched shares.
        #[arg(long, default_value = "data/coverage.yaml")]
        thresholds: PathBuf,
        /// Record the measured shares as the new minimums instead of checking them.
        #[arg(long)]
        write: bool,
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
        Command::Fetch {
            source,
            release,
            dir,
        } => match source.as_deref().unwrap_or("vpic") {
            "vpic" => {
                let release = match release {
                    Some(release) => release,
                    None => fetch::latest_release(&fetch::downloads_page()?)
                        .context("no plain-text release found on NHTSA's downloads page")?,
                };
                let path = fetch::download(&release, &dir)?;
                println!("{}", path.display());
            }
            nhtsa_vin_guides::SOURCE_ID => {
                if release.is_some() {
                    bail!("--release applies to vpic only");
                }
                // Not a fixed list of files: the index is asked for one
                // range of dates at a time.
                for path in nhtsa_vin_guides::fetch(&dir, &now())? {
                    println!("{}", path.display());
                }
            }
            id => {
                if release.is_some() {
                    bail!("--release applies to vpic only");
                }
                // A source in this match has to find out which files there are.
                let paths = match id {
                    epa_guide::SOURCE => epa_guide::fetch(&dir, &now())?,
                    nrcan_fuel::SOURCE => nrcan_fuel::fetch(&dir, &now())?,
                    _ => {
                        let Some(source) = sources::find(id) else {
                            bail!(
                                "{id} is not a source; use vpic, {}, {}, {}, {}",
                                sources::ids().join(", "),
                                nhtsa_vin_guides::SOURCE_ID,
                                epa_guide::SOURCE,
                                nrcan_fuel::SOURCE
                            );
                        };
                        sources::fetch(source, &dir, &now())?
                    }
                };
                for path in paths {
                    println!("{}", path.display());
                }
            }
        },
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
        Command::BuildShop {
            catalog,
            out,
            data_version,
            curated,
            nhtsa_recalls,
            tc_recalls,
            nhtsa_investigations,
            nhtsa_recall_reports,
            nhtsa_ratings,
            nhtsa_vin_guides,
        } => {
            let built_at = now();
            let data_version = data_version.unwrap_or_else(|| {
                built_at
                    .chars()
                    .take(10)
                    .collect::<String>()
                    .replace('-', ".")
            });
            let info = ShopBuildInfo {
                data_version,
                built_at,
            };
            let inputs = ShopInputs {
                nhtsa_recalls,
                tc_recalls,
                nhtsa_investigations,
                nhtsa_recall_reports,
                nhtsa_ratings,
                nhtsa_vin_guides,
            };
            let summary = build_shop(&catalog, &out, &curated, &info, &inputs)?;
            println!(
                "{}: shop data {}, matched against data {}, {} datasets",
                out.display(),
                summary.data_version,
                summary.catalog_data_version,
                summary.datasets.len()
            );
            for (dataset, entry) in summary.datasets.iter() {
                println!("{dataset}: {} rows, as of {}", entry.rows, entry.as_of);
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
        Command::Coverage {
            shop,
            catalog,
            thresholds,
            write,
        } => {
            let (catalog, core_version) = open_catalog(&catalog)?;
            let source = ShopSource::open(&shop)
                .map_err(|error| anyhow::anyhow!("opening {}: {error}", shop.display()))?;
            let shop = ShopData::new(source)?;
            shop.check_catalog(&core_version)?;
            let report = coverage::run(&shop, &coverage::light_makes(&catalog)?, coverage::TABLES)?;
            print!("{}", coverage::table(&report));
            if write {
                std::fs::write(&thresholds, coverage::yaml(&report))
                    .with_context(|| format!("writing {}", thresholds.display()))?;
                println!("minimums written to {}", thresholds.display());
            } else {
                let minimums = coverage::Thresholds::load(&thresholds)?;
                let problems = coverage::failures(&report, &minimums);
                if !problems.is_empty() {
                    for problem in &problems {
                        eprintln!("below: {problem}");
                    }
                    bail!(
                        "matching fell below the minimums in {}",
                        thresholds.display()
                    );
                }
                println!("matching is at or above every minimum");
            }
        }
    }
    Ok(())
}
