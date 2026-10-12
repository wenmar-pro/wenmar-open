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
    batch: {
        parameters: {
            query?: never;
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
            /** @description `validation_failed`: the body is not `{ "vins": [...] }` or lists more than 50. */
            400: {
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
}
