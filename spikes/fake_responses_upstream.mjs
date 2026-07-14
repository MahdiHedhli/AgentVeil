#!/usr/bin/env node

import http from "node:http";

const host = "127.0.0.1";
const port = Number.parseInt(process.env.AGENTVEIL_SPIKE_PORT ?? "48741", 10);

function collectPaths(value, prefix = "$", output = new Set()) {
  if (Array.isArray(value)) {
    output.add(`${prefix}[]`);
    for (const item of value) collectPaths(item, `${prefix}[]`, output);
    return output;
  }

  if (value && typeof value === "object") {
    for (const [key, child] of Object.entries(value)) {
      const path = `${prefix}.${key}`;
      output.add(path);
      collectPaths(child, path, output);
    }
  }

  return output;
}

function responseShape(id, status, output = [], usage = null) {
  return {
    id,
    object: "response",
    created_at: Math.floor(Date.now() / 1000),
    status,
    background: false,
    error: null,
    incomplete_details: null,
    instructions: null,
    max_output_tokens: null,
    max_tool_calls: null,
    model: "gpt-5.6-luna",
    output,
    parallel_tool_calls: true,
    previous_response_id: null,
    prompt: null,
    prompt_cache_key: null,
    reasoning: { effort: "medium", summary: null },
    safety_identifier: null,
    service_tier: "default",
    store: false,
    temperature: 1,
    text: { format: { type: "text" }, verbosity: "medium" },
    tool_choice: "auto",
    tools: [],
    top_logprobs: 0,
    top_p: 1,
    truncation: "disabled",
    usage,
    user: null,
    metadata: {},
  };
}

function writeEvent(res, event) {
  res.write(`event: ${event.type}\n`);
  res.write(`data: ${JSON.stringify(event)}\n\n`);
}

function streamResponse(res) {
  const responseId = `resp_agentveil_spike_${Date.now()}`;
  const itemId = `msg_agentveil_spike_${Date.now()}`;
  const text = "AGENTVEIL_SPIKE_OK";
  const messageInProgress = {
    id: itemId,
    type: "message",
    status: "in_progress",
    role: "assistant",
    content: [],
  };
  const contentInProgress = { type: "output_text", annotations: [], logprobs: [], text: "" };
  const contentDone = { ...contentInProgress, text };
  const messageDone = { ...messageInProgress, status: "completed", content: [contentDone] };

  res.writeHead(200, {
    "content-type": "text/event-stream; charset=utf-8",
    "cache-control": "no-cache",
    connection: "keep-alive",
    "x-agentveil-spike": "local-only",
  });

  writeEvent(res, {
    type: "response.created",
    sequence_number: 0,
    response: responseShape(responseId, "in_progress"),
  });
  writeEvent(res, {
    type: "response.in_progress",
    sequence_number: 1,
    response: responseShape(responseId, "in_progress"),
  });
  writeEvent(res, {
    type: "response.output_item.added",
    sequence_number: 2,
    output_index: 0,
    item: messageInProgress,
  });
  writeEvent(res, {
    type: "response.content_part.added",
    sequence_number: 3,
    item_id: itemId,
    output_index: 0,
    content_index: 0,
    part: contentInProgress,
  });
  writeEvent(res, {
    type: "response.output_text.delta",
    sequence_number: 4,
    item_id: itemId,
    output_index: 0,
    content_index: 0,
    delta: text,
    logprobs: [],
  });
  writeEvent(res, {
    type: "response.output_text.done",
    sequence_number: 5,
    item_id: itemId,
    output_index: 0,
    content_index: 0,
    text,
    logprobs: [],
  });
  writeEvent(res, {
    type: "response.content_part.done",
    sequence_number: 6,
    item_id: itemId,
    output_index: 0,
    content_index: 0,
    part: contentDone,
  });
  writeEvent(res, {
    type: "response.output_item.done",
    sequence_number: 7,
    output_index: 0,
    item: messageDone,
  });
  writeEvent(res, {
    type: "response.completed",
    sequence_number: 8,
    response: responseShape(responseId, "completed", [messageDone], {
      input_tokens: 1,
      input_tokens_details: { cached_tokens: 0 },
      output_tokens: 1,
      output_tokens_details: { reasoning_tokens: 0 },
      total_tokens: 2,
    }),
  });
  res.end();
}

const server = http.createServer((req, res) => {
  if (req.method === "GET" && req.url === "/health") {
    res.writeHead(200, { "content-type": "application/json" });
    res.end(JSON.stringify({ status: "ok", binding: "loopback" }));
    return;
  }

  if (req.method !== "POST" || !req.url?.endsWith("/responses")) {
    res.writeHead(404, { "content-type": "application/json" });
    res.end(JSON.stringify({ error: { message: "not found", type: "invalid_request_error" } }));
    return;
  }

  const chunks = [];
  let total = 0;
  req.on("data", (chunk) => {
    total += chunk.length;
    if (total > 8 * 1024 * 1024) {
      req.destroy(new Error("request too large"));
      return;
    }
    chunks.push(chunk);
  });
  req.on("end", () => {
    let body;
    try {
      body = JSON.parse(Buffer.concat(chunks).toString("utf8"));
    } catch {
      res.writeHead(400, { "content-type": "application/json" });
      res.end(JSON.stringify({ error: { message: "invalid JSON", type: "invalid_request_error" } }));
      return;
    }

    const evidence = {
      method: req.method,
      path: req.url,
      content_type: req.headers["content-type"] ?? null,
      authorization_present: typeof req.headers.authorization === "string",
      header_names: Object.keys(req.headers)
        .map((name) => name.toLowerCase())
        .sort(),
      request_bytes: total,
      model: typeof body.model === "string" ? body.model : null,
      stream: body.stream === true,
      input_item_types: Array.isArray(body.input)
        ? [...new Set(body.input.map((item) => item?.type).filter((type) => typeof type === "string"))].sort()
        : [],
      tool_names: Array.isArray(body.input)
        ? [
            ...new Set(
              body.input
                .flatMap((item) => (Array.isArray(item?.tools) ? item.tools : []))
                .map((tool) => tool?.name)
                .filter((name) => typeof name === "string"),
            ),
          ].sort()
        : [],
      json_paths: [...collectPaths(body)].sort(),
    };
    process.stdout.write(`${JSON.stringify(evidence)}\n`);
    streamResponse(res);
  });
});

server.listen(port, host, () => {
  process.stdout.write(`${JSON.stringify({ ready: true, host, port })}\n`);
});

for (const signal of ["SIGINT", "SIGTERM"]) {
  process.on(signal, () => server.close(() => process.exit(0)));
}
