// Generated from crates/open-server/openapi.json by scripts/generate.mjs.
// Do not edit. Run: npm run generate

export interface paths {
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
             *     `not_found`, `method_not_allowed`, `payload_too_large`,
             *     `uri_too_long`, `rate_limited`, `unavailable`, `internal_error`.
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
}
