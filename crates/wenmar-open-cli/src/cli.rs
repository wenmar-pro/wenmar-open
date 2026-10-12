//! The command tree.

use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};

use crate::request::{Action, Lookup, RecallAction, RecallLookup, Request};
use crate::setup::Agent;

/// Free vehicle data for auto repair shops: decode a VIN, or look a vehicle
/// up by year, make, model, submodel and engine.
///
/// Answers come from a local data file when there is one (see `wenmar-open
/// data pull`), and from the hosted API otherwise. Nothing needs a key or
/// an account.
#[derive(Debug, Parser)]
#[command(name = "wenmar-open", version, propagate_version = true)]
pub struct Cli {
    #[command(flatten)]
    pub global: Global,
    #[command(subcommand)]
    pub command: Option<Command>,
}

/// Options every command takes, before or after the command's name.
#[derive(Debug, Clone, Default, Args)]
pub struct Global {
    /// Print JSON even at a terminal. JSON is what is printed anyway when
    /// the output is piped.
    #[arg(long, global = true)]
    pub json: bool,
    /// Filter the JSON with a jq expression, such as `.make` or `.[].id`.
    /// Text results are printed without quotes.
    #[arg(long, global = true, value_name = "EXPR")]
    pub jq: Option<String>,
    /// The base URL of the hosted API, used when there is no local data
    /// file. Also WENMAR_OPEN_API. The default is https://open.wenmarpro.com.
    #[arg(long, global = true, value_name = "URL")]
    pub api: Option<String>,
    /// Answer only from the local data file, and fail if there is none.
    #[arg(long, global = true, conflicts_with = "online")]
    pub offline: bool,
    /// Answer only from the hosted API, even if there is a local data file.
    #[arg(long, global = true)]
    pub online: bool,
    /// The directory holding the data file. Also WENMAR_OPEN_DATA_DIR.
    #[arg(long, global = true, value_name = "DIR")]
    pub data_dir: Option<PathBuf>,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Decode vehicle identification numbers.
    Vin {
        #[command(subcommand)]
        command: VinCommand,
    },
    /// Look vehicles up by year, make, model, submodel and engine.
    Vehicles {
        #[command(subcommand)]
        command: VehiclesCommand,
    },

    /// Recall campaigns from NHTSA and Transport Canada that may apply to a
    /// vehicle.
    ///
    /// A campaign lists a year, a make and a model. Nothing here says a
    /// recall applies to one vehicle: confirm by VIN with the manufacturer.
    /// Offline, the recalls come from the shop data file (see `wenmar-open
    /// data pull --shop`).
    Recalls {
        #[command(subcommand)]
        command: RecallsCommand,
    },
    /// The local data file, for answers without a connection.
    Data {
        #[command(subcommand)]
        command: DataCommand,
    },
    /// Run an MCP server on standard input and output, for AI agents.
    ///
    /// It offers three tools, wenmar_vin, wenmar_vehicles and wenmar_recalls,
    /// the same as the hosted server at /mcp.
    Mcp,
    /// Set up a coding agent to use this tool.
    ///
    /// Writes a skill file that describes the commands, and prints the
    /// command that registers the MCP server. With --yes it runs that
    /// command too.
    Setup {
        /// The agent to set up.
        #[arg(value_enum)]
        agent: Agent,
        /// The directory the agent reads skills from. The default is
        /// ~/.claude/skills for claude and ~/.agents/skills for codex.
        #[arg(long, value_name = "DIR")]
        dir: Option<PathBuf>,
        /// Replace a skill file that has been edited by hand.
        #[arg(long)]
        force: bool,
        /// Also run the agent's command that registers the MCP server.
        #[arg(long)]
        yes: bool,
    },
    /// Check the data file and the API, and say which of them answers.
    Doctor,
    /// Filter JSON from standard input with a jq expression.
    ///
    /// This is what `--jq` runs, in a process of its own: an expression
    /// that calls itself without end uses up the stack, which ends the
    /// process it runs in, and this way that is not the one that answers.
    #[command(hide = true)]
    Jq {
        /// The jq expression.
        #[arg(allow_hyphen_values = true)]
        expression: String,
    },
}

