# Graftcode integration

This directory holds the parts of Quinjet's [Graftcode](https://graftcode.com)
integration that are not Rust.

- [`agent`](./agent) is a Python Graftcode receiver. Its methods draft commit
  messages, review patches, explain failed checks, and triage pull requests by
  calling Quinjet's grafted tools. Start it with `agent/serve.sh`.

The Rust side lives in `src/graft` and the `quinjet graft` verbs. The
[Graftcode guide](../docs/guides/graftcode.md) covers both.
