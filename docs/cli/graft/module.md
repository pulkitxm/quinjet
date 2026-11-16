# `quinjet graft module`

Writes the Graftcode receiver that `graft serve` would host, without starting
a gateway.

```bash
quinjet graft module --out ./quinjet-graft
quinjet graft module --out ./quinjet-graft --allow-writes --json
```

The directory receives `package.json` and `index.js`. `index.js` declares one
`Quinjet` class with a static method per command, documented from the
command's help, and runs `quinjet <command> --json` with the arguments it is
given. Optional arguments take `""`, switches take `true` or `false`, and list
arguments take space-separated values or an array.

Host it with `gg --runtime nodejs --modules ./quinjet-graft/index.js --types
Quinjet`, setting `QUINJET_GRAFT_REPO` to the repository and, when `quinjet`
is not on `PATH`, `QUINJET_GRAFT_BIN` to the executable.

`--json` prints the directory and every tool with its command path, access,
and parameters.
