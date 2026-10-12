// Generated from crates/open-server/openapi.json by scripts/generate.mjs.
// Do not edit. Run: npm run generate

export interface paths {
    "/v1/makes/{make}": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        /** One make, with links to the maker's own pages where they are recorded. */
        get: operations["make"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/v1/meta": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        /** The data version, the vPIC release it came from, and when it was built. */
        get: operations["meta"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/v1/models/{make}/{model}": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        /**
         * Every model year of one model, newest first, each with its vehicle id
         *     and the number of recall campaigns that may apply to it.
         */
        get: operations["model_years"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/v1/recalls": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        /**
         * Recall campaigns, newest first: every one, or those narrowed by make,
         *     model, year, component, free text or country.
         * @description Each campaign may apply to the vehicles it lists. A make or a model the
         *     catalog does not have gives an empty list.
         */
        get: operations["recall_search"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/v1/recalls/recent": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        /**
         * Recall campaigns reported on or after a day, newest first, each with
         *     the vehicles it lists.
         * @description For an application that keeps a list of vehicles: ask once a day with
         *     the day of the last answer, and compare `vehicles[].vehicle_id` with
         *     its own. Each campaign may apply to the vehicles it lists.
         */
        get: operations["recent_recalls"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/v1/recalls/{source}/{campaign}": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        /** One recall campaign with every vehicle it lists. */
        get: operations["recall_campaign"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/v1/recalls/{source}/{campaign}/investigations": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        /**
         * The NHTSA investigations that led to a recall campaign.
         * @description Open and closed, oldest first. Most campaigns followed no investigation
         *     and give an empty list, as every Transport Canada campaign does. The
         *     campaign number is not checked against the recall data.
         */
        get: operations["recall_investigations"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/v1/vehicles/engines": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        /** Engines of a model year, in short form, optionally for one submodel. */
        get: operations["engines"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/v1/vehicles/makes": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        /** Makes, popular ones first and the rest by name. */
        get: operations["makes"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/v1/vehicles/models": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        /** Models of a make, by name. An unknown make has no models. */
        get: operations["models"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/v1/vehicles/search": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        /**
         * Catalog entries for free text, best first. Text that names nothing gives
         *     an empty list.
         */
        get: operations["search"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/v1/vehicles/submodels": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        /**
         * Submodels of a model year: trims where NHTSA has them, else series, else
         *     this project's own list.
         */
        get: operations["submodels"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/v1/vehicles/trims": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        /** The same as `/v1/vehicles/submodels`, under the name most people use. */
        get: operations["trims"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/v1/vehicles/years": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        /** Model years, newest first. */
        get: operations["years"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/v1/vehicles/{id}": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        /** One catalog entry by its stable id. */
        get: operations["entry"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/v1/vehicles/{id}/fluids": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        /**
         * Fluids of a model year. Today: the engine oil viscosity grade.
         * @description The grade is the one the maker certified the engine with, as EPA's
         *     Fuel Economy Guide records it. It is not read from the owner's manual,
         *     and it gives no capacity and no specification. One entry per engine.
         */
        get: operations["vehicle_fluids"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/v1/vehicles/{id}/fuel": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        /**
         * Fuel consumption ratings of a model year.
         * @description EPA's ratings and Natural Resources Canada's, each matched to the
         *     model year. `confidence` says how far: `model`, `engine` or
         *     `submodel`. `engine` is the catalog engine only when the match settled
         *     one. `l_100km` and `mpg` (US gallons) are the same figures in two
         *     units; both are null for a fuel not sold by the litre.
         */
        get: operations["vehicle_fuel"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/v1/vehicles/{id}/investigations": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        /**
         * NHTSA investigations that list a model year.
         * @description By default only the open ones about a possible defect in the vehicles
         *     listed. An investigation is not a finding that a defect exists, and it
         *     lists a year, make and model, never one vehicle: `note` says so.
         */
        get: operations["vehicle_investigations"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/v1/vehicles/{id}/overview": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        /**
         * Everything held for a model year, in one answer.
         * @description A block this server has no data for is `null`. The `recalls` block
         *     counts the campaigns that may apply to the year, make and model; it is
         *     not a lookup by VIN.
         */
        get: operations["vehicle_overview"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/v1/vehicles/{id}/recalls": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        /**
         * Recalls that may apply to a model year.
         * @description The list is for the year, make and model. It is not a lookup by VIN:
         *     `may_apply` is always `true`, and `lookup` gives the official addresses
         *     where a VIN can be checked.
         */
        get: operations["vehicle_recalls"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/v1/vehicles/{id}/safety": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        /**
         * NHTSA's crash test ratings for a model year.
         * @description NHTSA rates by body style and drive, so `ratings` often has several
         *     entries and their stars can differ: each says which vehicle it is for,
         *     in NHTSA's words. An entry with no stars is a vehicle NHTSA lists and
         *     has not tested. `ratings` is empty when NHTSA lists none for the model
         *     year.
         */
        get: operations["vehicle_safety"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/v1/vehicles/{id}/specs": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        /**
         * Specifications of a model year from EPA's files.
         * @description One entry per EPA rating: transmission and gears, drive, recommended
         *     fuel, forced induction, stop-start, cylinder deactivation, valves,
         *     passenger and luggage volume, hybrid battery, and the emission
         *     standards it was certified to. A field EPA does not give is null.
         */
        get: operations["vehicle_specs"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/v1/vin/batch": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        /**
         * Decode up to 50 VINs.
         * @description The answer is an array in the order asked. Each item is a decode, or the
         *     error object a single request for that VIN would have returned.
         *
         *     With `?include=recalls`, each decode that reaches a model year of the
         *     catalog also carries `recalls`: how many campaigns may apply to that
         *     model year. It is a count for the year, make and model, never a lookup
         *     by VIN.
         */
        post: operations["batch"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/v1/vin/{vin}": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        /**
         * Decode one VIN.
         * @description Spaces and dashes in the VIN are ignored and letters may be in either
         *     case. A wrong check digit is not an error: `valid` is `false` and a
         *     warning says so.
         */
        get: operations["decode"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/v1/vin/{vin}/fluids": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        /**
         * Fluids for the vehicle a VIN decodes to.
         * @description When the decode settles the engine, `engine_oil` holds the one entry
         *     of that engine, or none. Otherwise it holds every engine's.
         */
        get: operations["vin_fluids"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/v1/vin/{vin}/fuel": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        /**
         * Fuel consumption ratings for the vehicle a VIN decodes to.
         * @description When the decode settles the engine, `engine_settled` is `true` and
         *     `ratings` holds only the ratings matched to that engine. Otherwise it
         *     holds every rating of the model year, as the vehicle route does.
         */
        get: operations["vin_fuel"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/v1/vin/{vin}/overview": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        /**
         * Everything held for the model year a VIN decodes to, in one answer.
         * @description The VIN only names the model year: `recalls` counts the campaigns that
         *     may apply to it, and no public data says a recall applies to one VIN. A
         *     VIN that decodes to no vehicle in the catalog gets `vehicle: null`,
         *     every block `null` and the warning `no_catalog_vehicle`, not an error.
         */
        get: operations["vin_overview"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/v1/vin/{vin}/recalls": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        /**
         * Recalls that may apply to the year, make and model a VIN decodes to.
         * @description This decodes the VIN and lists the recalls of its model year. It does
         *     not look the VIN itself up, which no public data allows: `may_apply` is
         *     always `true`. A VIN that decodes to no vehicle in the catalog gets an
         *     empty list and the warning `no_catalog_vehicle`, not an error.
         */
        get: operations["vin_recalls"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/v1/vin/{vin}/specs": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        /**
         * Specifications for the vehicle a VIN decodes to.
         * @description Narrowed to the decoded engine when the decode settles it, as the
         *     fuel route is.
         */
        get: operations["vin_specs"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
}
export type webhooks = Record<string, never>;
export interface components {
    schemas: {
        /**
         * @description One answer of a batch: a decode, or the error a single request for that
         *     VIN would have given.
         */
        BatchItem: components["schemas"]["VinDecode"] | components["schemas"]["ErrorBody"];
        BatchRequest: {
            /**
             * @description At most 50 VINs.
             * @example [
             *       "KM8K2CAB4PU001140",
             *       "1HGCM82633A004352"
             *     ]
             */
            vins: string[];
        };
        CheckDigit: {
            /**
             * @description What position 9 is.
             * @example 4
             */
            actual: string;
            /**
             * @description What position 9 should be.
             * @example 4
             */
            expected: string;
            valid: boolean;
        };
        /** @description City, highway and combined consumption in litres per 100 km. */
        Consumption: {
            /** Format: double */
            city?: number | null;
            /** Format: double */
            combined?: number | null;
            /** Format: double */
            highway?: number | null;
        };
        /** @description One dataset the server holds beside the catalog. */
        DatasetInfo: {
            /**
             * @description The day the source was current.
             * @example 2026-10-11
             */
            as_of: string;
            /**
             * @description The credit to show with this data. Empty when none is asked for.
             * @example
             */
            attribution: string;
            /**
             * @description `public-domain`, `ogl-canada`, `cc-by-4.0` or `mit`.
             * @example public-domain
             */
            licence: string;
            /**
             * Format: int64
             * @description Rows in the dataset.
             * @example 26394
             */
            rows: number;
        };
        Engine: {
            configuration?: string | null;
            /** Format: int32 */
            cylinders?: number | null;
            /** Format: int32 */
            displacement_cc?: number | null;
            /** Format: double */
            displacement_l?: number | null;
            electrification?: string | null;
            fuel?: string | null;
            /**
             * @description The short name shops use, such as `2.0L Turbo` or `3.5L V6`.
             * @example 2.0L
             */
            label?: string | null;
            model?: string | null;
            turbo?: boolean | null;
        };
        /** @description The engine oil viscosity grade of one engine. */
        EngineOil: {
            /**
             * @description The maker's own spellings the grades were read from.
             * @example [
             *       "5W30 GF-5"
             *     ]
             */
            as_written: string[];
            /**
             * @description `engine` when the grade is for one catalog engine; `model` when
             *     only the model year is known, and then `engine` is null.
             */
            confidence: string;
            /** Format: int32 */
            cylinders?: number | null;
            /** Format: double */
            displacement_l?: number | null;
            engine?: null | components["schemas"]["EngineRef"];
            /**
             * @description Standard grades. More than one when the maker listed more than one.
             * @example [
             *       "5W-30"
             *     ]
             */
            grades: string[];
            /**
             * @description Always `EPA fuel economy certification`: the grade the maker
             *     certified with, not a reading of the owner's manual.
             */
            source: string;
        };
        /** @description An engine in the engine step. */
        EngineOption: {
            /** @example 3-5l-turbo-v6 */
            id: string;
            /** @example 3.5L Turbo V6 */
            label: string;
            /** @description Whether the engine comes from this project's list instead of NHTSA. */
            preset: boolean;
            /**
             * @description The characters in position 8 of a VIN that mean this engine and no
             *     other for this model year, where the data settles it.
             */
            vin8?: string | null;
        };
        /** @description A catalog engine of the model year. */
        EngineRef: {
            /**
             * @description The engine's id form, as in a vehicle id.
             * @example 2-7l-v6
             */
            id: string;
            /** @example 2.7L V6 */
            label: string;
            /**
             * @description The characters in position 8 of a VIN that mean this engine.
             * @example P
             */
            vin8?: string | null;
        };
        /** @description One vehicle of the catalog, down to whatever level was asked for. */
        Entry: {
            body?: string | null;
            drive?: string | null;
            engine?: string | null;
            /**
             * @description The stable id. It stays the same from one data release to the next
             *     for as long as the names do.
             * @example 2019_honda_civic_si
             */
            id: string;
            make: string;
            model: string;
            submodel?: string | null;
            /**
             * @description One line naming everything above that is known.
             * @example 2019 Honda Civic Si, Manual, FWD, Sedan
             */
            summary: string;
            transmission?: string | null;
            /** @description NHTSA's vehicle types for this model year, such as `Truck`. */
            vehicle_types: string[];
            /** Format: int32 */
            year: number;
        };
        /** @description The body of every error: `{ "error": { "code", "message", "details" } }`. */
        ErrorBody: {
            error: components["schemas"]["ErrorDetail"];
        };
        ErrorDetail: {
            /**
             * @description A stable code to branch on. One of `invalid_vin`, `validation_failed`,
             *     `not_found`, `dataset_unavailable`, `method_not_allowed`,
             *     `payload_too_large`, `uri_too_long`, `rate_limited`, `unavailable`,
             *     `internal_error`.
             * @example invalid_vin
             */
            code: string;
            /**
             * @description More about the error. Always an object; empty when there is nothing
             *     to add.
             */
            details: Record<string, unknown>;
            /**
             * @description A sentence for a person to read.
             * @example a VIN has 17 characters, this has 16
             */
            message: string;
        };
        /**
         * @description The day each country's fuel ratings were current. Null for a country
         *     this server holds no ratings for.
         */
        FuelAsOf: {
            /** @example 2026-10-06 */
            ca?: string | null;
            /** @example 2026-09-29 */
            us?: string | null;
        };
        /** @description One fuel consumption rating. */
        FuelRating: {
            /**
             * Format: int32
             * @description Tailpipe carbon dioxide, combined.
             */
            co2_g_km?: number | null;
            /**
             * @description How far the rating was matched: `model` (the model year only),
             *     `engine` (one engine) or `submodel` (an engine and a trim).
             * @example engine
             */
            confidence: string;
            /**
             * @description `us` or `ca`.
             * @example us
             */
            country: string;
            /** Format: int32 */
            cylinders?: number | null;
            /**
             * @description The source's own name for the vehicle rated.
             * @example F150 Pickup 2WD
             */
            described_as: string;
            /** Format: double */
            displacement_l?: number | null;
            /** @example Rear-Wheel Drive */
            drive?: string | null;
            engine?: null | components["schemas"]["EngineRef"];
            /**
             * @description The fuel the figures are for, which is the recommended grade.
             * @example Regular Gasoline
             */
            fuel_grade: string;
            /**
             * Format: double
             * @description Combined electricity use.
             */
            kwh_100km?: number | null;
            l_100km?: null | components["schemas"]["Consumption"];
            /**
             * @description How the model was found: `exact`, `base` or `curated`.
             * @example exact
             */
            match: string;
            mpg?: null | components["schemas"]["Mpg"];
            /**
             * Format: int32
             * @description On a full battery.
             */
            range_km?: number | null;
            /** Format: int32 */
            range_mi?: number | null;
            /** @description A second fuel the vehicle takes, such as `E85` or `Electricity`. */
            second_fuel?: string | null;
            /**
             * @description `epa` or `nrcan`.
             * @example epa
             */
            source: string;
            /**
             * @description EPA's id for the rating. Null for one from Natural Resources Canada.
             * @example 39241
             */
            source_id?: string | null;
            /** @example Automatic (S10) */
            transmission?: string | null;
        };
        /**
         * @description One NHTSA defect investigation. An investigation is not a finding that a
         *     defect exists.
         */
        Investigation: {
            /**
             * @description NHTSA's action number.
             * @example EA23001
             */
            action: string;
            /**
             * @description The recall campaigns it led to, as NHTSA numbers them. Empty when it
             *     led to none, or to none yet.
             */
            campaigns: string[];
            /**
             * @description The day it was closed. `null` while it is open, and for some old
             *     investigations whose closing NHTSA's file does not record.
             */
            closed?: string | null;
            /** @description The components it names, as NHTSA words them, separated by `; `. */
            component?: string | null;
            /**
             * @description The letters the number starts with.
             * @example EA
             */
            kind: string;
            /**
             * @description The kind in words. `null` for a kind with no name on file.
             * @example Engineering analysis
             */
            kind_name?: string | null;
            /** @description The manufacturers it names, separated by `; `. */
            manufacturer?: string | null;
            /**
             * @description In an answer about one model year: how the investigation's vehicle
             *     text reached that model. `exact`, `base` (it names a series or trim
             *     of the model) or `curated`. `null` in an answer about a campaign.
             * @example exact
             */
            match?: string | null;
            /**
             * @description In an answer about one model year: the model names the
             *     investigation lists that reached it.
             */
            models: string[];
            /** @description Whether it was opened in 2010 or later and has no day closed. */
            open: boolean;
            /**
             * @description The day it was opened.
             * @example 2023-03-17
             */
            opened?: string | null;
            /**
             * @description NHTSA's one-line subject.
             * @example High pressure fuel pump failure
             */
            subject?: string | null;
            /** @description NHTSA's summary. It can be several thousand characters. */
            summary?: string | null;
        };
        /** @description A make in the make step of a vehicle form. */
        Make: {
            /**
             * @description The make's id form.
             * @example mercedes-benz
             */
            id: string;
            /** @example Mercedes-Benz */
            name: string;
            /** @description Whether it is on the list of popular makes, which are given first. */
            popular: boolean;
        };
        /** @description One make, with links to the maker's own pages. */
        MakeDetail: {
            /**
             * @description The make's id form.
             * @example honda
             */
            id: string;
            links?: null | components["schemas"]["MakeLinks"];
            /** @example Honda */
            name: string;
            /** @description Whether it is on the list of popular makes. */
            popular: boolean;
        };
        /**
         * @description The maker's own pages, found by a person. An address that was not
         *     found is `null`. They lead off this service and can move.
         */
        MakeLinks: {
            /**
             * @description The day every link was last opened by a person.
             * @example 2026-10-12
             */
            checked: string;
            /** @description The maker's telephone line for customers. */
            customer_line?: string | null;
            maintenance_url?: string | null;
            manual_url?: string | null;
            parts_url?: string | null;
            /** @description Where an owner checks a VIN for open recalls. */
            recall_lookup_url?: string | null;
            /** @description The maker's service information site, where it sells access. */
            service_info_url?: string | null;
        };
        Manufacturer: {
            country?: string | null;
            /** @example Hyundai Motor Co */
            name: string;
            vehicle_type?: string | null;
            /**
             * @description The manufacturer code: three characters, or six for a low-volume
             *     maker.
             * @example KM8
             */
            wmi: string;
        };
        /** @description What is being served. */
        MetaResponse: {
            /**
             * @description When the data file was built, in UTC.
             * @example 2026-10-01 04:25:57
             */
            built_at: string;
            /**
             * @description The data release, such as `2026.09`. Also sent as `X-Data-Version`.
             * @example 2026.09
             */
            data_version: string;
            /**
             * @description The datasets served beside the catalog, such as `recalls_us`, by
             *     id. Empty when the server has none. A route for a dataset that is
             *     not listed answers `404` with the code `dataset_unavailable`.
             */
            datasets: {
                [key: string]: components["schemas"]["DatasetInfo"];
            };
            /**
             * @description The version of the server.
             * @example 0.2.0
             */
            server_version: string;
            /**
             * @description The NHTSA vPIC release the data was built from.
             * @example vPICList_lite_2026_09
             */
            vpic_release: string;
        };
        /** @description A model in the model step. */
        Model: {
            /**
             * @description The model's id form. Unique within its make.
             * @example f-150
             */
            id: string;
            /** @example F-150 */
            name: string;
            /**
             * Format: int32
             * @description The first and last model year the model exists in.
             */
            year_from: number;
            /** Format: int32 */
            year_to: number;
        };
        /** @description One model year of a model. */
        ModelYearSummary: {
            /**
             * Format: int32
             * @description How many service bulletins are on file. `null` until this server
             *     holds bulletins.
             */
            bulletins?: number | null;
            /**
             * @description The vehicle id of the year, make and model.
             * @example 2018_ford_f-150
             */
            id: string;
            recalls?: null | components["schemas"]["RecallCount"];
            /**
             * @description The model year's page on this site.
             * @example https://open.wenmarpro.com/makes/ford/f-150/2018
             */
            url: string;
            /** Format: int32 */
            year: number;
        };
        /** @description Every model year of one model, newest first. */
        ModelYears: {
            as_of: components["schemas"]["RecallAsOf"];
            attribution: string[];
            lookup: components["schemas"]["RecallLookup"];
            make: components["schemas"]["NamedId"];
            /**
             * @description Always `true`: a recall count is of campaigns that may apply to the
             *     model year.
             */
            may_apply: boolean;
            model: components["schemas"]["NamedId"];
            years: components["schemas"]["ModelYearSummary"][];
        };
        /** @description City, highway and combined economy in miles per US gallon. */
        Mpg: {
            /** Format: int32 */
            city?: number | null;
            /** Format: int32 */
            combined?: number | null;
            /** Format: int32 */
            highway?: number | null;
        };
        /** @description A make or a model: its id form and its name. */
        NamedId: {
            /** @example ford */
            id: string;
            /** @example Ford */
            name: string;
        };
        /**
         * @description Everything held for one model year, in one answer.
         *
         *     A block this server has no data for is `null`: `specs`, `fuel`,
         *     `fluids`, `tires`, `labor` and `bulletins` are `null` in every answer
         *     today, and each is filled when its data arrives. A client must accept
         *     an object there.
         */
        Overview: {
            /** @description The credit lines the data's licences ask for. */
            attribution: string[];
            bulletins?: Record<string, unknown> | null;
            decode?: null | components["schemas"]["OverviewDecode"];
            fluids?: Record<string, unknown> | null;
            fuel?: Record<string, unknown> | null;
            labor?: Record<string, unknown> | null;
            links: components["schemas"]["OverviewLinks"];
            recalls?: null | components["schemas"]["OverviewRecalls"];
            safety?: null | components["schemas"]["VehicleSafety"];
            specs?: Record<string, unknown> | null;
            tires?: Record<string, unknown> | null;
            vehicle?: null | components["schemas"]["RecallVehicle"];
            warnings: components["schemas"]["Warning"][];
        };
        /** @description What the VIN of an overview decoded to. */
        OverviewDecode: {
            /**
             * @description The engine in short form, such as `2.0L`, when the VIN names one.
             * @example 2.0L
             */
            engine?: string | null;
            /** @description Whether the VIN settles on one engine of the catalog. */
            engine_settled: boolean;
            /**
             * @description The most specific catalog entry the VIN reaches.
             * @example 2023_hyundai_kona_se
             */
            entry_id?: string | null;
            make?: string | null;
            model?: string | null;
            trim?: string | null;
            /** @description Whether the check digit, position 9, is right. */
            valid: boolean;
            /** @example KM8K2CAB4PU001140 */
            vin: string;
            /** Format: int32 */
            year?: number | null;
        };
        /** @description Where to read more. */
        OverviewLinks: {
            maker?: null | components["schemas"]["MakeLinks"];
            /**
             * @description The model year's page on this site. `null` when the VIN reaches no
             *     model year of the catalog.
             * @example https://open.wenmarpro.com/makes/ford/f-150/2018
             */
            page?: string | null;
        };
        /** @description The recalls of a model year, in brief. */
        OverviewRecalls: {
            as_of: components["schemas"]["RecallAsOf"];
            count: components["schemas"]["RecallCount"];
            /** @description Whether any campaign carries NHTSA's advisory not to drive. */
            do_not_drive: boolean;
            lookup: components["schemas"]["RecallLookup"];
            /**
             * @description Always `true`. A campaign lists a year, make and model; only the
             *     manufacturer can say whether one vehicle is in it.
             */
            may_apply: boolean;
            /**
             * @description The three newest campaigns. The whole list is at
             *     `/v1/vehicles/{id}/recalls`.
             */
            newest: components["schemas"]["Recall"][];
            /** @description Whether any campaign carries NHTSA's advisory to park outside. */
            park_outside: boolean;
            /**
             * @description The model year's recall page on this site.
             * @example https://open.wenmarpro.com/makes/ford/f-150/2018/recalls
             */
            url: string;
        };
        /** @description Where the vehicle was built. `code` is position 11 of the VIN. */
        Plant: {
            city?: string | null;
            /** @example U */
            code: string;
            company?: string | null;
            country?: string | null;
            state?: string | null;
        };
        /** @description One recall campaign in a list. */
        Recall: {
            /**
             * @description In an answer about one model year: the first and last build date
             *     the campaign gives. Compare them with the date on the door sticker.
             */
            built_from?: string | null;
            built_to?: string | null;
            /**
             * @description The publisher's campaign number.
             * @example 21V983000
             */
            campaign: string;
            /**
             * @description The component, as the source words it.
             * @example LATCHES/LOCKS/LINKAGES:TAILGATE:LATCH
             */
            component?: string | null;
            /**
             * @description `us` or `ca`.
             * @example us
             */
            country: string;
            /**
             * @description The day the campaign was reported.
             * @example 2021-12-16
             */
            date?: string | null;
            /**
             * @description NHTSA's advisory not to drive the vehicle until it is repaired.
             *     `null` for a Canadian campaign: Transport Canada publishes no such
             *     field.
             */
            do_not_drive?: boolean | null;
            manufacturer?: string | null;
            /**
             * @description In an answer about one model year: how the campaign's vehicle text
             *     reached that model. `exact`, `base` (the campaign names a series or
             *     trim of it) or `curated`. `null` in a list that is not about one
             *     model year.
             * @example exact
             */
            match?: string | null;
            /**
             * @description The manufacturer's own campaign number, which a dealer looks a
             *     recall up by.
             * @example 21S53
             */
            mfr_campaign?: string | null;
            /**
             * @description In an answer about one model year: the model names the campaign
             *     lists that reached it.
             */
            models: string[];
            /** @description Transport Canada's notification type, such as `Safety Mfr`. */
            notification?: string | null;
            /** @description NHTSA's advisory to park outside, away from buildings. */
            park_outside?: boolean | null;
            /**
             * @description Who published it: `nhtsa` or `tc` (Transport Canada).
             * @example nhtsa
             */
            source: string;
            /** @description What is wrong, in the source's words. */
            summary?: string | null;
            /**
             * Format: int32
             * @description Units potentially affected.
             */
            units?: number | null;
            /**
             * @description The campaign's page on this site.
             * @example https://open.wenmarpro.com/recalls/nhtsa/21V983000
             */
            url: string;
        };
        /**
         * @description The day each country's recall data was current. `null` for a country
         *     this server holds no recalls for.
         */
        RecallAsOf: {
            /** @example 2026-10-11 */
            ca?: string | null;
            /** @example 2026-10-11 */
            us?: string | null;
        };
        /** @description One campaign with every vehicle it lists. */
        RecallCampaign: {
            /** @description The day this publisher's data was current. */
            as_of?: string | null;
            attribution: string[];
            /** @example 19V888000 */
            campaign: string;
            completion?: null | components["schemas"]["RecallCompletion"];
            component?: string | null;
            consequence?: string | null;
            /** @example us */
            country: string;
            date?: string | null;
            do_not_drive?: boolean | null;
            french?: null | components["schemas"]["RecallFrench"];
            lookup: components["schemas"]["RecallLookup"];
            manufacturer?: string | null;
            /** @description Always `true`: the campaign may apply to a vehicle it lists. */
            may_apply: boolean;
            mfr_campaign?: string | null;
            notes?: string | null;
            notification?: string | null;
            /** @description The day owners were notified. */
            owner_notified?: string | null;
            park_outside?: boolean | null;
            /** @description The manufacturer's part numbers, separated by commas. */
            part_number?: string | null;
            remedy?: string | null;
            /** @example nhtsa */
            source: string;
            summary?: string | null;
            /** Format: int32 */
            units?: number | null;
            /** @description The campaign's page on this site. */
            url: string;
            /** @description Every vehicle the campaign lists, by make, model and year. */
            vehicles: components["schemas"]["RecallCampaignVehicle"][];
        };
        /** @description One vehicle a campaign lists. */
        RecallCampaignVehicle: {
            built_from?: string | null;
            built_to?: string | null;
            /**
             * @description The make and model as the source words them.
             * @example CHEVROLET
             */
            make: string;
            /**
             * @description `exact`, `base` or `curated`; `null` when the model is not in the
             *     catalog, as for most heavy trucks, buses and trailers.
             * @example base
             */
            match?: string | null;
            /** @example SILVERADO 1500 */
            model: string;
            /**
             * @description The catalog's id for the model year, when the model matched.
             * @example 2019_chevrolet_silverado
             */
            vehicle_id?: string | null;
            /** Format: int32 */
            year?: number | null;
        };
        /**
         * @description The newest report of remedy progress a manufacturer filed with NHTSA
         *     for a campaign. The counts are totals for the whole campaign at the end
         *     of the period named, which may be years past. They say nothing of one
         *     vehicle: check a VIN at the addresses in `lookup`.
         */
        RecallCompletion: {
            /**
             * @description The day NHTSA's report files were current.
             * @example 2026-10-12
             */
            as_of?: string | null;
            /**
             * Format: int32
             * @description Vehicles in the campaign.
             * @example 57591
             */
            involved: number;
            /**
             * @description `quarterly` or `annual`.
             * @example annual
             */
            kind: string;
            /**
             * @description The first quarter the report covers, as a year and a quarter.
             * @example 2025-2
             */
            period_from: string;
            /**
             * @description The last quarter it covers. The counts are as of the end of it. A
             *     quarterly report has the same quarter in both.
             * @example 2026-1
             */
            period_to: string;
            /**
             * Format: int32
             * @description Of those, reported remedied.
             * @example 38917
             */
            remedied: number;
            /**
             * Format: int32
             * @description `remedied` as a share of `involved`, in whole percent, rounded
             *     down. `null` when `involved` is 0.
             * @example 67
             */
            remedied_percent?: number | null;
            /**
             * Format: int32
             * @description NHTSA's "total removed", as reported.
             * @example 23
             */
            removed: number;
            /**
             * Format: int32
             * @description The report's number among those of its kind, from 1.
             * @example 2
             */
            report_number: number;
            /**
             * @description The day the manufacturer filed the report.
             * @example 2026-04-13
             */
            submitted?: string | null;
            /**
             * Format: int32
             * @description NHTSA's "total unreachable", as reported.
             * @example 1011
             */
            unreachable: number;
        };
        /**
         * @description How many recall campaigns list a model year, for each country. `null`
         *     for a country this server holds no recalls for; `0` when it holds them
         *     and none lists the model year.
         */
        RecallCount: {
            /**
             * Format: int32
             * @example 5
             */
            ca?: number | null;
            /**
             * Format: int32
             * @example 7
             */
            us?: number | null;
        };
        /** @description What a VIN decoded to, as far as recalls need it. */
        RecallDecode: {
            make?: string | null;
            model?: string | null;
            trim?: string | null;
            /** @description Whether the check digit is right. */
            valid: boolean;
            /** @example KM8K2CAB4PU001140 */
            vin: string;
            /** Format: int32 */
            year?: number | null;
        };
        /** @description Transport Canada's French text for a campaign. */
        RecallFrench: {
            component?: string | null;
            notification?: string | null;
            summary?: string | null;
        };
        /** @description The NHTSA investigations that led to one recall campaign. */
        RecallInvestigations: {
            /** @example 2026-10-12 */
            as_of: string;
            attribution: string[];
            /** @example 18V568000 */
            campaign: string;
            /**
             * @description Oldest first. Empty when the campaign followed no investigation,
             *     and for every Transport Canada campaign.
             */
            investigations: components["schemas"]["Investigation"][];
            note: string;
            /**
             * @description `nhtsa` or `tc`.
             * @example nhtsa
             */
            source: string;
            /** @description The campaign's page on this site. */
            url: string;
        };
        /** @description One page of campaigns, newest first. */
        RecallList: {
            as_of: components["schemas"]["RecallAsOf"];
            attribution: string[];
            lookup: components["schemas"]["RecallLookup"];
            /** @description Always `true`: a campaign may apply to the vehicles it lists. */
            may_apply: boolean;
            /** @description Hand this back as `cursor` for the next page. `null` on the last. */
            next_cursor?: string | null;
            recalls: components["schemas"]["Recall"][];
        };
        /**
         * @description Where to check one vehicle for open recalls. Neither government
         *     publishes recalls by VIN as data, so no answer here can say a recall
         *     applies to a VIN: these lookups can.
         */
        RecallLookup: {
            /** @description The maker's own lookup by VIN, where one is on file. */
            maker?: string | null;
            /**
             * @description NHTSA's lookup by VIN, for the United States.
             * @example https://www.nhtsa.gov/recalls
             */
            nhtsa: string;
            /** @description Transport Canada's recall search, for Canada. */
            tc: string;
        };
        /** @description The model year an answer is about. */
        RecallVehicle: {
            /**
             * @description The id of the year, make and model.
             * @example 2018_ford_f-150
             */
            id: string;
            /** @example Ford */
            make: string;
            /** @example F-150 */
            model: string;
            /** Format: int32 */
            year: number;
        };
        /** @description One campaign of the recent list, with the vehicles it lists. */
        RecentRecall: {
            recall: components["schemas"]["Recall"];
            /**
             * @description The vehicles the campaign lists: at most 200 of them. Every one is
             *     at `/v1/recalls/{source}/{campaign}`.
             */
            vehicles: components["schemas"]["RecallCampaignVehicle"][];
            /**
             * Format: int32
             * @description How many vehicles the campaign lists in all.
             */
            vehicles_listed: number;
        };
        /** @description Campaigns reported on or after a day, newest first. */
        RecentRecalls: {
            as_of: components["schemas"]["RecallAsOf"];
            attribution: string[];
            lookup: components["schemas"]["RecallLookup"];
            /** @description Always `true`: a campaign may apply to the vehicles it lists. */
            may_apply: boolean;
            /** @description Hand it back as `cursor` for the next page. `null` on the last. */
            next_cursor?: string | null;
            recalls: components["schemas"]["RecentRecall"][];
            /**
             * @description The day asked for.
             * @example 2026-09-01
             */
            since: string;
        };
        /**
         * @description Safety equipment as the manufacturer reported it: `Standard`, `Optional`,
         *     or a system type such as `Direct`.
         */
        Safety: {
            abs?: string | null;
            adaptive_cruise?: string | null;
            airbags_curtain?: string | null;
            airbags_front?: string | null;
            airbags_knee?: string | null;
            airbags_side?: string | null;
            auto_brake?: string | null;
            backup_camera?: string | null;
            blind_spot?: string | null;
            dynamic_brake_support?: string | null;
            esc?: string | null;
            forward_collision?: string | null;
            lane_centering?: string | null;
            lane_departure?: string | null;
            lane_keep?: string | null;
            park_assist?: string | null;
            pedestrian_braking?: string | null;
            rear_cross_traffic?: string | null;
            /** @description How tire pressure is monitored, as NHTSA words it, such as `Direct`. */
            tpms?: string | null;
            traction_control?: string | null;
        };
        /**
         * @description The safety features NHTSA lists with a rating. A feature is `standard`,
         *     `optional`, `both` (standard on some versions, optional on others) or
         *     `no`. An evaluation is `meets`, `does_not_meet`, `standard_only`,
         *     `upgrade_only` or `pending`. `null` where NHTSA's file does not say.
         *
         *     For the four driver-assistance features (forward collision warning,
         *     lane departure warning, crash imminent braking, dynamic brake support)
         *     NHTSA's file means two things by its years. Through model year 2022,
         *     `standard`, `optional` and `both` say the feature is fitted and met
         *     NHTSA's performance criteria, `no` says it is not fitted or did not
         *     meet them, and there is no evaluation. From model year 2023 the value
         *     says only whether the feature is fitted, and the evaluation says what
         *     NHTSA's test found.
         */
        SafetyFeatures: {
            backup_camera?: string | null;
            blind_spot_detection?: string | null;
            /** @description Automatic emergency braking: the vehicle brakes by itself. */
            crash_imminent_braking?: string | null;
            crash_imminent_braking_evaluation?: string | null;
            /** @description The vehicle adds to the driver's braking. */
            dynamic_brake_support?: string | null;
            dynamic_brake_support_evaluation?: string | null;
            /**
             * @description Electronic stability control.
             * @example standard
             */
            esc?: string | null;
            forward_collision_warning?: string | null;
            /** @example meets */
            forward_collision_warning_evaluation?: string | null;
            /** @description Side air bags for the head, and for the torso. */
            head_side_airbag?: string | null;
            lane_departure_warning?: string | null;
            lane_departure_warning_evaluation?: string | null;
            torso_side_airbag?: string | null;
        };
        /** @description One rating NHTSA gave a vehicle of a model year. */
        SafetyRating: {
            /** @example PU/CC */
            body_style?: string | null;
            /**
             * Format: int32
             * @description The weight of the vehicle NHTSA tested. NHTSA gives pounds; the
             *     kilograms are worked out from them and rounded.
             * @example 2295
             */
            curb_weight_kg?: number | null;
            /**
             * Format: int32
             * @example 5060
             */
            curb_weight_lb?: number | null;
            /**
             * @description The vehicle NHTSA rated, in NHTSA's words, on one line.
             * @example F-150 SUPER CREW, PU/CC, 4x4
             */
            description: string;
            /** @example 4x4 */
            drive?: string | null;
            features: components["schemas"]["SafetyFeatures"];
            /**
             * @description How NHTSA's model name reached this model: `exact`, `base` (NHTSA
             *     names a body or a version of it) or `curated`.
             * @example base
             */
            match: string;
            /**
             * @description The model, body style, vehicle type and drive as NHTSA words them.
             * @example F-150 SUPER CREW
             */
            model: string;
            /**
             * @description NHTSA's safety concerns and footnotes: what happened in a test, and
             *     which vehicles the rating does not cover.
             */
            notes: string[];
            /**
             * Format: int32
             * @description 1 for the first release of a model year; 2 or 3 for a later one
             *     that NHTSA rated again.
             * @example 1
             */
            release: number;
            /**
             * Format: double
             * @description The chance of a rollover in a single-vehicle crash, in percent.
             * @example 19.1
             */
            rollover_risk_percent?: number | null;
            /** @description Whether the vehicle tipped up in NHTSA's dynamic test. */
            rollover_tip?: boolean | null;
            stars: components["schemas"]["SafetyStars"];
            /** @example TRUCK */
            vehicle_type?: string | null;
        };
        /**
         * @description The stars of one rating, each from 1 to 5. `null` where NHTSA gave
         *     none: a test it did not run, or a rating older than the overall rating,
         *     which began with model year 2011.
         */
        SafetyStars: {
            /**
             * Format: int32
             * @description The frontal crash test, overall.
             */
            frontal?: number | null;
            /** Format: int32 */
            frontal_driver?: number | null;
            /** Format: int32 */
            frontal_passenger?: number | null;
            /**
             * Format: int32
             * @example 5
             */
            overall?: number | null;
            /** Format: int32 */
            rollover?: number | null;
            /**
             * Format: int32
             * @description The side crash tests, overall.
             */
            side?: number | null;
            /** Format: int32 */
            side_barrier?: number | null;
            /**
             * Format: int32
             * @description The side barrier test: the driver, and the rear passenger.
             */
            side_driver?: number | null;
            /**
             * Format: int32
             * @description The front seat and the rear seat, over the barrier and pole tests.
             */
            side_front_seat?: number | null;
            /** Format: int32 */
            side_passenger?: number | null;
            /** Format: int32 */
            side_pole?: number | null;
            /** Format: int32 */
            side_rear_seat?: number | null;
        };
        /** @description The catalog's answer for one decoded VIN. */
        Selection: {
            /** @description The engine's id form, when the decode settles on one. */
            engine_id?: string | null;
            /** @description The most specific entry the decode reaches. */
            entry: components["schemas"]["Entry"];
            /** @description The submodel's id form, when the decode names exactly one. */
            submodel_id?: string | null;
            /**
             * @description The id of the year, make and model.
             * @example 2023_hyundai_kona
             */
            vehicle_id: string;
        };
        /** @description The specifications that come with one EPA rating. */
        Spec: {
            /** @example Lithium Ion */
            battery_type?: string | null;
            /** Format: int32 */
            battery_voltage?: number | null;
            /** @description `model`, `engine` or `submodel`, as on a fuel rating. */
            confidence: string;
            cylinder_deactivation?: boolean | null;
            /** @description The maker's name for it, such as `Active Fuel Management`. */
            cylinder_deactivation_name?: string | null;
            /** @example F150 Pickup 2WD */
            described_as: string;
            drive?: string | null;
            /** @description EPA's kind of alternative powertrain: `Hybrid`, `Plug-in Hybrid`, `EV`, `FFV`, `Diesel`. */
            electrification?: string | null;
            emission_standards: components["schemas"]["SpecEmission"][];
            engine?: null | components["schemas"]["EngineRef"];
            ev_motor?: string | null;
            /** Format: int32 */
            exhaust_valves?: number | null;
            /** @example Regular Gasoline */
            fuel_grade: string;
            /**
             * @description Whether the grade is recommended or required, in EPA's words.
             * @example Gasoline (Premium Unleaded Required)
             */
            fuel_usage?: string | null;
            /** Format: int32 */
            gears?: number | null;
            /**
             * Format: int32
             * @description Per cylinder.
             */
            intake_valves?: number | null;
            /** Format: int32 */
            luggage_volume_ft3?: number | null;
            /** Format: int32 */
            luggage_volume_l?: number | null;
            /** @description Rated motor power in kW, as EPA's file writes it: `135`, `8 and 30`. */
            motor_kw?: string | null;
            /** Format: int32 */
            passenger_volume_ft3?: number | null;
            /** Format: int32 */
            passenger_volume_l?: number | null;
            /** @example Standard Pickup Trucks 2WD */
            size_class: string;
            /** @example epa */
            source: string;
            /** @example 39241 */
            source_id: string;
            start_stop?: boolean | null;
            supercharger: boolean;
            transmission?: string | null;
            /** @example Semi-Automatic */
            transmission_type?: string | null;
            turbo: boolean;
            variable_valve_lift?: boolean | null;
            variable_valve_timing?: boolean | null;
        };
        /** @description An emission standard a vehicle was certified to. */
        SpecEmission: {
            /**
             * @description The engine family (test group).
             * @example JFMXT02.73JK
             */
            engine_family: string;
            /**
             * Format: int32
             * @description EPA's sales area code, a number. EPA's column list does not say
             *     what each number means, so no name is given.
             * @example 3
             */
            sales_area: number;
            /**
             * Format: int32
             * @description EPA's smog rating, 1 (worst) to 10 (best).
             */
            smog_score?: number | null;
            /** @example T3B70 */
            standard: string;
            /** @example Federal Tier 3 Bin 70 */
            standard_text?: string | null;
        };
        /** @description A submodel: a trim, or a series where the data has no trim. */
        Submodel: {
            /** @example ex-l */
            id: string;
            /**
             * @description `trim` or `series` as NHTSA has it, or `preset` from this project's
             *     own list.
             * @example trim
             */
            kind: string;
            /** @example EX-L */
            name: string;
        };
        /** @description The fluids of one model year. */
        VehicleFluids: {
            /** @example 2026-09-29 */
            as_of?: string | null;
            attribution: string[];
            engine_oil: components["schemas"]["EngineOil"][];
            vehicle: components["schemas"]["RecallVehicle"];
        };
        /** @description The fuel ratings of one model year. */
        VehicleFuel: {
            as_of: components["schemas"]["FuelAsOf"];
            attribution: string[];
            ratings: components["schemas"]["FuelRating"][];
            vehicle: components["schemas"]["RecallVehicle"];
        };
        /** @description NHTSA investigations that list one model year. */
        VehicleInvestigations: {
            /**
             * @description The day NHTSA's investigation file was current.
             * @example 2026-10-12
             */
            as_of: string;
            /** @description The credit lines the data's licence asks for. */
            attribution: string[];
            /** @description Newest first. Empty when none lists the model year. */
            investigations: components["schemas"]["Investigation"][];
            /**
             * @description That an investigation is not a finding of a defect and is not about
             *     one vehicle. Show it wherever the list is shown.
             */
            note: string;
            vehicle: components["schemas"]["RecallVehicle"];
        };
        /** @description Recalls that may apply to one model year. */
        VehicleRecalls: {
            as_of: components["schemas"]["RecallAsOf"];
            /** @description The credit lines the data's licences ask for. */
            attribution: string[];
            lookup: components["schemas"]["RecallLookup"];
            /**
             * @description Always `true`. A campaign lists a year, make and model; only the
             *     manufacturer can say whether one vehicle is in it. Check the VIN at
             *     the addresses in `lookup`.
             */
            may_apply: boolean;
            /** @description Newest first. Empty when none was found as of the dates in `as_of`. */
            recalls: components["schemas"]["Recall"][];
            vehicle: components["schemas"]["RecallVehicle"];
        };
        /** @description NHTSA's crash test ratings for one model year. */
        VehicleSafety: {
            /**
             * @description The day NHTSA's file was current.
             * @example 2026-08-07
             */
            as_of: string;
            /** @description The credit lines the data's licence asks for. NHTSA's asks for none. */
            attribution: string[];
            /**
             * @description One entry for each vehicle of the model year NHTSA rated or lists,
             *     by NHTSA's model name, body style and drive. Empty when it lists
             *     none.
             */
            ratings: components["schemas"]["SafetyRating"][];
            /**
             * @description Who rates the vehicles.
             * @example NHTSA New Car Assessment Program
             */
            source: string;
            /**
             * @description NHTSA's own pages of ratings.
             * @example https://www.nhtsa.gov/ratings
             */
            source_url: string;
            vehicle: components["schemas"]["RecallVehicle"];
        };
        /** @description The specifications of one model year. */
        VehicleSpecs: {
            /** @example 2026-09-29 */
            as_of?: string | null;
            attribution: string[];
            specs: components["schemas"]["Spec"][];
            vehicle: components["schemas"]["RecallVehicle"];
        };
        /** @description A decoded VIN. A field that could not be determined is left out. */
        VinDecode: {
            /**
             * Format: double
             * @description Manufacturer base price in US dollars, when NHTSA has one.
             */
            base_price_usd?: number | null;
            body?: string | null;
            catalog?: null | components["schemas"]["Selection"];
            check_digit: components["schemas"]["CheckDigit"];
            /** Format: int32 */
            doors?: number | null;
            /** @example FWD */
            drivetrain?: string | null;
            engine?: null | components["schemas"]["Engine"];
            /** @description Gross vehicle weight rating class, as NHTSA words it. */
            gvwr?: string | null;
            /** @example Hyundai */
            make?: string | null;
            manufacturer: components["schemas"]["Manufacturer"];
            /** @example Kona */
            model?: string | null;
            plant: components["schemas"]["Plant"];
            recalls?: null | components["schemas"]["VinRecallCount"];
            safety?: null | components["schemas"]["Safety"];
            /** Format: int32 */
            seat_rows?: number | null;
            /** Format: int32 */
            seats?: number | null;
            series?: string | null;
            transmission?: string | null;
            /** Format: int32 */
            transmission_speeds?: number | null;
            /** @example SE */
            trim?: string | null;
            /**
             * @description Whether the check digit, position 9, is right. Many genuine VINs from
             *     outside North America fail it.
             */
            valid: boolean;
            /** @example KM8K2CAB4PU001140 */
            vin: string;
            /** @description Things to know about a decode that still succeeded. */
            warnings: components["schemas"]["Warning"][];
            /**
             * Format: int32
             * @description Front wheel diameter in inches.
             */
            wheel_size_front?: number | null;
            /**
             * Format: int32
             * @description Rear wheel diameter in inches.
             */
            wheel_size_rear?: number | null;
            /**
             * Format: int32
             * @example 2023
             */
            year?: number | null;
        };
        /** @description The fluids for the vehicle a VIN decodes to. */
        VinFluids: {
            as_of?: string | null;
            attribution: string[];
            decode: components["schemas"]["RecallDecode"];
            engine?: null | components["schemas"]["EngineRef"];
            engine_oil: components["schemas"]["EngineOil"][];
            /** @description Whether `engine_oil` is narrowed to `engine`. */
            engine_settled: boolean;
            vehicle?: null | components["schemas"]["RecallVehicle"];
            warnings: components["schemas"]["Warning"][];
        };
        /** @description The fuel ratings for the vehicle a VIN decodes to. */
        VinFuel: {
            as_of: components["schemas"]["FuelAsOf"];
            attribution: string[];
            decode: components["schemas"]["RecallDecode"];
            engine?: null | components["schemas"]["EngineRef"];
            /** @description Whether `ratings` is narrowed to `engine`. */
            engine_settled: boolean;
            ratings: components["schemas"]["FuelRating"][];
            vehicle?: null | components["schemas"]["RecallVehicle"];
            warnings: components["schemas"]["Warning"][];
        };
        /**
         * @description What a batch decode says of recalls when `include=recalls` is asked:
         *     how many campaigns may apply to the model year the VIN decodes to.
         */
        VinRecallCount: {
            count: components["schemas"]["RecallCount"];
            /** @description Whether any of them carries NHTSA's advisory not to drive. */
            do_not_drive: boolean;
            lookup: components["schemas"]["RecallLookup"];
            /**
             * @description Always `true`. The count is for the year, make and model, not for
             *     the VIN: check the VIN at the addresses in `lookup`.
             */
            may_apply: boolean;
            /** @description Whether any of them carries NHTSA's advisory to park outside. */
            park_outside: boolean;
        };
        /** @description Recalls that may apply to the year, make and model a VIN decodes to. */
        VinRecalls: {
            as_of: components["schemas"]["RecallAsOf"];
            attribution: string[];
            decode: components["schemas"]["RecallDecode"];
            lookup: components["schemas"]["RecallLookup"];
            /**
             * @description Always `true`. This is not a lookup by VIN: the list is for the
             *     model year, and the VIN only names it.
             */
            may_apply: boolean;
            recalls: components["schemas"]["Recall"][];
            vehicle?: null | components["schemas"]["RecallVehicle"];
            /**
             * @description The decode's warnings, and `no_catalog_vehicle` when the VIN names
             *     no model year to list recalls for.
             */
            warnings: components["schemas"]["Warning"][];
        };
        /** @description The specifications for the vehicle a VIN decodes to. */
        VinSpecs: {
            as_of?: string | null;
            attribution: string[];
            decode: components["schemas"]["RecallDecode"];
            engine?: null | components["schemas"]["EngineRef"];
            /** @description Whether `specs` is narrowed to `engine`. */
            engine_settled: boolean;
            specs: components["schemas"]["Spec"][];
            vehicle?: null | components["schemas"]["RecallVehicle"];
            warnings: components["schemas"]["Warning"][];
        };
        Warning: {
            /**
             * @description `invalid_check_digit`, `model_year_unknown`, `no_patterns` or
             *     `model_unresolved`. More may be added.
             * @example invalid_check_digit
             */
            code: string;
            message: string;
            /** @description VINs the caller may have meant. */
            suggestions?: string[];
        };
    };
    responses: never;
    parameters: never;
    requestBodies: never;
    headers: never;
    pathItems: never;
}
export type $defs = Record<string, never>;
export interface operations {
    make: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                /**
                 * @description A make's id, name or alias.
                 * @example honda
                 */
                make: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description The make. `links` is `null` when none are recorded. */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["MakeDetail"];
                };
            };
            /** @description `not_found`: no make has that id. */
            404: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
        };
    };
    meta: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description About the data being served. */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["MetaResponse"];
                };
            };
        };
    };
    model_years: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                /**
                 * @description A make's id, name or alias.
                 * @example ford
                 */
                make: string;
                /**
                 * @description A model's id or name.
                 * @example f-150
                 */
                model: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description The model years. `recalls` is `null` in each when this server holds no recalls. */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ModelYears"];
                };
            };
            /** @description `not_found`: no model has that id. */
            404: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
        };
    };
    recall_search: {
        parameters: {
            query?: {
                /**
                 * @description Only campaigns that list a vehicle of this make: a name, an alias
                 *     or an id, as in `/v1/vehicles/models`.
                 * @example ford
                 */
                make?: string;
                /**
                 * @description Only campaigns that list this model: a name or an id. Needs `make`.
                 * @example f-150
                 */
                model?: string;
                /**
                 * @description Only campaigns that list this model year.
                 * @example 2018
                 */
                year?: number;
                /**
                 * @description Only campaigns whose component text contains this, in either case.
                 * @example air bags
                 */
                component?: string;
                /**
                 * @description Only campaigns with this text in them, in either case: in the
                 *     campaign number, the manufacturer's campaign number, the
                 *     manufacturer, the component, the summary, the consequence, the
                 *     remedy, the notes, or the make or model of a vehicle listed. At
                 *     most 100 characters are read.
                 * @example tailgate
                 */
                q?: string;
                /** @description `us` or `ca`. Left out: both, where the server holds both. */
                country?: string;
                /** @description At most this many, up to 100. The default is 25. */
                limit?: number;
                /** @description `next_cursor` from the page before, unchanged. */
                cursor?: string;
            };
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description One page of campaigns. */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["RecallList"];
                };
            };
            /** @description `validation_failed`: `country`, `year`, `limit` or `cursor` is wrong, or `model` was given without `make`. */
            400: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
            /** @description `dataset_unavailable`: this server holds no recalls, or none for the country asked for. */
            404: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
        };
    };
    recent_recalls: {
        parameters: {
            query?: {
                /**
                 * @description Only campaigns reported on or after this day, as `2026-09-01`.
                 *     Required.
                 * @example 2026-09-01
                 */
                since?: string;
                /** @description `us` or `ca`. Left out: both, where the server holds both. */
                country?: string;
                /**
                 * @description Only campaigns that list a vehicle of this make: a name, an alias
                 *     or an id.
                 * @example ford
                 */
                make?: string;
                /** @description At most this many, up to 100. The default is 25. */
                limit?: number;
                /** @description `next_cursor` from the page before, unchanged. */
                cursor?: string;
            };
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description One page of campaigns. */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["RecentRecalls"];
                };
            };
            /** @description `validation_failed`: `since` is missing or is not a day, or `country`, `limit` or `cursor` is wrong. */
            400: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
            /** @description `dataset_unavailable`: this server holds no recalls, or none for the country asked for. */
            404: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
        };
    };
    recall_campaign: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                /**
                 * @description Who published it: `nhtsa` or `tc` (Transport Canada).
                 * @example nhtsa
                 */
                source: string;
                /**
                 * @description The publisher's campaign number. Letters may be in either case.
                 * @example 19V888000
                 */
                campaign: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description The campaign. */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["RecallCampaign"];
                };
            };
            /** @description `not_found`: no campaign has that number. `dataset_unavailable`: this server holds no recalls from that publisher. */
            404: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
        };
    };
    recall_investigations: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                /**
                 * @description Who published the campaign: `nhtsa` or `tc` (Transport Canada).
                 * @example nhtsa
                 */
                source: string;
                /**
                 * @description The publisher's campaign number. Letters may be in either case.
                 * @example 18V568000
                 */
                campaign: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description The investigations, oldest first. */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["RecallInvestigations"];
                };
            };
            /** @description `not_found`: that is not a campaign number. `dataset_unavailable`: this server holds no investigations. */
            404: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
        };
    };
    engines: {
        parameters: {
            query: {
                /** @example honda */
                make: string;
                /** @example civic */
                model: string;
                /** @example 2019 */
                year: number;
                /**
                 * @description Only engines this submodel comes with.
                 * @example si
                 */
                submodel?: string;
                /** @description Only engines whose label starts with this. */
                term?: string;
            };
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Engines. */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["EngineOption"][];
                };
            };
            /** @description `validation_failed`: `make`, `model` or `year` is missing. */
            400: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
        };
    };
    makes: {
        parameters: {
            query?: {
                /**
                 * @description Only makes with a model in this model year.
                 * @example 2019
                 */
                year?: number;
                /** @description `light` (the default), `all`, or a vPIC vehicle type id. */
                scope?: string;
                /**
                 * @description Only makes whose name or alias starts with this. Case and punctuation
                 *     are ignored.
                 */
                term?: string;
                /** @description At most this many, up to 500. */
                limit?: number;
            };
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Makes. */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["Make"][];
                };
            };
            /** @description `validation_failed`. */
            400: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
        };
    };
    models: {
        parameters: {
            query: {
                /**
                 * @description A make's name, alias or id: `Chevrolet`, `chevy` or `chevrolet`.
                 * @example honda
                 */
                make: string;
                /**
                 * @description Only models of this model year.
                 * @example 2019
                 */
                year?: number;
                /** @description `light` (the default), `all`, or a vPIC vehicle type id. */
                scope?: string;
                /** @description Only models whose name starts with this. */
                term?: string;
                /** @description At most this many, up to 500. */
                limit?: number;
            };
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Models. */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["Model"][];
                };
            };
            /** @description `validation_failed`: `make` is missing. */
            400: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
        };
    };
    search: {
        parameters: {
            query: {
                /**
                 * @description What someone would type: `2019 civic si`, `chevy 1500`, `f150`.
                 * @example 2019 civic si
                 */
                q: string;
                /** @description `light` (the default), `all`, or a vPIC vehicle type id. */
                scope?: string;
                /** @description At most this many, up to 50. The default is 10. */
                limit?: number;
            };
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Entries, best first. */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["Entry"][];
                };
            };
            /** @description `validation_failed`: `q` is missing. */
            400: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
        };
    };
    submodels: {
        parameters: {
            query: {
                /** @example honda */
                make: string;
                /**
                 * @description A model's name or id: `F-150`, `f150` or `f-150`.
                 * @example civic
                 */
                model: string;
                /** @example 2019 */
                year: number;
                /** @description Only submodels whose name starts with this. */
                term?: string;
            };
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Submodels. */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["Submodel"][];
                };
            };
            /** @description `validation_failed`: `make`, `model` or `year` is missing. */
            400: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
        };
    };
    trims: {
        parameters: {
            query: {
                /** @example honda */
                make: string;
                /**
                 * @description A model's name or id: `F-150`, `f150` or `f-150`.
                 * @example civic
                 */
                model: string;
                /** @example 2019 */
                year: number;
                /** @description Only submodels whose name starts with this. */
                term?: string;
            };
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Submodels. */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["Submodel"][];
                };
            };
            /** @description `validation_failed`: `make`, `model` or `year` is missing. */
            400: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
        };
    };
    years: {
        parameters: {
            query?: {
                /**
                 * @description `light` (cars, MPVs and trucks; the default), `all`, or a vPIC
                 *     vehicle type id such as `6` for trailers.
                 */
                scope?: string;
                /** @description Only years that start with this, such as `201`. */
                term?: string;
            };
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Model years, newest first. */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    /**
                     * @example [
                     *       2027,
                     *       2026,
                     *       2025
                     *     ]
                     */
                    "application/json": number[];
                };
            };
            /** @description `validation_failed`. */
            400: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
        };
    };
    entry: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                /**
                 * @description A vehicle id: year, make and model, then optionally submodel and engine, joined by underscores.
                 * @example 2019_honda_civic_si
                 */
                id: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description The entry. */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["Entry"];
                };
            };
            /** @description `not_found`: no vehicle has that id. */
            404: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
        };
    };
    vehicle_fluids: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                /**
                 * @description A vehicle id.
                 * @example 2018_ford_f-150
                 */
                id: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description The engine oil grades, by engine. */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["VehicleFluids"];
                };
            };
            /** @description `not_found`: no vehicle has that id. `dataset_unavailable`: this server holds no oil grades. */
            404: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
        };
    };
    vehicle_fuel: {
        parameters: {
            query?: {
                /**
                 * @description `us` for EPA's ratings, `ca` for Natural Resources Canada's.
                 *     Leave out for both.
                 */
                country?: string;
            };
            header?: never;
            path: {
                /**
                 * @description A vehicle id. Ratings are listed by year, make and model; a submodel or an engine in the id is set aside.
                 * @example 2018_ford_f-150
                 */
                id: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description The ratings, EPA's first, each by engine size. */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["VehicleFuel"];
                };
            };
            /** @description `validation_failed`: `country` is not `us` or `ca`. */
            400: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
            /** @description `not_found`: no vehicle has that id. `dataset_unavailable`: this server holds no fuel ratings, or none for the country asked for. */
            404: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
        };
    };
    vehicle_investigations: {
        parameters: {
            query?: {
                /**
                 * @description `open` (the default): only investigations that are open and about a
                 *     possible defect in the vehicles they list. `all`: every
                 *     investigation that lists the model year, closed ones and audit
                 *     queries included.
                 * @example open
                 */
                status?: string;
            };
            header?: never;
            path: {
                /**
                 * @description A vehicle id. Investigations are listed by year, make and model; a submodel or an engine in the id is set aside.
                 * @example 2019_ram_1500
                 */
                id: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description The investigations, newest first. An empty list when none lists the model year. */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["VehicleInvestigations"];
                };
            };
            /** @description `validation_failed`: `status` is not `open` or `all`. */
            400: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
            /** @description `not_found`: no vehicle has that id. `dataset_unavailable`: this server holds no investigations. */
            404: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
        };
    };
    vehicle_overview: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                /**
                 * @description A vehicle id. The overview is of the year, make and model; a submodel or an engine in the id is set aside.
                 * @example 2018_ford_f-150
                 */
                id: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description The overview. `decode` is `null`. */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["Overview"];
                };
            };
            /** @description `not_found`: no vehicle has that id. */
            404: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
        };
    };
    vehicle_recalls: {
        parameters: {
            query?: {
                /**
                 * @description `us` or `ca`. Left out: both, where the server holds both.
                 * @example us
                 */
                country?: string;
            };
            header?: never;
            path: {
                /**
                 * @description A vehicle id. Recalls are listed by year, make and model; a submodel or an engine in the id is set aside.
                 * @example 2018_ford_f-150
                 */
                id: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Recalls that may apply, newest first. */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["VehicleRecalls"];
                };
            };
            /** @description `validation_failed`: `country` is not `us` or `ca`. */
            400: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
            /** @description `not_found`: no vehicle has that id. `dataset_unavailable`: this server holds no recalls, or none for the country asked for. */
            404: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
        };
    };
    vehicle_safety: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                /**
                 * @description A vehicle id. Ratings are listed by year, make and model; a submodel or an engine in the id is set aside.
                 * @example 2018_ford_f-150
                 */
                id: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Every rating NHTSA gave a vehicle of the model year. */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["VehicleSafety"];
                };
            };
            /** @description `not_found`: no vehicle has that id. `dataset_unavailable`: this server holds no crash test ratings. */
            404: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
        };
    };
    vehicle_specs: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                /**
                 * @description A vehicle id.
                 * @example 2018_ford_f-150
                 */
                id: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description The specifications, by engine size. */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["VehicleSpecs"];
                };
            };
            /** @description `not_found`: no vehicle has that id. `dataset_unavailable`: this server holds no specifications. */
            404: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
        };
    };
    batch: {
        parameters: {
            query?: {
                /**
                 * @description `recalls` adds to each decode the number of recall campaigns that
                 *     may apply to the model year it reaches. Left out: nothing is added.
                 * @example recalls
                 */
                include?: string;
            };
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["BatchRequest"];
            };
        };
        responses: {
            /** @description One item per VIN, in order. */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["BatchItem"][];
                };
            };
            /** @description `validation_failed`: the body is not `{ "vins": [...] }` or lists more than 50, or `include` is not `recalls`. */
            400: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
            /** @description `dataset_unavailable`: `include=recalls` was asked of a server that holds no recalls. */
            404: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
            /** @description `payload_too_large`. */
            413: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
            /** @description `rate_limited`. */
            429: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
        };
    };
    decode: {
        parameters: {
            query?: {
                /**
                 * @description Use this model year instead of working it out from the VIN.
                 * @example 2023
                 */
                year?: number;
            };
            header?: never;
            path: {
                /**
                 * @description A 17-character VIN.
                 * @example KM8K2CAB4PU001140
                 */
                vin: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description The decode. */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["VinDecode"];
                };
            };
            /** @description `invalid_vin`: wrong length or illegal characters. `details.suggestions` lists likely corrections. */
            400: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
            /** @description `not_found`: no manufacturer is registered for the first three characters. */
            404: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
            /** @description `rate_limited`: over 600 requests a minute. See `Retry-After`. */
            429: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
        };
    };
    vin_fluids: {
        parameters: {
            query?: {
                /** @description The model year, when the VIN's tenth character leaves two possible. */
                year?: number;
            };
            header?: never;
            path: {
                /**
                 * @description A 17-character VIN.
                 * @example KM8K2CAB4PU001140
                 */
                vin: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description The engine oil grades. */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["VinFluids"];
                };
            };
            /** @description `invalid_vin`, or `validation_failed` for `year`. */
            400: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
            /** @description `not_found`: no manufacturer is registered for the first three characters. `dataset_unavailable`: this server holds no oil grades. */
            404: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
        };
    };
    vin_fuel: {
        parameters: {
            query?: {
                /** @description `us` or `ca`. Leave out for both. */
                country?: string;
                /** @description The model year, when the VIN's tenth character leaves two possible. */
                year?: number;
            };
            header?: never;
            path: {
                /**
                 * @description A 17-character VIN.
                 * @example KM8K2CAB4PU001140
                 */
                vin: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description The ratings. */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["VinFuel"];
                };
            };
            /** @description `invalid_vin`, or `validation_failed` for `country` or `year`. */
            400: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
            /** @description `not_found`: no manufacturer is registered for the first three characters. `dataset_unavailable`: this server holds no fuel ratings, or none for the country asked for. */
            404: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
        };
    };
    vin_overview: {
        parameters: {
            query?: {
                /**
                 * @description Use this model year instead of working it out from the VIN.
                 * @example 2023
                 */
                year?: number;
            };
            header?: never;
            path: {
                /**
                 * @description A 17-character VIN.
                 * @example KM8K2CAB4PU001140
                 */
                vin: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description The overview, with `decode`. */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["Overview"];
                };
            };
            /** @description `invalid_vin`, or `validation_failed` for `year`. */
            400: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
            /** @description `not_found`: no manufacturer is registered for the first three characters. */
            404: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
        };
    };
    vin_recalls: {
        parameters: {
            query?: {
                /**
                 * @description `us` or `ca`. Left out: both, where the server holds both.
                 * @example us
                 */
                country?: string;
                /**
                 * @description Use this model year instead of working it out from the VIN.
                 * @example 2023
                 */
                year?: number;
            };
            header?: never;
            path: {
                /**
                 * @description A 17-character VIN.
                 * @example KM8K2CAB4PU001140
                 */
                vin: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Recalls that may apply, newest first. */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["VinRecalls"];
                };
            };
            /** @description `invalid_vin`, or `validation_failed` for `country` or `year`. */
            400: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
            /** @description `not_found`: no manufacturer is registered for the first three characters. `dataset_unavailable`: this server holds no recalls, or none for the country asked for. */
            404: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
        };
    };
    vin_specs: {
        parameters: {
            query?: {
                /** @description The model year, when the VIN's tenth character leaves two possible. */
                year?: number;
            };
            header?: never;
            path: {
                /**
                 * @description A 17-character VIN.
                 * @example KM8K2CAB4PU001140
                 */
                vin: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description The specifications. */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["VinSpecs"];
                };
            };
            /** @description `invalid_vin`, or `validation_failed` for `year`. */
            400: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
            /** @description `not_found`: no manufacturer is registered for the first three characters. `dataset_unavailable`: this server holds no specifications. */
            404: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ErrorBody"];
                };
            };
        };
    };
}
