import type { components, operations } from "./schema.js";

type Schemas = components["schemas"];

/** A decoded VIN. A field that could not be determined is left out. */
export type VinDecode = Schemas["VinDecode"];
export type CheckDigit = Schemas["CheckDigit"];
export type Engine = Schemas["Engine"];
export type Safety = Schemas["Safety"];
export type Manufacturer = Schemas["Manufacturer"];
export type Plant = Schemas["Plant"];
export type Warning = Schemas["Warning"];
/** The catalog entry a decoded VIN reaches. */
export type Selection = Schemas["Selection"];
/** One vehicle of the catalog. */
export type Entry = Schemas["Entry"];
export type Make = Schemas["Make"];
/** One make with the maker's own links. */
export type MakeDetail = Schemas["MakeDetail"];
export type Model = Schemas["Model"];
export type Submodel = Schemas["Submodel"];
export type EngineOption = Schemas["EngineOption"];
export type Meta = Schemas["MetaResponse"];
/** Recalls that may apply to one model year. Never a lookup by VIN. */
export type VehicleRecalls = Schemas["VehicleRecalls"];
/** Recalls that may apply to the model year a VIN decodes to. */
export type VinRecalls = Schemas["VinRecalls"];
/** One recall campaign with every vehicle it lists. */
export type RecallCampaign = Schemas["RecallCampaign"];
/** The manufacturer's newest report of a campaign's remedy progress: totals for the whole campaign and a named period, never for one vehicle. */
export type RecallCompletion = Schemas["RecallCompletion"];
/** One page of recall campaigns, newest first. */
export type RecallList = Schemas["RecallList"];
/** One recall campaign in a list. */
export type Recall = Schemas["Recall"];
/** Where one vehicle can be checked for recalls by VIN. */
export type RecallLookup = Schemas["RecallLookup"];
/** Manufacturer communications that list one model year, one page. */
export type VehicleBulletins = Schemas["VehicleBulletins"];
/** Manufacturer communications for the model year a VIN decodes to. */
export type VinBulletins = Schemas["VinBulletins"];
/** One manufacturer communication with every vehicle it lists. */
export type BulletinDetail = Schemas["BulletinDetail"];
/** One page of manufacturer communications, newest first. */
export type BulletinList = Schemas["BulletinList"];
/** One manufacturer communication in a list. */
export type Bulletin = Schemas["Bulletin"];
/** A model year's bulletins counted by type, component, code and topic. */
export type BulletinCounts = Schemas["BulletinCounts"];
/** Where NHTSA's copy of a bulletin's document is expected to be. */
export type BulletinDocument = Schemas["BulletinDocument"];
/** Fuel consumption ratings of one model year, from EPA and Natural Resources Canada. */
export type VehicleFuel = Schemas["VehicleFuel"];
/** Fuel consumption ratings for the vehicle a VIN decodes to. */
export type VinFuel = Schemas["VinFuel"];
/** One rating. `confidence` says how far it was matched: `model`, `engine` or `submodel`. */
export type FuelRating = Schemas["FuelRating"];
/** Specifications of one model year from EPA's files. */
export type VehicleSpecs = Schemas["VehicleSpecs"];
/** Specifications for the vehicle a VIN decodes to. */
export type VinSpecs = Schemas["VinSpecs"];
/** The specifications that come with one EPA rating. */
export type Spec = Schemas["Spec"];
/** The fluids of one model year: EPA's oil grade, and the records read by hand. */
export type VehicleFluids = Schemas["VehicleFluids"];
/** The fluids for the vehicle a VIN decodes to. */
export type VinFluids = Schemas["VinFluids"];
/** The oil grade a maker certified an engine with, per EPA. Not from the owner's manual. */
export type EngineOil = Schemas["EngineOil"];
/** One fluid a person read from the maker's own document, with its source and the day it was checked. */
export type FluidRecord = Schemas["FluidRecord"];
/** Every original-equipment tire fitment on record for one model year. */
export type VehicleTires = Schemas["VehicleTires"];
/** The tire fitments of the model year a VIN decodes to. */
export type VinTires = Schemas["VinTires"];
/** One tire size the maker fitted, with the trims it covers, pressures and wheel nut torque. */
export type TireFitment = Schemas["TireFitment"];
/** What a parts counter asks for about one vehicle, each line with its source. */
export type PartsCard = Schemas["PartsCard"];
/** One line of a parts-ordering card. */
export type PartsCardLine = Schemas["PartsCardLine"];
/**
 * Everything held for one model year. A block the server has no data for is
 * `null`; `recalls` counts campaigns that may apply, never a lookup by VIN.
 */
