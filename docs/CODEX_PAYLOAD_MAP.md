# Codex Responses payload map

Evidence target: Codex CLI `0.144.4`, HTTP Responses/SSE, GPT-5.6. This map distinguishes observation from implementation and verification.

| JSON path / item | Semantic source | Model-visible | Planned handling | Evidence |
|---|---|---:|---|---|
| `$.input[*].type = message` | Developer/user/prior message | yes | Scan and rewrite supported text content | Observed structurally; adapter tests pending |
| `$.input[*].content[*].type = input_text` | Text content block | yes | Scan/rewrite `text` only | Observed structurally; adapter tests pending |
| `$.input[*].type = additional_tools` | Codex dynamic tool definitions | yes | Preserve names/schema; scan descriptions conservatively and block findings | Observed structurally; exact shape fixture pending |
| `$.instructions` | Provider/developer instruction text when present | yes | Scan/rewrite string | Documented; live observation pending |
| `$.input[*].type = function_call_output` | Tool/function result replay | yes | Scan/rewrite supported string output | Required contract/live gate pending |
| `$.input[*].type = function_call` | Prior model function arguments | possibly replayed | Scan supported JSON-string arguments; never restore into arguments | Contract gate pending |
| `$.tools[*]` | Tool names, descriptions, parameter schema | yes | Never rewrite names/schema; reject protected findings in descriptions/opaque schema text | Observed tool names only; fixture pending |
| `$.model`, IDs, numeric settings, protocol flags | Protocol/configuration | no or integrity-sensitive | Never rewrite | Observed |
| Binary/image/encrypted/opaque fields | Unsupported content | unknown | Reject when model-visible inspection cannot be proven | Not supported |

Observed fake-upstream structural evidence contained a `POST /v1/responses`, `stream: true`, message and `additional_tools` input items, and tool declarations including `exec`, `request_user_input`, and `wait`. Only header names and structural metadata were recorded; no auth or body values were persisted.

Inbound restoration is limited to exact AgentVeil-issued tokens inside verified assistant display-text events such as `response.output_text.delta`. Tool arguments, commands, URLs, file patches, IDs, reasoning metadata, errors, audit, dashboard, and unknown events are never restoration sinks.