#[derive(Debug, Subcommand)]
pub enum DataCommand {
    /// Download the published data file and put it in place.
    ///
    /// The file is about 30 MB to download and 120 MB on disk. It replaces
    /// the one in place only once it has arrived whole and been checked.
    Pull {
        /// The data version to download, such as 2026.09. The default is
        /// the newest.
        #[arg(value_name = "VERSION")]
        data_version: Option<String>,
        /// Download even if that version is already in place.
        #[arg(long)]
        force: bool,
        /// Where the releases are listed. Also WENMAR_OPEN_RELEASES.
        #[arg(long, value_name = "URL")]
        releases: Option<String>,
        /// Download the shop data file instead: the recalls, matched to
        /// the data file already in place. It is published weekly, is
        /// about 20 MB to download and 80 MB on disk, and VERSION is then
        /// a day, such as 2026.10.12.
        #[arg(long)]
        shop: bool,
    },
    /// Say whether there is a data file, which version it is, and where.
    Status {
        /// Say the same of the shop data file, which holds the recalls.
        #[arg(long)]
        shop: bool,
    },
}

#[derive(Debug, Subcommand)]
pub enum VinCommand {
    /// Decode one VIN.
    ///
    /// Spaces and dashes are ignored and letters may be in either case. A
    /// wrong check digit is not an error: the result has `valid: false` and
    /// a warning.
    Decode {
        /// The 17-character VIN.
        vin: String,
        /// Use this model year instead of the one worked out from the VIN.
        #[arg(long)]
        year: Option<u16>,
    },
}

/// `--scope`, for the commands that take one.
#[derive(Debug, Clone, Default, Args)]
pub struct ScopeArg {
    /// light (cars, MPVs and trucks), all, or a vPIC vehicle type id such
    /// as 6 for trailers.
    #[arg(long, value_name = "SCOPE")]
    pub scope: Option<String>,
}

#[derive(Debug, Subcommand)]
pub enum VehiclesCommand {
    /// Model years, newest first.
    Years {
        #[command(flatten)]
        scope: ScopeArg,
        /// Only years that start with this, such as 201.
        #[arg(long)]
        term: Option<String>,
    },
    /// Makes, popular ones first, then by name.
    Makes {
        /// Only makes with a model in this model year.
        #[arg(long)]
        year: Option<u16>,
        #[command(flatten)]
        scope: ScopeArg,
        /// Only makes whose name or alias starts with this.
        #[arg(long)]
        term: Option<String>,
        /// At most this many, up to 500.
        #[arg(long)]
        limit: Option<usize>,
    },
    /// Models of a make, by name.
    Models {
        /// A make's name, alias or id: Chevrolet, chevy or chevrolet.
        #[arg(long)]
        make: String,
        /// Only models of this model year.
        #[arg(long)]
        year: Option<u16>,
        #[command(flatten)]
        scope: ScopeArg,
        /// Only models whose name starts with this.
        #[arg(long)]
        term: Option<String>,
        /// At most this many, up to 500.
        #[arg(long)]
        limit: Option<usize>,
    },
    /// Trims of a model year, or its series where it has no trims.
    #[command(visible_alias = "trims")]
    Submodels {
        #[arg(long)]
        make: String,
        /// A model's name or id: F-150, f150 or f-150.
        #[arg(long)]
        model: String,
        #[arg(long)]
        year: u16,
        /// Only submodels whose name starts with this.
        #[arg(long)]
        term: Option<String>,
    },
    /// Engines of a model year, in short form such as 3.5L Turbo V6.
    Engines {
        #[arg(long)]
        make: String,
        #[arg(long)]
        model: String,
        #[arg(long)]
        year: u16,
        /// Only engines this submodel comes with.
        #[arg(long)]
        submodel: Option<String>,
        /// Only engines whose label starts with this.
        #[arg(long)]
        term: Option<String>,
    },
    /// Catalog entries for free text, best first: 2019 civic si
    ///
    /// The local data file reads the text as a year, a make, a model and a
    /// submodel, in that order. The hosted API also finds a model from its
    /// own words in any order, such as `type r` or `hd 2500`. So a search
    /// that finds nothing from the data file may still find something
    /// with --online.
    Search {
        /// What someone would type. Several words need no quotes.
        #[arg(required = true, value_name = "TEXT")]
        text: Vec<String>,
        #[command(flatten)]
        scope: ScopeArg,
        /// At most this many, up to 50. The default is 10.
        #[arg(long)]
        limit: Option<usize>,
    },
    /// One catalog entry by its id, such as 2019_honda_civic_si.
    Entry {
        /// The vehicle id.
        id: String,
    },
}

/// `--country`, for the recall commands that take one.
#[derive(Debug, Clone, Default, Args)]
pub struct CountryArg {
    /// us (NHTSA) or ca (Transport Canada). The default is both.
    #[arg(long, value_name = "COUNTRY")]
    pub country: Option<String>,
}

