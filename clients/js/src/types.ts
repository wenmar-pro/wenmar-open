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
export type Model = Schemas["Model"];
export type Submodel = Schemas["Submodel"];
export type EngineOption = Schemas["EngineOption"];
export type Meta = Schemas["MetaResponse"];
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
