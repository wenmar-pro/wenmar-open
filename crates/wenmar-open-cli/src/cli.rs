//! The command tree.

use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};

use crate::request::{Action, Lookup, Request};
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
    /// The local data file, for answers without a connection.
    Data {
        #[command(subcommand)]
        command: DataCommand,
    },
    /// Run an MCP server on standard input and output, for AI agents.
    ///
    /// It offers two tools, wenmar_vin and wenmar_vehicles, the same as the
    /// hosted server at /mcp.
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
    },
    /// Say whether there is a data file, which version it is, and where.
    Status,
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
