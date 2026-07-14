# Codex Responses payload map

Evidence anchor: commit `ed09354`, Codex CLI `0.144.4`, model
`gpt-5.6-luna`, HTTP `POST /v1/responses`, `stream: true`.

This map describes what the current adapter accepts and how it handles every
accepted string. It is not a promise that future Codex versions use the same
shape.

## Handling modes

| Mode | Meaning |
|---|---|
| Rewrite | scan the string; apply configured mask/tokenize actions to exact source spans; any block finding blocks the whole request |
| Block-only | scan the string but never alter it; any non-allow finding blocks the whole request to preserve protocol integrity |
| Typed control | validate its JSON type/value; every nested string and object key not already classified is still added as block-only structural text |
| Reject | return a local error before the upstream request body is created |

Every accepted JSON string value is a Rewrite or Block-only target. Every JSON
object key, including nested schema keys, is separately scanned as Block-only
structural text. Numeric, boolean, and null values are type/value validated but
do not contain text to scan.

An exact, unexpired AgentVeil token owned by the current session is allowed on
outbound replay. An unknown, expired, evicted, or cross-session AgentVeil token
is a hard block.

## Top-level request

Unknown top-level fields reject. Duplicate keys at any JSON object depth reject
during parsing.

| Path | Required shape/value | Handling |
|---|---|---|
| `$.model` | string, required | Block-only structural text; launcher separately pins `gpt-5.6-luna` |
| `$.instructions` | string, optional | Rewrite as `instructions` |
| `$.input` | string or array, required | raw string is Rewrite as message text; array uses item map below |
| `$.tools` | array, optional | recursively Block-only; tool descriptions/schema strings receive typed source classes |
| `$.tool_choice` | string `auto`, required | Block-only plus exact value validation |
| `$.parallel_tool_calls` | boolean, required | Typed control |
| `$.reasoning` | null or object, optional | only `effort`, `summary`, `context`; each optional value is string/null and Block-only |
| `$.store` | boolean `false`, required | Exact value validation |
| `$.stream` | boolean `true`, required | Exact value validation |
| `$.stream_options` | object, optional | only required string `reasoning_summary_delivery`; Block-only |
| `$.include` | array, optional | at most four strings, each exactly `reasoning.encrypted_content`; Block-only |
| `$.service_tier` | string/null, optional | Block-only |
| `$.prompt_cache_key` | string/null, optional | Block-only |
| `$.text` | null or object, optional | typed control map below; strings/keys Block-only |
| `$.client_metadata` | object, optional | every value must be a string and is Block-only as client metadata; keys Block-only |

`$.text` permits only:

- optional string/null `verbosity`; and
- optional `format` object whose required string `type` is either `text` (no
  other keys) or `json_schema` (required boolean `strict`, required string
  `name`, and required `schema` value).

The adapter does not interpret the JSON Schema semantically, but recursively
scans every string value and object key as Block-only structural text.

## Input items

All common identifiers and metadata strings that are not explicitly Rewrite
targets are caught by the final Block-only structural pass.

| `$.input[*].type` | Accepted content | Handling |
|---|---|---|
| `message` | required string `role`; `content` string or content-parts array; optional string/null `id`, `phase`, one typed metadata object | content string and `input_text`/`output_text` `.text` are Rewrite as message text |
| `agent_message` | required string `author` and `recipient`; required content array; optional `id` and typed metadata | `input_text.text` is Rewrite as agent-message text; `encrypted_content` string is Block-only |
| `function_call_output` | required string `call_id`; `output` string or parts array; optional `id` and typed metadata | string/`input_text.text` output is Rewrite as function output; encrypted part is Block-only |
| `custom_tool_call_output` | same output rules; required `call_id`; optional string/null `name`, `id`, typed metadata | Rewrite as custom-tool output; encrypted part is Block-only |
| `function_call` | required string `name`, `call_id`, and `arguments`; optional string/null `id`, `namespace`, typed metadata | `arguments` is Rewrite as function arguments; identifiers are Block-only |
| `custom_tool_call` | required string `name`, `call_id`, and `input`; optional string/null `id`, `status`, `namespace`, typed metadata | `input` is Rewrite as custom-tool input; identifiers are Block-only |
| `additional_tools` | required string `role` and required tool array; optional string/null `id` | tool tree strings are Block-only |
| `reasoning` | optional/null arrays `summary` and `content`; each part is `summary_text` or `reasoning_text`; optional encrypted string/null and typed metadata | part `.text` is Rewrite as reasoning summary; encrypted/control text is Block-only |
| `local_shell_call` | known type/common metadata plus arbitrary nested fields | all non-common nested strings are Block-only as function arguments; common strings/keys are structural Block-only |
| `tool_search_call` | same rule as `local_shell_call` | Block-only nested text |
| `tool_search_output` | same rule as `local_shell_call` | Block-only nested text |
| `web_search_call` | same rule as `local_shell_call` | Block-only nested text |
| `compaction` | encrypted string required; optional `id` and typed metadata | encrypted/control text Block-only; no decryption |
| `compaction_summary` | encrypted string required; optional `id` and typed metadata | encrypted/control text Block-only; no decryption |
| `context_compaction` | encrypted string/null optional; optional `id` and typed metadata | encrypted/control text Block-only; no decryption |
| `compaction_trigger` | encrypted content forbidden; optional `id` and typed metadata | control text Block-only |
| `image_generation_call` | any | Reject as media |
| any other item type | any | Reject |

