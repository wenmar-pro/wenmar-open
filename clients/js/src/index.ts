export {
  DEFAULT_BASE_URL,
  DEFAULT_TIMEOUT_MS,
  MAX_BATCH,
  WenmarOpen,
  isBatchError,
} from "./client.js";
export type {
  ClientOptions,
  DecodeOptions,
  FetchInit,
  FetchLike,
  FetchResponse,
  RequestOptions,
} from "./client.js";
export { WenmarOpenError } from "./errors.js";
export type { ApiErrorCode, ClientErrorCode, ErrorCode, WenmarOpenErrorInit } from "./errors.js";
export type {
  BatchItem,
  CheckDigit,
  Engine,
  EngineOption,
  EnginesQuery,
  Entry,
  ErrorBody,
  Make,
  MakesQuery,
  Manufacturer,
  Meta,
  Model,
  ModelsQuery,
  Plant,
  Safety,
  SearchQuery,
  Selection,
  Submodel,
  SubmodelsQuery,
  VinDecode,
  Warning,
  YearsQuery,
} from "./types.js";
export type { components, operations, paths } from "./schema.js";
