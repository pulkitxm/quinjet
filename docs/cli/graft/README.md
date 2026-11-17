# Graftcode gateways

`quinjet graft` runs Quinjet on [Graftcode](https://graftcode.com) gateways and
calls other gateways' tools with repository context filled in. Most people
never type it: agents start [`quinjet mcp`](../mcp/README.md), which starts the
shared gateway on its own. The [Graftcode guide](../../guides/graftcode.md)
walks through the whole picture.

```bash
quinjet graft status
quinjet graft serve --publish
quinjet graft add review http://localhost:7400
quinjet graft call review summarize --pr 42
```

| Page | What it covers |
| --- | --- |
| [`quinjet graft start`, `status`, `stop`, `install`](./start.md) | The shared gateway behind `quinjet mcp` and the terminal interface |
| [`quinjet graft serve`](./serve.md) | A foreground gateway for one repository, for typed Grafts and other hosts |
| [`quinjet graft module`](./module.md) | Write the receiver module without starting a gateway |
| [`quinjet graft add`](./add.md) | Register another gateway under a short name |
| [`quinjet graft list`](./list.md) | List registered gateways |
| [`quinjet graft remove`](./remove.md) | Forget a registered gateway |
| [`quinjet graft tools`](./tools.md) | List the tools registered gateways expose |
| [`quinjet graft call`](./call.md) | Call a tool with repository context |

Registered gateways live in `grafts.json` under the Quinjet state directory
(`QUINJET_STATE_DIR` overrides it). Gateway calls run through the same command
layer as every other verb, and the terminal interface runs them from the
command palette on a dedicated worker lane.
