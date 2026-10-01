// Not run: compiled. This file is an application using the package the way
// its README shows, under the strictest compiler settings, against the built
// declarations in dist/. If the public surface stops compiling for a
// consumer, `npm run test:types` fails.
import { MAX_BATCH, WenmarOpen, WenmarOpenError, isBatchError } from "wenmar-open";
import type {
  BatchItem,
  ClientOptions,
  EngineOption,
  Entry,
  ErrorCode,
  FetchLike,
  Make,
  Meta,
  Model,
  Submodel,
  VinDecode,
} from "wenmar-open";

const options: ClientOptions = { baseUrl: "http://localhost:3000", timeoutMs: 2_000 };
const client = new WenmarOpen(options);
const hosted: WenmarOpen = new WenmarOpen();

// The platform's fetch fits the option, in a browser and in Node.
const platform: FetchLike = fetch;
export const withFetch = new WenmarOpen({ fetch: platform });

export async function everyMethod(signal: AbortSignal): Promise<void> {
  const decode: VinDecode = await client.decodeVin("KM8K2CAB4PU001140", { year: 2023, signal, timeoutMs: 500 });
  const vin: string = decode.vin;
  const valid: boolean = decode.valid;
  const year: number | null | undefined = decode.year;
  const label: string | null | undefined = decode.engine?.label;
  const vehicleId: string | undefined = decode.catalog?.vehicle_id;
  const warning: string | undefined = decode.warnings[0]?.code;

  const items: BatchItem[] = await client.decodeVins(["KM8K2CAB4PU001140"], { signal });
  for (const item of items) {
    if (isBatchError(item)) {
      const code: string = item.error.code;
      const details: Record<string, unknown> = item.error.details;
      void [code, details];
    } else {
      const make: string | null | undefined = item.make;
      void make;
    }
  }

  const years: number[] = await hosted.years();
  const makes: Make[] = await client.makes({ year: 2019, term: "ho", limit: 10 });
  const models: Model[] = await client.models({ make: "honda" });
  const submodels: Submodel[] = await client.submodels({ make: "honda", model: "civic", year: 2019 });
  const trims: Submodel[] = await client.trims({ make: "honda", model: "civic", year: 2019 });
  const engines: EngineOption[] = await client.engines({ make: "honda", model: "civic", year: 2019, submodel: "si" });
  const found: Entry[] = await client.search({ q: "2019 civic si" });
  const entry: Entry = await client.vehicle("2019_honda_civic_si");
  const meta: Meta = await client.meta({ signal });
  const batch: 50 = MAX_BATCH;

  void [vin, valid, year, label, vehicleId, warning, years, makes, models, submodels, trims, engines, found, entry, meta, batch];
}

export function everyError(error: unknown): number {
  if (!(error instanceof WenmarOpenError)) throw error;
  const code: ErrorCode = error.code;
  const status: number | undefined = error.status;
  const details: Record<string, unknown> = error.details;
  const message: string = error.message;
  void [code, status, details, message];
  // Known codes are checked as literals; a code added later is still a string.
  if (error.code === "rate_limited" || error.code === "unavailable") return error.retryAfter ?? 60;
  if (error.code === "timeout" || error.code === "network") return 1;
  if (error.code === "a_code_added_later") return 2;
  return 0;
}

export async function mistakes(): Promise<void> {
  // @ts-expect-error a model list needs a make
  await client.models({});
  // @ts-expect-error a submodel list needs a year
  await client.submodels({ make: "honda", model: "civic" });
  // @ts-expect-error the year is a number
  await client.decodeVin("KM8K2CAB4PU001140", { year: "2023" });
  // @ts-expect-error a search needs its text
  await client.search({});
  // @ts-expect-error there is no such option
  new WenmarOpen({ apiKey: "none-is-needed" });
  const decode = await client.decodeVin("KM8K2CAB4PU001140");
  // @ts-expect-error the API has no such field
  void decode.colour;
  // @ts-expect-error an error's fields cannot be assigned
  new WenmarOpenError({ code: "network", message: "m" }).code = "timeout";
}
