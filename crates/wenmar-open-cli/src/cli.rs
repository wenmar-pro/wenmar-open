//! The command tree.

use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};

use crate::request::{
    Action, BulletinAction, BulletinLookup, Lookup, RecallAction, RecallLookup, Request, Topic,
};
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
    /// Manufacturer service bulletins, warranty extensions and software
    /// updates that NHTSA publishes. They are served by the hosted API
    /// only: the local shop data file does not hold them.
    Bulletins {
        #[command(subcommand)]
        command: BulletinsCommand,
    },
    /// The local data file, for answers without a connection.
    Data {
        #[command(subcommand)]
        command: DataCommand,
    },
    /// Run an MCP server on standard input and output, for AI agents.
    ///
    /// It offers four tools, wenmar_vin, wenmar_vehicles, wenmar_recalls and
    /// wenmar_bulletins, the same as the hosted server at /mcp.
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
    /// Everything held for the model year a VIN decodes to, in one answer.
    ///
    /// What the VIN decoded to in brief, a count of the recall campaigns
    /// that may apply to that year, make and model with the three newest,
    /// and links. A block with no data is `null`. The VIN only names the
    /// model year: nothing here says a recall applies to one vehicle.
    Overview {
        /// The 17-character VIN.
        vin: String,
        /// Use this model year instead of the one worked out from the VIN.
        #[arg(long)]
        year: Option<u16>,
    },
    /// Fuel consumption ratings for the model year a VIN decodes to.
    ///
    /// From EPA and Natural Resources Canada. When the decode settles the
    /// engine, only the ratings matched to that engine are listed.
    /// Offline, they come from the shop data file (see `wenmar-open data
    /// pull --shop`).
    Fuel {
        /// The 17-character VIN.
        vin: String,
        /// Use this model year instead of the one worked out from the VIN.
        #[arg(long)]
        year: Option<u16>,
        /// us (EPA) or ca (Natural Resources Canada). The default is both.
        #[arg(long, value_name = "COUNTRY")]
        country: Option<String>,
    },
    /// Specifications from EPA's files for the model year a VIN decodes to.
    Specs {
        /// The 17-character VIN.
        vin: String,
        /// Use this model year instead of the one worked out from the VIN.
        #[arg(long)]
        year: Option<u16>,
    },
    /// Fluids for the model year a VIN decodes to.
    ///
    /// The engine oil grade EPA records, and the records a person read
    /// from the maker's own documents, each with its source and the day
    /// it was checked. Nothing is estimated.
    Fluids {
        /// The 17-character VIN.
        vin: String,
        /// Use this model year instead of the one worked out from the VIN.
        #[arg(long)]
        year: Option<u16>,
    },
    /// Original-equipment tire fitments of the model year a VIN decodes to.
    ///
    /// Every size on record: a VIN does not say which wheels a vehicle was
    /// built with.
    Tires {
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
    /// NHTSA's crash test ratings for a model year.
    ///
    /// NHTSA rates a vehicle by body style and drive, so a model year
    /// often has several ratings, and their stars can differ: each says
    /// which vehicle it is for. Offline, the ratings come from the shop
    /// data file (see `wenmar-open data pull --shop`).
    Safety {
        /// The vehicle id, such as 2018_ford_f-150.
        id: String,
    },
    /// What owners reported to NHTSA about a model year, as counts by component. Reports, not verified defects.
    Complaints {
        /// The vehicle id, such as 2016_honda_civic.
        id: String,
    },
    /// Fuel consumption ratings of a model year.
    ///
    /// From EPA and Natural Resources Canada, in L/100 km and miles per US
    /// gallon. Each says how far it was matched: to the model year, to
    /// one engine, or to an engine and a trim. Offline, they come from the
    /// shop data file (see `wenmar-open data pull --shop`).
    Fuel {
        /// The vehicle id, such as 2018_ford_f-150.
        id: String,
        /// us (EPA) or ca (Natural Resources Canada). The default is both.
        #[arg(long, value_name = "COUNTRY")]
        country: Option<String>,
    },
    /// Specifications of a model year from EPA's files.
    Specs {
        /// The vehicle id, such as 2018_ford_f-150.
        id: String,
    },
    /// Fluids of a model year.
    ///
    /// The engine oil grade EPA records for each engine, and the records
    /// a person read from the maker's own documents, each with its source
    /// and the day it was checked. An empty list means nobody has
    /// recorded it: nothing is estimated.
    Fluids {
        /// The vehicle id, such as 2018_ford_f-150.
        id: String,
    },
    /// Original-equipment tire fitments of a model year.
    ///
    /// Every size on record, with the trims it covers. None is picked.
    Tires {
        /// The vehicle id, such as 2018_ford_f-150.
        id: String,
    },
    /// Everything held for one model year, in one answer.
    ///
    /// A count of the recall campaigns that may apply, with the three
    /// newest, and links. A block with no data is `null`. Offline it
    /// answers with no shop data file too: `recalls` is then `null`.
    Overview {
        /// The vehicle id, such as 2018_ford_f-150.
        id: String,
    },
    /// Every model year of one model, newest first, each with its vehicle
    /// id and the number of recall campaigns that may apply to it.
    ModelYears {
        /// A make's name, alias or id: Chevrolet, chevy or chevrolet.
        #[arg(long)]
        make: String,
        /// A model's name or id: F-150, f150 or f-150.
        #[arg(long)]
        model: String,
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
    /// Recall campaigns reported on or after a day, newest first, each
    /// with the vehicles it lists.
    ///
    /// For keeping a list of vehicles up to date: ask once a day with the
    /// day of the last answer. Each campaign may apply to the vehicles it
    /// lists; confirm by VIN with the manufacturer.
    Recent {
        /// The day, such as 2026-09-01. That day is included.
        #[arg(long, value_name = "DAY")]
        since: String,
        /// Only campaigns that list a vehicle of this make.
        #[arg(long)]
        make: Option<String>,
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

/// The filters of a bulletin list, the same in each command that takes
/// them.
#[derive(Debug, Clone, Default, Args)]
pub struct BulletinFilterArgs {
    /// Only this type: service_bulletin, service_campaign,
    /// warranty_extension, over_the_air, emissions or other.
    #[arg(long = "type", value_name = "TYPE")]
    pub kind: Option<String>,
    /// Only bulletins about this component, such as brakes or brake-pad.
    #[arg(long)]
    pub component: Option<String>,
    /// Only bulletins whose summary names this trouble code, such as P0420.
    #[arg(long)]
    pub code: Option<String>,
    /// Only bulletins with this topic. `maintenance` is the only one.
    #[arg(long)]
    pub topic: Option<String>,
    /// Only bulletins with this text in them, such as a symptom.
    #[arg(long)]
    pub query: Option<String>,
    /// At most this many, up to 100. The default is 25.
    #[arg(long)]
    pub limit: Option<usize>,
    /// `next_cursor` of the page before, to get the next page.
    #[arg(long)]
    pub cursor: Option<String>,
}

impl BulletinFilterArgs {
    fn into_lookup(self) -> BulletinLookup {
        BulletinLookup {
            kind: self.kind,
            component: self.component,
            code: self.code,
            topic: self.topic,
            query: self.query,
            limit: self.limit,
            cursor: self.cursor,
            ..BulletinLookup::default()
        }
    }
}

#[derive(Debug, Subcommand)]
pub enum BulletinsCommand {
    /// The bulletins that list a model year, newest first.
    ///
    /// The list is for the year, make and model, not for one vehicle.
    Vehicle {
        /// The vehicle id, such as 2018_ford_f-150.
        id: String,
        #[command(flatten)]
        filter: BulletinFilterArgs,
    },
    /// The bulletins that list the model year a VIN decodes to.
    ///
    /// The VIN only names the year, make and model. A bulletin is not a
    /// recall, and the list does not say which vehicles a bulletin concerns.
    Vin {
        /// The 17-character VIN.
        vin: String,
        /// Use this model year instead of the one worked out from the VIN.
        #[arg(long)]
        year: Option<u16>,
        #[command(flatten)]
        filter: BulletinFilterArgs,
    },
    /// One bulletin with every vehicle it lists.
    Get {
        /// NHTSA's id number for the communication, such as 10152456.
        nhtsa_id: u64,
    },
    /// Bulletins, newest first, narrowed by what is given.
    Search {
        /// Only bulletins that list a vehicle of this make.
        #[arg(long)]
        make: Option<String>,
        /// Only bulletins that list this model. Needs --make.
        #[arg(long)]
        model: Option<String>,
        /// Only bulletins that list this model year.
        #[arg(long)]
        year: Option<u16>,
        #[command(flatten)]
        filter: BulletinFilterArgs,
    },
}

impl BulletinsCommand {
    pub fn request(self) -> Request {
        let (action, lookup) = match self {
            BulletinsCommand::Vehicle { id, filter } => (
                BulletinAction::Vehicle,
                BulletinLookup {
                    id: Some(id),
                    ..filter.into_lookup()
                },
            ),
            BulletinsCommand::Vin { vin, year, filter } => (
                BulletinAction::Vin,
                BulletinLookup {
                    vin: Some(vin),
                    year,
                    ..filter.into_lookup()
                },
            ),
            BulletinsCommand::Get { nhtsa_id } => (
                BulletinAction::Get,
                BulletinLookup {
                    nhtsa_id: Some(nhtsa_id),
                    ..BulletinLookup::default()
                },
            ),
            BulletinsCommand::Search {
                make,
                model,
                year,
                filter,
            } => (
                BulletinAction::Search,
                BulletinLookup {
                    make,
                    model,
                    year,
                    ..filter.into_lookup()
                },
            ),
        };
        Request::Bulletins { action, lookup }
    }
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
            RecallsCommand::Recent {
                since,
                make,
                country,
                limit,
                cursor,
            } => (
                RecallAction::Recent,
                RecallLookup {
                    since: Some(since),
                    make,
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

fn vin_facts(topic: Topic, vin: String, year: Option<u16>) -> Request {
    Request::VinFacts {
        topic,
        vin,
        year,
        country: None,
    }
}

fn vehicle_facts(topic: Topic, id: String) -> Request {
    Request::VehicleFacts {
        topic,
        id,
        country: None,
    }
}

impl VinCommand {
    pub fn request(self) -> Request {
        match self {
            VinCommand::Decode { vin, year } => Request::VinDecode { vin, year },
            VinCommand::Overview { vin, year } => Request::VinOverview { vin, year },
            VinCommand::Fuel { vin, year, country } => Request::VinFacts {
                topic: Topic::Fuel,
                vin,
                year,
                country,
            },
            VinCommand::Specs { vin, year } => vin_facts(Topic::Specs, vin, year),
            VinCommand::Fluids { vin, year } => vin_facts(Topic::Fluids, vin, year),
            VinCommand::Tires { vin, year } => vin_facts(Topic::Tires, vin, year),
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
            VehiclesCommand::Fuel { id, country } => {
                return Request::VehicleFacts {
                    topic: Topic::Fuel,
                    id,
                    country,
                };
            }
            VehiclesCommand::Specs { id } => return vehicle_facts(Topic::Specs, id),
            VehiclesCommand::Fluids { id } => return vehicle_facts(Topic::Fluids, id),
            VehiclesCommand::Tires { id } => return vehicle_facts(Topic::Tires, id),
            VehiclesCommand::Safety { id } => (
                Action::Safety,
                Lookup {
                    id: Some(id),
                    ..Lookup::default()
                },
            ),
            VehiclesCommand::Complaints { id } => (
                Action::Complaints,
                Lookup {
                    id: Some(id),
                    ..Lookup::default()
                },
            ),
            VehiclesCommand::Overview { id } => (
                Action::Overview,
                Lookup {
                    id: Some(id),
                    ..Lookup::default()
                },
            ),
            VehiclesCommand::ModelYears { make, model } => (
                Action::ModelYears,
                Lookup {
                    make: Some(make),
                    model: Some(model),
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
    fn the_bulletin_commands_become_bulletin_requests() {
        let request = |args: &[&str]| {
            let cli = Cli::try_parse_from(args).unwrap();
            let Some(Command::Bulletins { command }) = cli.command else {
                panic!("a bulletins command");
            };
            let Request::Bulletins { action, lookup } = command.request() else {
                panic!("a bulletins request");
            };
            (action, lookup)
        };
        let (action, lookup) = request(&[
            "wenmar-open",
            "bulletins",
            "vehicle",
            "2018_ford_f-150",
            "--type",
            "warranty_extension",
            "--limit",
            "5",
        ]);
        assert_eq!(action, BulletinAction::Vehicle);
        assert_eq!(lookup.id.as_deref(), Some("2018_ford_f-150"));
        assert_eq!(lookup.kind.as_deref(), Some("warranty_extension"));
        assert_eq!(lookup.limit, Some(5));
        let (action, lookup) = request(&["wenmar-open", "bulletins", "get", "10152456"]);
        assert_eq!(action, BulletinAction::Get);
        assert_eq!(lookup.nhtsa_id, Some(10_152_456));
        let (action, lookup) = request(&[
            "wenmar-open",
            "bulletins",
            "search",
            "--make",
            "ford",
            "--code",
            "P0420",
        ]);
        assert_eq!(action, BulletinAction::Search);
        assert_eq!(lookup.make.as_deref(), Some("ford"));
        assert_eq!(lookup.code.as_deref(), Some("P0420"));
        let (action, lookup) = request(&[
            "wenmar-open",
            "bulletins",
            "vin",
            "KM8K2CAB4PU001140",
            "--year",
            "2023",
        ]);
        assert_eq!(action, BulletinAction::Vin);
        assert_eq!(lookup.year, Some(2023));
    }

    #[test]
    fn the_overview_commands_become_requests() {
        let request = |args: &[&str]| {
            let cli = Cli::try_parse_from(args).unwrap();
            match cli.command {
                Some(Command::Vin { command }) => command.request(),
                Some(Command::Vehicles { command }) => command.request(),
                Some(Command::Recalls { command }) => command.request(),
                Some(Command::Bulletins { command }) => command.request(),
                _ => panic!("a command that asks a question"),
            }
        };
        assert_eq!(
            request(&[
                "wenmar-open",
                "vin",
                "overview",
                "KM8K2CAB4PU001140",
                "--year",
                "2023"
            ]),
            Request::VinOverview {
                vin: "KM8K2CAB4PU001140".to_owned(),
                year: Some(2023)
            }
        );
        let Request::Vehicles { action, lookup } =
            request(&["wenmar-open", "vehicles", "overview", "2018_ford_f-150"])
        else {
            panic!("a vehicles request");
        };
        assert_eq!(action, Action::Overview);
        assert_eq!(lookup.id.as_deref(), Some("2018_ford_f-150"));
        let Request::Vehicles { action, lookup } = request(&[
            "wenmar-open",
            "vehicles",
            "model-years",
            "--make",
            "ford",
            "--model",
            "f-150",
        ]) else {
            panic!("a vehicles request");
        };
        assert_eq!(action, Action::ModelYears);
        assert_eq!(lookup.make.as_deref(), Some("ford"));
        assert_eq!(lookup.model.as_deref(), Some("f-150"));
        let Request::Recalls { action, lookup } = request(&[
            "wenmar-open",
            "recalls",
            "recent",
            "--since",
            "2026-09-01",
            "--make",
            "ford",
            "--country",
            "ca",
            "--limit",
            "5",
        ]) else {
            panic!("a recalls request");
        };
        assert_eq!(action, RecallAction::Recent);
        assert_eq!(lookup.since.as_deref(), Some("2026-09-01"));
        assert_eq!(lookup.make.as_deref(), Some("ford"));
        assert_eq!(lookup.country.as_deref(), Some("ca"));
        assert_eq!(lookup.limit, Some(5));
        // The day is required: without it the command line is refused.
        assert!(Cli::try_parse_from(["wenmar-open", "recalls", "recent"]).is_err());
    }

    #[test]
    fn the_complaints_command_becomes_a_request() {
        let cli =
            Cli::try_parse_from(["wenmar-open", "vehicles", "complaints", "2016_honda_civic"])
                .unwrap();
        let Some(Command::Vehicles { command }) = cli.command else {
            panic!("a vehicles command");
        };
        let Request::Vehicles { action, lookup } = command.request() else {
            panic!("a vehicles request");
        };
        assert_eq!(action, Action::Complaints);
        assert_eq!(lookup.id.as_deref(), Some("2016_honda_civic"));
    }

    #[test]
    fn the_safety_command_becomes_a_request() {
        let cli =
            Cli::try_parse_from(["wenmar-open", "vehicles", "safety", "2018_ford_f-150"]).unwrap();
        let Some(Command::Vehicles { command }) = cli.command else {
            panic!("a vehicles command");
        };
        let Request::Vehicles { action, lookup } = command.request() else {
            panic!("a vehicles request");
        };
        assert_eq!(action, Action::Safety);
        assert_eq!(lookup.id.as_deref(), Some("2018_ford_f-150"));
    }

    #[test]
    fn fuel_specs_fluids_and_tires_are_commands_of_vehicles_and_of_vin() {
        for topic in Topic::ALL {
            let cli =
                Cli::try_parse_from(["wenmar-open", "vehicles", topic.name(), "2018_ford_f-150"])
                    .unwrap();
            let Some(Command::Vehicles { command }) = cli.command else {
                panic!("a vehicles command");
            };
            assert_eq!(
                command.request(),
                Request::VehicleFacts {
                    topic,
                    id: "2018_ford_f-150".to_owned(),
                    country: None,
                }
            );
            let cli = Cli::try_parse_from([
                "wenmar-open",
                "vin",
                topic.name(),
                "KM8K2CAB4PU001140",
                "--year",
                "2023",
            ])
            .unwrap();
            let Some(Command::Vin { command }) = cli.command else {
                panic!("a vin command");
            };
            assert_eq!(
                command.request(),
                Request::VinFacts {
                    topic,
                    vin: "KM8K2CAB4PU001140".to_owned(),
                    year: Some(2023),
                    country: None,
                }
            );
        }
        // Only fuel takes a country.
        let cli = Cli::try_parse_from([
            "wenmar-open",
            "vehicles",
            "fuel",
            "2018_ford_f-150",
            "--country",
            "ca",
        ])
        .unwrap();
        let Some(Command::Vehicles { command }) = cli.command else {
            panic!("a vehicles command");
        };
        assert!(
            matches!(command.request(), Request::VehicleFacts { topic: Topic::Fuel, country: Some(country), .. } if country == "ca")
        );
        assert!(
            Cli::try_parse_from(["wenmar-open", "vehicles", "tires", "x", "--country", "ca"])
                .is_err()
        );
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
