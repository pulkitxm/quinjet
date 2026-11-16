# Graftcode gateways

`quinjet graft` connects Quinjet to [Graftcode](https://graftcode.com)
gateways in both directions. It serves Quinjet's read commands as gateway
methods for MCP clients and typed Grafts, and it calls other gateways' tools
with repository context filled in. The
[Graftcode guide](../../guides/graftcode.md) walks through the whole setup.

```bash
quinjet graft serve --register quinjet
quinjet graft add agent http://localhost:7390
quinjet graft tools
quinjet graft call agent draft_commit_message
```

| Page | What it covers |
| --- | --- |
| [`quinjet graft serve`](./serve.md) | Host this repository's commands on a gateway |
| [`quinjet graft module`](./module.md) | Write the receiver module without starting a gateway |
| [`quinjet graft add`](./add.md) | Register a gateway under a short name |
| [`quinjet graft list`](./list.md) | List registered gateways |
| [`quinjet graft remove`](./remove.md) | Forget a registered gateway |
| [`quinjet graft tools`](./tools.md) | List the tools registered gateways expose |
| [`quinjet graft call`](./call.md) | Call a tool with repository context |

Registered gateways live in `grafts.json` under the Quinjet state directory
(`QUINJET_STATE_DIR` overrides it). Gateway calls run through the same command
layer as every other verb, and the terminal interface runs them from the
command palette on a dedicated worker lane.