#[derive(Debug, Subcommand)]
pub enum RecallsCommand {
    /// The recalls that may apply to a model year, newest first.
    ///
    /// The list is for the year, make and model, not for one vehicle.
    Vehicle {
        /// The vehicle id, such as 2018_ford_f-150.
        id: String,
        #[command(flatten)]
        country: CountryArg,
    },
    /// The recalls that may apply to the model year a VIN decodes to.
    ///
    /// The VIN only names the year, make and model. Whether a recall
    /// applies to that one vehicle is for the manufacturer to say: the
    /// answer's `lookup` gives the official addresses.
    Vin {
        /// The 17-character VIN.
        vin: String,
        /// Use this model year instead of the one worked out from the VIN.
        #[arg(long)]
        year: Option<u16>,
        #[command(flatten)]
        country: CountryArg,
    },
    /// One recall campaign with every vehicle it lists.
    Campaign {
        /// Who published it: nhtsa or tc (Transport Canada).
        source: String,
        /// The publisher's campaign number, such as 19V888000.
        campaign: String,
    },
    /// Recall campaigns, newest first, narrowed by what is given.
    Search {
        /// Words to look for in the campaign. Several words need no quotes.
        #[arg(value_name = "TEXT")]
        text: Vec<String>,
        /// Only campaigns that list a vehicle of this make.
        #[arg(long)]
        make: Option<String>,
        /// Only campaigns that list this model. Needs --make.
        #[arg(long)]
        model: Option<String>,
        /// Only campaigns that list this model year.
        #[arg(long)]
        year: Option<u16>,
        /// Only campaigns whose component text contains this.
        #[arg(long)]
        component: Option<String>,
        #[command(flatten)]
        country: CountryArg,
        /// At most this many, up to 100. The default is 25.
        #[arg(long)]
        limit: Option<usize>,
        /// `next_cursor` of the page before, to get the next page.
        #[arg(long)]
        cursor: Option<String>,
    },
}

impl RecallsCommand {
    pub fn request(self) -> Request {
        let (action, lookup) = match self {
            RecallsCommand::Vehicle { id, country } => (
                RecallAction::Vehicle,
                RecallLookup {
                    id: Some(id),
                    country: country.country,
                    ..RecallLookup::default()
                },
            ),
            RecallsCommand::Vin { vin, year, country } => (
                RecallAction::Vin,
                RecallLookup {
                    vin: Some(vin),
                    year,
                    country: country.country,
                    ..RecallLookup::default()
                },
            ),
            RecallsCommand::Campaign { source, campaign } => (
                RecallAction::Campaign,
                RecallLookup {
                    source: Some(source),
                    campaign: Some(campaign),
                    ..RecallLookup::default()
                },
            ),
            RecallsCommand::Search {
                text,
                make,
                model,
                year,
                component,
                country,
                limit,
                cursor,
            } => (
                RecallAction::Search,
                RecallLookup {
                    query: Some(text.join(" ")),
                    make,
                    model,
                    year,
                    component,
                    country: country.country,
                    limit,
                    cursor,
                    ..RecallLookup::default()
                },
            ),
        };
        Request::Recalls { action, lookup }
    }
}

impl VinCommand {
    pub fn request(self) -> Request {
        match self {
            VinCommand::Decode { vin, year } => Request::VinDecode { vin, year },
        }
    }
}

impl VehiclesCommand {
    pub fn request(self) -> Request {
        let (action, lookup) = match self {
            VehiclesCommand::Years { scope, term } => (
                Action::Years,
                Lookup {
                    scope: scope.scope,
                    term,
                    ..Lookup::default()
                },
            ),
            VehiclesCommand::Makes {
                year,
                scope,
                term,
                limit,
            } => (
                Action::Makes,
                Lookup {
                    year,
                    scope: scope.scope,
                    term,
                    limit,
                    ..Lookup::default()
                },
            ),
            VehiclesCommand::Models {
                make,
                year,
                scope,
                term,
                limit,
            } => (
                Action::Models,
                Lookup {
                    make: Some(make),
                    year,
                    scope: scope.scope,
                    term,
                    limit,
                    ..Lookup::default()
                },
            ),
            VehiclesCommand::Submodels {
                make,
                model,
                year,
                term,
            } => (
                Action::Submodels,
                Lookup {
                    make: Some(make),
                    model: Some(model),
                    year: Some(year),
                    term,
                    ..Lookup::default()
                },
            ),
            VehiclesCommand::Engines {
                make,
                model,
                year,
                submodel,
                term,
            } => (
                Action::Engines,
                Lookup {
                    make: Some(make),
                    model: Some(model),
                    year: Some(year),
                    submodel,
                    term,
                    ..Lookup::default()
                },
            ),
            VehiclesCommand::Search { text, scope, limit } => (
                Action::Search,
                Lookup {
                    query: Some(text.join(" ")),
                    scope: scope.scope,
                    limit,
                    ..Lookup::default()
                },
            ),
            VehiclesCommand::Entry { id } => (
                Action::Entry,
                Lookup {
                    id: Some(id),
                    ..Lookup::default()
                },
            ),
        };
        Request::Vehicles { action, lookup }
    }
}

