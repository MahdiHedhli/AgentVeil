# Generated evidence

This tracked directory is the stable parent for private, Git-ignored release
evidence. AgentVeil creates `reports/generated/` only when needed, with mode
`0700`, and writes value-free reports there as mode `0600` files.

Do not commit generated evidence. Do not place request bodies, credentials,
protected values, token mappings, authorization material, personal paths, or
donor runtime state in this tree.