export type Overview = Schemas["Overview"];
/** The recalls of a model year, in brief. */
export type OverviewRecalls = Schemas["OverviewRecalls"];
/** The bulletins of a model year, in brief. */
export type OverviewBulletins = Schemas["OverviewBulletins"];
/** How many recall campaigns list a model year, for each country. */
export type RecallCount = Schemas["RecallCount"];
/** Every model year of one model, newest first. */
export type ModelYears = Schemas["ModelYears"];
/** Campaigns reported on or after a day, each with the vehicles it lists. */
export type RecentRecalls = Schemas["RecentRecalls"];
/** What a batch decode says of recalls when `include: "recalls"` is asked. */
export type VinRecallCount = Schemas["VinRecallCount"];
/** One NHTSA defect investigation. Not a finding that a defect exists. */
export type Investigation = Schemas["Investigation"];
/** NHTSA investigations that list one model year. */
export type VehicleInvestigations = Schemas["VehicleInvestigations"];
/** The NHTSA investigations that led to one recall campaign. */
export type RecallInvestigations = Schemas["RecallInvestigations"];

/** NHTSA's crash test ratings for one model year. */
export type VehicleSafety = Schemas["VehicleSafety"];
/** One rating: the vehicle NHTSA tested, its stars and its safety features. */
export type SafetyRating = Schemas["SafetyRating"];

/** What owners reported to NHTSA about one model year, as counts. Reports, not verified defects. */
export type VehicleComplaints = Schemas["VehicleComplaints"];

/** One generic trouble code, described in this project's own words. */
export type Code = Schemas["Code"];
/** A trouble code in a list. */
export type CodeLink = Schemas["CodeLink"];
/** One page of the described trouble codes. */
export type CodeList = Schemas["CodeList"];
/** A trouble code on one model year, with its bulletins and recalls. */
export type VehicleCode = Schemas["VehicleCode"];

/** The repair jobs a labour estimate can be about. */
export type JobList = Schemas["JobList"];
/** One repair job: what it covers and what it typically takes by class. */
export type JobDetail = Schemas["JobDetail"];
/** One labour estimate: hours, the scope it is for and its basis. */
export type LaborFigure = Schemas["LaborFigure"];
/** Labour estimates for one model year. An estimate, not a quote. */
export type VehicleLabor = Schemas["VehicleLabor"];
/** Labour estimates for the vehicle a VIN decodes to. */
export type VinLabor = Schemas["VinLabor"];

/** The deductions from one regular pay in Ontario by CRA's T4127 formulas. An estimate: the employer remains liable. */
export type PayrollDeductions = Schemas["PayrollDeductions"];
/** One T4127 factor used, with its value and meaning. */
export type PayrollFactor = Schemas["PayrollFactor"];

/** The body of every error the API sends. */
export type ErrorBody = Schemas["ErrorBody"];
/** One answer of a batch: a decode, or the error for that VIN. */
export type BatchItem = Schemas["BatchItem"];
/**
 * A decode narrowed to the `fields` asked for, in the same nesting. `vin` is
 * always kept.
 */
export type NarrowedDecode = { vin: string } & Record<string, unknown>;
/** One answer of a batch asked with `fields`: a narrowed decode, or the error for that VIN, whole. */
export type NarrowedBatchItem = NarrowedDecode | ErrorBody;

type Query<Name extends keyof operations> = NonNullable<operations[Name]["parameters"]["query"]>;

export type YearsQuery = Query<"years">;
export type MakesQuery = Query<"makes">;
export type ModelsQuery = Query<"models">;
export type SubmodelsQuery = Query<"submodels">;
export type EnginesQuery = Query<"engines">;
export type SearchQuery = Query<"search">;
export type VehicleRecallsQuery = Query<"vehicle_recalls">;
export type VinRecallsQuery = Query<"vin_recalls">;
export type VehicleFuelQuery = Query<"vehicle_fuel">;
export type VinFuelQuery = Query<"vin_fuel">;
export type VinYearQuery = Query<"vin_specs">;
export type RecallSearchQuery = Query<"recall_search">;
export type BulletinFilterQuery = Query<"vehicle_bulletins">;
export type VinBulletinsQuery = Query<"vin_bulletins">;
export type BulletinSearchQuery = Query<"bulletin_search">;
export type CodeSearchQuery = Query<"code_search">;
export type PayrollDeductionsQuery = Query<"payroll_deductions">;
export type VehicleInvestigationsQuery = Query<"vehicle_investigations">;
export type RecentRecallsQuery = Query<"recent_recalls">;
