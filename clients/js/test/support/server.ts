// An HTTP server on 127.0.0.1 that answers like the API: every route of the
// OpenAPI description, with the example the description gives for it. A test
// may queue a different answer for the next request. Nothing here reaches
// the network.
import { createServer } from "node:http";
import type { IncomingHttpHeaders, Server } from "node:http";
import type { AddressInfo } from "node:net";

import { errorBody, exampleResponse, routes } from "./openapi.js";

/** A request the server received, as it arrived. */
export interface Received {
  method: string;
  /** The path and query string exactly as sent, before any decoding. */
  url: string;
  headers: IncomingHttpHeaders;
  body: string;
}

/** An answer queued for the next request. */
export interface Reply {
  status?: number;
  headers?: Record<string, string>;
  /** Sent as it is if a string, as JSON otherwise. */
  body?: unknown;
  /** Never answer. The connection stays open until the server closes. */
  hang?: boolean;
  /** Send the headers and part of the body, then stall. */
  stall?: boolean;
}

export class ApiServer {
  readonly received: Received[] = [];
  readonly #server: Server;
  readonly #queue: Reply[] = [];

  private constructor(server: Server) {
    this.#server = server;
  }

  static async start(): Promise<ApiServer> {
    const server = createServer();
    const api = new ApiServer(server);
    server.on("request", (request, response) => {
      const chunks: Buffer[] = [];
      request.on("data", (chunk: Buffer) => chunks.push(chunk));
      request.on("end", () => {
        api.received.push({
          method: request.method ?? "",
          url: request.url ?? "",
          headers: request.headers,
          body: Buffer.concat(chunks).toString("utf8"),
        });
        const reply = api.#queue.shift() ?? api.#documented(request.method ?? "", request.url ?? "");
        if (reply.hang === true) return;
        const text = typeof reply.body === "string" ? reply.body : JSON.stringify(reply.body);
        response.writeHead(reply.status ?? 200, {
          "content-type": "application/json",
          "x-data-version": "2026.09",
          ...reply.headers,
        });
        if (reply.stall === true) {
          response.write(text.slice(0, 1));
          return;
        }
        response.end(text);
      });
    });
    await new Promise<void>((resolve) => server.listen(0, "127.0.0.1", resolve));
    return api;
  }

  /** `http://127.0.0.1:<port>`, with no trailing slash. */
  get url(): string {
    const { port } = this.#server.address() as AddressInfo;
    return `http://127.0.0.1:${port}`;
  }

  /** The next request gets this answer instead of the documented one. */
  next(reply: Reply): void {
    this.#queue.push(reply);
  }

  /** The most recent request. */
  get last(): Received {
    const last = this.received.at(-1);
    if (last === undefined) throw new Error("the server received no request");
    return last;
  }

  async close(): Promise<void> {
    this.#server.closeAllConnections();
    await new Promise<void>((resolve) => this.#server.close(() => resolve()));
  }

  #documented(method: string, url: string): Reply {
    const path = url.split("?")[0] ?? "";
    const route = routes.find((candidate) => candidate.method === method && candidate.pattern.test(path));
    if (route === undefined) {
      return { status: 404, body: errorBody("not_found", "There is nothing at this address.") };
    }
    return { body: exampleResponse(route.operationId) };
  }
}
