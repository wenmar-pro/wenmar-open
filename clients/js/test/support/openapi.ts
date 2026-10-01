// Reads the OpenAPI description the server commits and builds example
// responses from the examples written in it, so the client is tested against
// what the API is documented to send and nothing made up here.
import { readFileSync } from "node:fs";

/** crates/open-server/openapi.json, from .test-build/support/ or test/support/. */
const DESCRIPTION = new URL("../../../../crates/open-server/openapi.json", import.meta.url);

type Json = null | boolean | number | string | Json[] | { [key: string]: Json };

interface Schema {
  $ref?: string;
  type?: string | string[];
  example?: Json;
  properties?: Record<string, Schema>;
  items?: Schema;
  oneOf?: Schema[];
}

interface Operation {
  operationId: string;
  responses: Record<string, { content?: { "application/json"?: { schema: Schema; example?: Json } } }>;
}

interface Description {
  paths: Record<string, Record<string, Operation>>;
  components: { schemas: Record<string, Schema> };
}

export const description = JSON.parse(readFileSync(DESCRIPTION, "utf8")) as Description;

export interface Route {
  operationId: string;
  method: string;
  /** The path as the description writes it, such as `/v1/vin/{vin}`. */
  template: string;
  /** Matches a request path and captures nothing. */
  pattern: RegExp;
}

/** Every operation of the description. Fixed paths come before templates. */
export const routes: Route[] = Object.entries(description.paths)
  .flatMap(([template, methods]) =>
    Object.entries(methods).map(([method, operation]) => ({
      operationId: operation.operationId,
      method: method.toUpperCase(),
      template,
      pattern: new RegExp(
        `^${template.replace(/[.*+?^$()|[\]\\]/g, "\\$&").replace(/\{[^}]+\}/g, "[^/]+")}$`,
      ),
    })),
  )
  .sort((left, right) => Number(left.template.includes("{")) - Number(right.template.includes("{")));

function resolve(schema: Schema): Schema {
  if (schema.$ref === undefined) return schema;
  const name = schema.$ref.replace("#/components/schemas/", "");
  const found = description.components.schemas[name];
  if (found === undefined) throw new Error(`no schema named ${name}`);
  return found;
}

/**
 * An example value for a schema: its own `example` where it has one, else
 * one built from its properties. Every property is included, required or
 * not, so the client is shown the fullest answer the description allows.
 */
export function example(given: Schema): Json {
  const schema = resolve(given);
  if (schema.example !== undefined) return schema.example;
  if (schema.oneOf !== undefined) {
    const chosen = schema.oneOf.find((option) => option.type !== "null");
    if (chosen === undefined) return null;
    return example(chosen);
  }
  const types = Array.isArray(schema.type) ? schema.type : [schema.type];
  const type = types.find((name) => name !== "null");
  switch (type) {
    case "object": {
      const value: { [key: string]: Json } = {};
      for (const [name, property] of Object.entries(schema.properties ?? {})) {
        value[name] = example(property);
      }
      return value;
    }
    case "array":
      return schema.items === undefined ? [] : [example(schema.items)];
    case "string":
      return "text";
    case "integer":
      return 2019;
    case "number":
      return 1.5;
    case "boolean":
      return true;
    default:
      throw new Error(`no example for a schema of type ${String(type)}`);
  }
}

/** The documented body of an operation's answer with the given status. */
export function exampleResponse(operationId: string, status = "200"): Json {
  for (const methods of Object.values(description.paths)) {
    for (const operation of Object.values(methods)) {
      if (operation.operationId !== operationId) continue;
      const content = operation.responses[status]?.content?.["application/json"];
      if (content === undefined) throw new Error(`${operationId} documents no ${status} body`);
      return content.example ?? example(content.schema);
    }
  }
  throw new Error(`no operation named ${operationId}`);
}

/** The API's error body with the given code. */
export function errorBody(code: string, message: string, details: { [key: string]: Json } = {}): Json {
  return { error: { code, message, details } };
}
