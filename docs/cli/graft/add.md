# `quinjet graft add`

Registers a Graftcode gateway under a short name, or updates the one with that
name.

```bash
quinjet graft add agent http://localhost:7390
quinjet graft add review http://localhost:7400/mcp --token-env REVIEW_TOKEN
```

The address may be the gateway root, its Vision page (`/GV`), its WebSocket
(`/ws`), or its MCP endpoint; Quinjet stores the MCP endpoint. Names use
letters, digits, `-`, and `_`, up to 40 characters.

`--token-env NAME` makes every call send the value of the environment variable
`NAME` as a bearer token. Only the variable's name is stored.

Addresses are plain HTTP. An `https://` address is rejected with a hint to
reach that gateway through an SSH tunnel such as
`ssh -N -L 7390:localhost:7390 <host>` and add `http://localhost:7390`.

Exit code 1 means the name or address was rejected.