The typed metadata object is
`internal_chat_message_metadata_passthrough`. It may be null or an object with
only optional string/null `turn_id`; its text and keys are Block-only.

## Content parts

| Parent | Part type | Allowed fields | Handling |
|---|---|---|---|
| `message.content[*]` | `input_text`, `output_text` | exactly `type`, string `text` | Rewrite using parent message class |
| `agent_message.content[*]` | `input_text` | exactly `type`, string `text` | Rewrite as agent-message text |
| tool output `output[*]` | `input_text` | exactly `type`, string `text` | Rewrite using function/custom-tool output class |
| agent/tool output | `encrypted_content` | exactly `type`, string `encrypted_content` | Block-only; not decrypted |
| reasoning summary/content | `summary_text`, `reasoning_text` | exactly `type`, string `text` | Rewrite as reasoning summary |
| message/tool output | `input_image` | any | Reject |
| message | `input_file` | any | Reject |
| any parent | unknown part | any | Reject |

## Tool definitions and schemas

Top-level `tools` and `additional_tools[*].tools` must be arrays. Their nested
JSON can contain objects, arrays, scalars, and nulls. Every string is scanned:

- `description` strings are classified as tool description;
- other non-identifier schema strings are classified as tool schema text; and
- names, types, namespaces, IDs, references, all other strings, and every object
  key are ultimately structural Block-only targets.

No tool-definition string is rewritten. A detected email/private IP/custom term
in a tool schema therefore blocks the whole request instead of mutating the
schema.

## Request-wide rejection rules

The gateway rejects locally for:

- non-object JSON root, invalid JSON, or any duplicate object key;
- body larger than the policy maximum (4 MiB under the default policy);
- content type other than `application/json`;
- content encoding other than absent/identity;
- missing/wrong-typed required fields or unsupported exact control values;
- unknown top-level field, unknown item type, unknown content part, or extra
  field in a strict item/part/control object;
- media/file/image content;
- normalization/source-map/view-limit failure;
- a protected finding in any Block-only string/key; or
- rewrite, ledger, randomness, or serialization failure.

For typed structural items (`local_shell_call`, tool-search items, and
`web_search_call`), arbitrary nested keys are accepted but every string/key is
Block-only. That is narrower in functionality than rewriting them, but ensures
a supported finding cannot silently pass.

## Inbound Responses SSE

Live OpenAI mode performs no restoration. The table below applies only to the
explicit loopback synthetic-test mode.

| SSE JSON `type` | Potential restoration sink | Other fields |
|---|---|---|
| `response.output_text.delta` | string `delta` | unchanged |
| `response.output_text.done` | string `text` | unchanged |
| `response.content_part.added`, `response.content_part.done` | `part.text` only when `part.type == output_text` | unchanged |
| `response.output_item.added`, `response.output_item.done` | `item.content[*].text` only when `item.type == message` and part is `output_text` | unchanged |
| `response.created`, `response.in_progress`, `response.completed` | same message/output-text fields under `response.output[*]` | unchanged |
| every other event, including function/tool arguments | none | entire frame unchanged |

Restoration requires an exact token owned by the same live session and within
TTL. SSE event type/header disagreement, invalid JSON/UTF-8, multiple data lines,
unsupported text shape, an incomplete frame over 1 MiB, or truncated stream
fails the local synthetic stream. Transport byte fragmentation is tested at
every split; a token divided across separate semantic delta events is not
reassembled.

## Evidence labels

- The custom provider’s Responses route, streaming, message/additional-tool
  structure, normal auth forwarding, and GPT-5.6 Luna response were observed on
  a live Codex `0.144.4` route.
- Function/custom-tool output protection is covered by engine and fake-upstream
  tests; a synthetic custom-tool-output next turn also completed on the live
  route.
- Every accepted-string classification, unknown top-level/item/content/media
  rejection, duplicate-key rejection, structural-key block, metadata block,
  and composed-bypass block has automated evidence.
- Accepted shapes not exercised by a live Codex turn are adapter support, not a
  claim that Codex currently emits them.

Any Codex upgrade or newly observed field must remain rejected until this map,
typed adapter, test fixture, fake-upstream proof, and live evidence are updated
together.
