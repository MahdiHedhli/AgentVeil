#!/usr/bin/env node

import http from "node:http";
import https from "node:https";

const host = "127.0.0.1";
const port = Number.parseInt(process.env.AGENTVEIL_SPIKE_PORT ?? "48742", 10);
const maxRequestBytes = 8 * 1024 * 1024;
const hopByHop = new Set([
  "connection",
  "keep-alive",
  "proxy-authenticate",
  "proxy-authorization",
  "te",
  "trailer",
  "transfer-encoding",
  "upgrade",
  "host",
  "content-length",
  "cookie",
]);

function targetFor(req) {
  const chatgpt = typeof req.headers["chatgpt-account-id"] === "string";
  const requestUrl = new URL(req.url ?? "/", `http://${host}`);

  if (requestUrl.pathname === "/v1/responses") {
    return new URL(
      chatgpt
        ? "https://chatgpt.com/backend-api/codex/responses"
        : "https://api.openai.com/v1/responses",
    );
  }

  if (requestUrl.pathname === "/v1/models") {
    const target = new URL(
      chatgpt
        ? "https://chatgpt.com/backend-api/codex/models"
        : "https://api.openai.com/v1/models",
    );
    target.search = requestUrl.search;
    return target;
  }

  return null;
}

function forwardHeaders(headers) {
  return Object.fromEntries(
    Object.entries(headers).filter(
      ([name, value]) => !hopByHop.has(name.toLowerCase()) && value !== undefined,
    ),
  );
}

const server = http.createServer((req, res) => {
  const target = targetFor(req);
  if (!target) {
    res.writeHead(404, { "content-type": "application/json" });
    res.end(JSON.stringify({ error: { message: "unsupported Codex route" } }));
    return;
  }

  let requestBytes = 0;
  let responseBytes = 0;
  const upstream = https.request(
    target,
    {
      method: req.method,
      headers: forwardHeaders(req.headers),
    },
    (upstreamResponse) => {
      const headers = Object.fromEntries(
        Object.entries(upstreamResponse.headers).filter(
          ([name, value]) => !hopByHop.has(name.toLowerCase()) && value !== undefined,
        ),
      );
      res.writeHead(upstreamResponse.statusCode ?? 502, headers);
      upstreamResponse.on("data", (chunk) => {
        responseBytes += chunk.length;
      });
      upstreamResponse.pipe(res);
      upstreamResponse.on("end", () => {
        process.stdout.write(
          `${JSON.stringify({
            route: target.pathname,
            auth_mode: typeof req.headers["chatgpt-account-id"] === "string" ? "chatgpt" : "api",
            authorization_present: typeof req.headers.authorization === "string",
            request_bytes: requestBytes,
            response_bytes: responseBytes,
            status: upstreamResponse.statusCode ?? 502,
          })}\n`,
        );
      });
    },
  );

  upstream.on("error", () => {
    if (!res.headersSent) {
      res.writeHead(502, { "content-type": "application/json" });
    }
    res.end(JSON.stringify({ error: { message: "upstream connection failed" } }));
  });

  req.on("data", (chunk) => {
    requestBytes += chunk.length;
    if (requestBytes > maxRequestBytes) {
      upstream.destroy();
      req.destroy(new Error("request too large"));
    }
  });
  req.pipe(upstream);
});

server.listen(port, host, () => {
  process.stdout.write(`${JSON.stringify({ ready: true, host, port, mode: "passthrough-spike" })}\n`);
});

for (const signal of ["SIGINT", "SIGTERM"]) {
  process.on(signal, () => server.close(() => process.exit(0)));
}
