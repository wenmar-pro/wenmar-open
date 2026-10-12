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
/** One page of recall campaigns, newest first. */
export type RecallList = Schemas["RecallList"];
/** One recall campaign in a list. */
export type Recall = Schemas["Recall"];
/** Where one vehicle can be checked for recalls by VIN. */
export type RecallLookup = Schemas["RecallLookup"];
/**
 * Everything held for one model year. A block the server has no data for is
 * `null`; `recalls` counts campaigns that may apply, never a lookup by VIN.
 */
export type Overview = Schemas["Overview"];
/** The recalls of a model year, in brief. */
export type OverviewRecalls = Schemas["OverviewRecalls"];
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

/** The body of every error the API sends. */
export type ErrorBody = Schemas["ErrorBody"];
/** One answer of a batch: a decode, or the error for that VIN. */
export type BatchItem = Schemas["BatchItem"];

type Query<Name extends keyof operations> = NonNullable<operations[Name]["parameters"]["query"]>;

export type YearsQuery = Query<"years">;
export type MakesQuery = Query<"makes">;
export type ModelsQuery = Query<"models">;
export type SubmodelsQuery = Query<"submodels">;
export type EnginesQuery = Query<"engines">;
export type SearchQuery = Query<"search">;
export type VehicleRecallsQuery = Query<"vehicle_recalls">;
export type VinRecallsQuery = Query<"vin_recalls">;
export type RecallSearchQuery = Query<"recall_search">;
export type VehicleInvestigationsQuery = Query<"vehicle_investigations">;
export type RecentRecallsQuery = Query<"recent_recalls">;
