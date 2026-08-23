# `quinjet remote forget`

Removes recent SSH repositories from local Quinjet state.

```bash
quinjet remote forget tuf-wired
quinjet remote forget tuf-wired --only-folder ~/src/project
```

Without `--only-folder`, every recent repository for the target is removed.
With a folder, only that target and folder pair is removed.