#[cfg(test)]
mod tests {
    use clap::CommandFactory;

    use super::*;

    #[test]
    fn the_command_tree_is_well_formed() {
        Cli::command().debug_assert();
    }

    #[test]
    fn several_search_words_need_no_quotes() {
        let cli = Cli::try_parse_from(["wenmar-open", "vehicles", "search", "2019", "civic", "si"])
            .unwrap();
        let Some(Command::Vehicles { command }) = cli.command else {
            panic!("a vehicles command");
        };
        let Request::Vehicles { action, lookup } = command.request() else {
            panic!("a vehicles request");
        };
        assert_eq!(action, Action::Search);
        assert_eq!(lookup.query.as_deref(), Some("2019 civic si"));
    }

    #[test]
    fn trims_is_another_name_for_submodels() {
        for name in ["submodels", "trims"] {
            let cli = Cli::try_parse_from([
                "wenmar-open",
                "vehicles",
                name,
                "--make",
                "honda",
                "--model",
                "civic",
                "--year",
                "2019",
            ])
            .unwrap();
            assert!(matches!(
                cli.command,
                Some(Command::Vehicles {
                    command: VehiclesCommand::Submodels { .. }
                })
            ));
        }
    }

    #[test]
    fn the_recall_commands_become_recall_requests() {
        let request = |args: &[&str]| {
            let cli = Cli::try_parse_from(args).unwrap();
            let Some(Command::Recalls { command }) = cli.command else {
                panic!("a recalls command");
            };
            let Request::Recalls { action, lookup } = command.request() else {
                panic!("a recalls request");
            };
            (action, lookup)
        };
        let (action, lookup) = request(&[
            "wenmar-open",
            "recalls",
            "vehicle",
            "2018_ford_f-150",
            "--country",
            "ca",
        ]);
        assert_eq!(action, RecallAction::Vehicle);
        assert_eq!(lookup.id.as_deref(), Some("2018_ford_f-150"));
        assert_eq!(lookup.country.as_deref(), Some("ca"));

        let (action, lookup) = request(&[
            "wenmar-open",
            "recalls",
            "vin",
            "KM8K2CAB4PU001140",
            "--year",
            "2023",
        ]);
        assert_eq!(action, RecallAction::Vin);
        assert_eq!(lookup.vin.as_deref(), Some("KM8K2CAB4PU001140"));
        assert_eq!(lookup.year, Some(2023));

        let (action, lookup) =
            request(&["wenmar-open", "recalls", "campaign", "nhtsa", "19V888000"]);
        assert_eq!(action, RecallAction::Campaign);
        assert_eq!(lookup.source.as_deref(), Some("nhtsa"));
        assert_eq!(lookup.campaign.as_deref(), Some("19V888000"));

        let (action, lookup) = request(&[
            "wenmar-open",
            "recalls",
            "search",
            "wiring",
            "harness",
            "--make",
            "hyundai",
            "--limit",
            "5",
        ]);
        assert_eq!(action, RecallAction::Search);
        assert_eq!(lookup.query.as_deref(), Some("wiring harness"));
        assert_eq!(lookup.make.as_deref(), Some("hyundai"));
        assert_eq!(lookup.limit, Some(5));
        // A search needs no words: it is then the list of every campaign.
        let (_, lookup) = request(&["wenmar-open", "recalls", "search"]);
        assert_eq!(lookup.query.as_deref(), Some(""));
    }

    #[test]
    fn the_data_commands_take_shop() {
        for args in [
            ["wenmar-open", "data", "pull", "--shop"],
            ["wenmar-open", "data", "status", "--shop"],
        ] {
            let cli = Cli::try_parse_from(args).unwrap();
            assert!(matches!(
                cli.command,
                Some(Command::Data {
                    command: DataCommand::Pull { shop: true, .. }
                        | DataCommand::Status { shop: true }
                })
            ));
        }
    }

    #[test]
    fn global_options_are_taken_before_or_after_the_command() {
        for args in [
            ["wenmar-open", "--data-dir", "/d", "vehicles", "years"],
            ["wenmar-open", "vehicles", "years", "--data-dir", "/d"],
        ] {
            let cli = Cli::try_parse_from(args).unwrap();
            assert_eq!(cli.global.data_dir, Some(PathBuf::from("/d")));
        }
    }
}
