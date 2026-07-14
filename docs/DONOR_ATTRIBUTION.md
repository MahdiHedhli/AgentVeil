# Donor attribution

AgentVeil is a new focused implementation informed by three repositories owned by Mahdi Hedhli.

| Donor | Starting revision | License | Current use |
|---|---|---|---|
| PromptFence | `14ef00b9cb5a25d1622b46cbc99be71dd732fe6f` | Apache-2.0 | Architecture, baseline normalization/detector/ledger concepts and selected test scenarios; reimplemented for typed Codex fields and authenticated session scope |
| PromptGuard | `91b6cfe3a8fea2d981d9798013a8077a5601e74f` | Apache-2.0 with NOTICE | Threat model, span/overlap/token test concepts, regex attribution lessons, wire-verification methodology |
| PromptGate | `805242a4bd1cd2f354493aae8f8290065ac3b812` | Declared Apache-2.0 | Typed payload-extraction, policy ergonomics, fake-upstream and no-leak test concepts |

Ignored donor runtime state, environment files, captures, archives, benchmark corpora, and authenticated artifacts are excluded. No donor runtime data may be copied into AgentVeil.

The current `src/` implementation is a clean reimplementation informed by the donor concepts rather than a verbatim source copy. AgentVeil adds real NFKC compatibility handling, typed Codex field selection, duplicate-key rejection, authenticated-session ledger lookup, policy-enforced TTL, atomic rollback of newly issued mappings on rewrite failure, and no token-enumeration API. If source is copied or adapted later, this document and source headers must record the exact path, license, modifications, and test coverage.
