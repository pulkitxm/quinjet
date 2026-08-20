# Caching: Correctness by Construction

Quinjet keeps a single on-disk cache for everything it learns from GitHub and for the expensive
byte streams Git produces while serving a pull request. The cache is designed so that a stale
answer is not merely unlikely but structurally impossible for most of its contents: entries whose
key already names their content never expire, and only the handful of genuinely time-varying reads
carry a clock. This page walks through that design from the bottom up: the `CacheLife` split, every
cache key in the codebase, the byte-level entry format and hashed filenames, atomic writes and
private modes, eviction, the stale-fallback path, the `cached` indicator, and the staleness bugs an
adversarial review found in early versions of these keys and how the design absorbed the fixes.

## Contents

- [The problem with caches](#the-problem-with-caches)
- [The CacheLife split](#the-cachelife-split)
- [Why OID-keyed entries can never go stale](#why-oid-keyed-entries-can-never-go-stale)
- [The complete key inventory](#the-complete-key-inventory)
- [Key anatomy](#key-anatomy)
- [The on-disk store](#the-on-disk-store)
- [Writing entries: atomic, private, bounded](#writing-entries-atomic-private-bounded)
- [Reading entries: self-healing by construction](#reading-entries-self-healing-by-construction)
- [The cache-through read path](#the-cache-through-read-path)
- [Validated reads: ETags and 304](#validated-reads-etags-and-304)
- [The immutable producers](#the-immutable-producers)
- [The clocked producers](#the-clocked-producers)
- [Never caching a running job](#never-caching-a-running-job)
- [Eviction: 128 MiB, 2,048 entries, oldest first](#eviction-128-mib-2048-entries-oldest-first)
- [Cache placement and relocation](#cache-placement-and-relocation)
- [The cached indicator](#the-cached-indicator)
- [The cache as its own index: recent pull requests](#the-cache-as-its-own-index-recent-pull-requests)
- [Why credentials are never cached](#why-credentials-are-never-cached)
- [Staleness findings from the adversarial review](#staleness-findings-from-the-adversarial-review)
- [Version bumps as schema migration](#version-bumps-as-schema-migration)
- [Interaction with prefetch and progressive loading](#interaction-with-prefetch-and-progressive-loading)
- [A full lifecycle trace](#a-full-lifecycle-trace)
- [Measured effect](#measured-effect)
- [Design alternatives and why they lost](#design-alternatives-and-why-they-lost)
- [Edge cases and failure modes](#edge-cases-and-failure-modes)
- [Test coverage as a specification](#test-coverage-as-a-specification)

## The problem with caches

A cache is a claim: "the answer to this question is still this value." Every cache design is a
strategy for keeping that claim true, and there are only three fundamental strategies.

**1. Expiry.** Attach a clock to each entry and stop believing it after a fixed interval. Expiry is
simple and universally applicable, but it is a guess in both directions: too short and the cache
saves nothing, too long and the reader sees stale data for up to the full interval. Worse, expiry
says nothing about correctness. A five-minute TTL on pull-request metadata means the title shown
can be five minutes old; whether that is acceptable is a product decision, not a property of the
mechanism.

**2. Validation.** Keep the entry along with a fingerprint of the answer, and ask the origin
whether the fingerprint still matches before trusting it. HTTP conditional requests are the
canonical form: the server hands out an `ETag` with each response, the client replays it in
`If-None-Match`, and a `304 Not Modified` reply confirms the cached body without resending it.
Validation is always correct but always costs a round trip, so it converts bandwidth savings into
latency that is still bounded below by the network.

**3. Immutability.** Construct the key so that it names the content. If the key fully determines
the value, the entry can never be wrong: a different question produces a different key, and the old
entry simply stops being asked for. There is no invalidation problem because there is nothing to
invalidate. The only thing that can happen to such an entry is eviction for space.

Quinjet uses all three, but the design center is the third. The architecture contract states it
directly in `ARCHITECTURE.md` invariant 12:

> Cached content is split by whether its key already names it. Entries whose key contains their
> identity are immutable and never expire: a finished run's steps and log keyed by job, a
> changed-file listing and each file's patch keyed by the merge-base and head commits, a
> conversation keyed by the stamp GitHub moves on any activity. A new head or a new comment
> therefore asks a different question rather than aging an old answer, so a stale read is
> impossible and only eviction applies. Only genuinely time-varying reads keep a clock: repository
> identity for a day, pull-request metadata for five minutes, the check list for thirty seconds. A
> run still in progress is never cached, because re-reading it is what tails it.

The rest of this page is the machinery that makes that paragraph true, and the review findings
that showed exactly what happens when a key fails to contain all of its identity.

### Why a Git TUI gets to cheat

Most caches cannot use the immutability strategy for their hot data because their questions are
not content-addressed. Quinjet's are, for a structural reason: Git itself is a content-addressed
database. A commit OID is a hash over the commit object, which names its tree and its parents by
their hashes, recursively down to every byte of every file. Two commit OIDs therefore determine
not just two snapshots but two entire histories, and any deterministic function of those histories
is a pure function of the OID pair. The diff between them, the list of changed paths, the per-file
line counts, and the merge base are all such functions. See
[the object model](../git-internals/object-model.md) for the byte-level construction of that
guarantee and [merge bases and history](../git-internals/merge-bases-and-history.md) for why even
the merge base, which depends on graph topology rather than file content, is covered by it.

GitHub adds two more identity-shaped handles on top of Git's:

- A GitHub Actions job id names one execution of one job. A finished job's log and step list can
  never change; a re-run is a different job with a different id.
- A pull request's `updatedAt` stamp moves on any activity. A conversation snapshot taken at a
  given stamp is immutable by fiat: new activity moves the stamp, which changes the key.

Everything else, which is to say the small set of questions whose answers genuinely drift in
place, gets a clock sized to how fast it drifts. That is the whole design.

## The CacheLife split

The split is a two-variant enum in `src/git/github/mod.rs`, and its doc comment is the shortest
correct statement of the policy:

```rust
/// How long an entry stays usable. `Immutable` is for content whose identity is
/// already in its key: a finished run's log, or a patch between two fixed
/// commits. Such an entry can never become wrong, only evicted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CacheLife {
    Immutable,
    Ttl(Duration),
}

impl CacheLife {
    fn accepts(self, age: Duration) -> bool {
        match self {
            Self::Immutable => true,
            Self::Ttl(ttl) => age <= ttl,
        }
    }
}
```

`accepts` is the only place freshness is ever judged. An entry's age is its file mtime distance
from now (computed at read time, see
[Reading entries](#reading-entries-self-healing-by-construction)), and every reader passes the
`CacheLife` it believes in. Three consequences fall out of this tiny interface:

**1. Freshness is the caller's belief, not the entry's property.** The store does not stamp
entries with a lifetime. The same bytes on disk can be fresh to one caller and expired to another,
which is exactly what the stale-fallback path exploits: an expired TTL entry still exists on disk
and can be served with a warning when the network fails.

**2. Immutable is not "a very long TTL".** `Immutable` accepts any age unconditionally. There is
no cliff after which an OID-keyed patch stops being trusted, because no passage of time can make
`git diff <merge-base> <head>` produce different bytes for the same two commits.

**3. A zero TTL is a usable value.** `CacheLife::Ttl(Duration::ZERO)` accepts only an age of zero,
which an mtime-derived age never is in practice. The checks module uses exactly this to express
"never serve this from cache, but still write it so a stale copy exists for offline": the steps of
a still-running job are fetched every time, yet the last fetched copy survives on disk and can be
served as stale if the network drops mid-run.

### The three clocks

Only three TTL durations exist in the codebase, one per genuinely time-varying question:

| Constant | Value | Guards | Declared at |
|---|---|---|---|
| `REPOSITORY_CACHE_TTL` | 24 hours | repository identity from `gh repo view` | `src/git/github/mod.rs:48` |
| `PULL_REQUEST_CACHE_TTL` | 5 minutes | PR metadata from `gh pr view` | `src/git/github/mod.rs:49` |
| `CHECK_LIST_CACHE_TTL` | 30 seconds | the check list from `gh pr checks` | `src/git/github/checks.rs:14` |

Each duration is an explicit judgment about drift rate:

- **Repository identity drifts on the order of never.** A remote URL resolving to an
  `owner/name` pair changes only when a repository is renamed or transferred. A day-long TTL means
  the resolution subprocess runs at most once per day per remote, and a rename is picked up within
  a day, which matches how often anyone notices a rename.
- **PR metadata drifts on the order of minutes.** Titles, descriptions, states, and the
  additions/deletions totals move while a PR is under review. Five minutes bounds how stale a
  silently reused snapshot can be; the adaptive poll described in
  [the API strategy page](./api-strategy.md) refreshes it far more often while the PR is actually
  on screen, using the `refresh` flag to bypass the TTL deliberately.
- **Check state drifts on the order of seconds.** The doc comment on the constant in
  `src/git/github/checks.rs` says why it is the outlier:

```rust
/// Check state is the one thing here that genuinely changes minute to minute,
/// so it is the one thing kept on a clock rather than on an identity.
const CHECK_LIST_CACHE_TTL: Duration = Duration::from_secs(30);
```

Thirty seconds is long enough that the 5-second active poll cadence (invariant 11) usually
answers from disk between real transitions, and short enough that a run flipping from pending to
failed appears within half a minute even without the poll.

Everything not in this table is `Immutable`. That is the punchline of the whole design: the table
of things that can go stale has three rows, and every one of them is small metadata with a
deliberate, documented drift budget.

## Why OID-keyed entries can never go stale

The claim "this entry can never become wrong, only evicted" deserves a precise argument, because
the entire cache leans on it and because the adversarial review later found the two places where
the argument had a hole (see
[the staleness findings](#staleness-findings-from-the-adversarial-review)).

**1. An OID names bytes, transitively.** Git computes an object id as a hash over
`"type size\0content"`. For a blob that covers the file bytes. For a tree it covers the child
names, modes, and child OIDs, so it transitively covers every byte of every file below it. For a
commit it covers the root tree OID, the parent commit OIDs, author, committer, and message, so it
transitively covers the entire reachable history. Two distinct histories with the same OID would
be a hash collision, which both SHA-1 (with hardened detection) and SHA-256 object formats treat
as computationally unreachable. The construction is laid out byte by byte in
[the object model page](../git-internals/object-model.md).

**2. Deterministic functions of named inputs are cacheable forever.** If a computation reads only
the objects reachable from a fixed set of OIDs and its algorithm is deterministic, its output is a
pure function of those OIDs. `git diff <a> <b>` with a pinned flag set is such a computation. So
is `git diff --name-status`, so is `--numstat`, and so is `git merge-base <a> <b>`, whose output
depends only on the parent structure the two OIDs seal. Cache any of these under a key that spells
out the OID pair and the entry is valid until the heat death of the repository.

**3. The key must contain every input.** This is the load-bearing caveat. The value must be a
function of the key, not merely correlated with it. Per-file additions and deletions for a pull
request are a function of the merge base and the head; a key that mentions only the head is
missing an input, and when the missing input changes (the PR is retargeted, the base branch is
reset) the cache serves an answer to a question that is no longer being asked. Exactly this bug
existed in the first version of the file-counts key and is why the current key is
`pr-file-counts-v3` with both commits spelled out.

**4. Identity handles from GitHub are honorary OIDs.** A job id and an `updatedAt` stamp are not
hashes, but GitHub's own semantics make them behave like names of immutable content: a completed
job's artifacts are frozen, and the stamp moves on any mutation of the PR. Quinjet treats them as
immutable keys with two provisos enforced in code: a job still running is never cached at all
(its content is still being appended), and a conversation entry is only trusted for the exact
stamp it was captured under.

The practical payoff is enormous for a diff viewer. A pull request's file index, its per-file
counts, and every one of its patches are all OID-pair functions. Once fetched they are answered
from disk for the rest of time, across sessions and across processes, and no polling loop ever
needs to reconsider them. The adaptive poll can concentrate on the three clocked questions
because everything expensive is behind immutable keys.

## The complete key inventory

Every cache key in the codebase, its life, its size cap, and the code that writes it. This is the
whole population: there are no other writers, and each key template appears in exactly one
function.

| Key template | Life | Size cap | Written by |
|---|---|---|---|
| `pull-request-v3\n{repo url}\n{number}` | Ttl 5 min | 2 MiB | `pull_request_metadata`, `src/git/github/mod.rs:824` |
| `repository\n{identity}` | Ttl 24 h | 2 MiB | `resolve_github_repository`, `src/git/github/mod.rs:1026` |
| `pr-merge-base-v1\n{repo url}\n{base}\n{head}` | Immutable | 2 MiB | `merge_base_from_api`, `src/git/github/mod.rs:1288` |
| `pr-file-counts-v3\n{repo url}\n{number}\n{base}\n{head}` | Immutable | 8 MiB | `pull_request_file_counts_from_api`, `src/git/github/mod.rs:1238` |
| `pr-files-v1\n{merge_base}\n{head}` | Immutable | 8 MiB | `changed_files_in_repository`, `src/git/github/mod.rs:1981` |
| `pr-numstat-v1\n{merge_base}\n{head}` | Immutable | 8 MiB | `numstat_counts`, `src/git/github/mod.rs:2094` |
| `pr-patch-v1\n{merge_base}\n{head}\n{path}` | Immutable | 1 MiB | `diff_file` / `diff_files`, `src/git/github/mod.rs:402` |
| `checks-v1\n{repo url}\n{number}\n{head_oid}` | Ttl 30 s | 2 MiB | `pull_request_checks`, `src/git/github/checks.rs:203` |
| `check-steps-v1\n{repo}\n{job}\n{life:?}` | Immutable or Ttl(0) | 2 MiB | `check_run_steps`, `src/git/github/checks.rs:308` |
| `check-log-v1\n{repo}\n{job}` | Immutable, settled runs only | 8 MiB | `check_run_raw_log`, `src/git/github/checks.rs:332` |
| `conversation-timeline-v2\n{url}\n{number}\n{updated_at}` | Immutable | 2 MiB | `conversation_records`, `src/git/github/conversation.rs:233` |
| `conversation-comments-v2\n{url}\n{number}\n{updated_at}` | Immutable | 2 MiB | `conversation_records`, `src/git/github/conversation.rs:233` |
| `conversation-timeline-validator-v2\n{url}\n{number}` | Immutable (ETag + body) | 2 MiB | `validated_gh`, `src/git/github/mod.rs:605` |
| `conversation-comments-validator-v2\n{url}\n{number}` | Immutable (ETag + body) | 2 MiB | `validated_gh`, `src/git/github/mod.rs:605` |
| `recent-pull-requests-v1` | Immutable | 2 MiB | `record_recent_pull_request`, `src/git/github/mod.rs:571` |

A few observations before the per-key deep dives:

**1. Immutable rows dominate, and they are the big ones.** The three clocked rows and the recents
list hold small TSV or JSON metadata. All of the bulk (patches, raw check logs, file listings,
conversation snapshots) sits behind identity keys. The cache budget is sized for that, per the doc
comment on the constants in `src/git/github/mod.rs`:

```rust
/// The cache now holds immutable content (finished run logs, patches for a
/// fixed pair of commits) rather than only small metadata blobs, so the budget
/// is sized for those and pruned oldest-first.
const MAX_CACHE_BYTES: u64 = 128 * 1024 * 1024;
const MAX_CACHE_ENTRIES: usize = 2_048;
```

**2. Two key families carry the OID pair directly.** `pr-files-v1`, `pr-numstat-v1`, and
`pr-patch-v1` are keyed by `{merge_base}\n{head}` with no repository URL at all. They do not need
one: the OIDs already name the content globally. Two different remotes serving the same commits
would legitimately share these entries, which is a feature, not an accident.

**3. Two key families carry the OID pair indirectly.** `pr-merge-base-v1` and `pr-file-counts-v3`
ask GitHub API questions, so they include the repository URL (the API needs to know whom to ask)
alongside both OIDs (which make the answer immutable). The counts key also carries the PR number
because the endpoint is addressed by number; the OIDs are what pin the answer.

**4. The checks key smuggles identity into a TTL entry.** `checks-v1` is clocked at 30 seconds,
but its key includes `head_oid`. A force-push to the PR branch changes the head OID, which changes
the key, which orphans the old check list instantly instead of letting it linger for up to 30
seconds against the wrong commit. Even the clocked entries borrow the identity trick where an
identity exists.

**5. The conversation splits content and validator into sibling keys.** The content key embeds
`updated_at` (a snapshot name), while the validator key omits it (the ETag must survive across
stamps to be useful). The two-layer read this enables is walked through in
[Validated reads](#validated-reads-etags-and-304) and in
[the conversation and checks page](./conversation-and-checks.md).

## Key anatomy

Keys are plain Rust strings with `\n` as the field separator. The choice repays a close look.

**1. Newline is unambiguous for every field that precedes one.** Repository URLs are sanitized
before they become keys (credentials, query, and fragment stripped by `remote_url_for_gh`,
`src/git/github/mod.rs:1584`), and a URL, an OID, a PR number, or a job id cannot contain a
newline. So every fixed-position field is delimiter-safe without any escaping layer.

**2. The one variable field goes last.** `pr-patch-v1\n{merge_base}\n{head}\n{path}` ends with the
file path, the only component an outside party controls. A hostile path containing newlines cannot
shift earlier fields or impersonate a different key template, because nothing is parsed after it;
the whole remaining string is the path field. The same tail position is used for `{life:?}` in the
check-steps key.

**3. Every template starts with a name and version.** `pr-files-v1`, `conversation-timeline-v2`,
`pr-file-counts-v3`: the prefix is simultaneously a namespace (two features can never collide) and
a schema version (see [Version bumps as schema migration](#version-bumps-as-schema-migration)).

**4. Trailing slashes are normalized out.** Every URL field is written as
`repository.url.trim_end_matches('/')`, so `https://github.com/oven-sh/bun` and the same URL with
a trailing slash produce one key instead of two divergent cache lines.

**5. The key never touches the filesystem.** Keys contain newlines and arbitrary path bytes, so
they would be hostile as filenames. They are hashed first (next section), and the original key is
not recoverable from the store, which is fine: nothing ever enumerates the cache by key, readers
always know the key they want.

### A worked example

Take a merge-base entry for a hypothetical repository, with deliberately synthetic OIDs so the
arithmetic is easy to follow. The key assembled by `merge_base_from_api` would be these 129 bytes
(shown with `\n` written out):

```text
pr-merge-base-v1\n
https://github.com/oven-sh/bun\n
1111111111111111111111111111111111111111\n
2222222222222222222222222222222222222222
```

Running those bytes through `stable_cache_hash` (the exact algorithm quoted in the next section)
yields the two 64-bit lanes that become the filename:

```text
key    pr-merge-base-v1\nhttps://github.com/oven-sh/bun\n1111...\n2222...
file   3a32470e3dcda1642a96155507e56c22.cache
```

Change the final byte of the head OID from `2` to `3` and the name moves:

```text
key    pr-merge-base-v1\nhttps://github.com/oven-sh/bun\n1111...\n2223...
file   3a32480e3dcda3172a95d55507e4ff62.cache
```

Note that the two names share a visible prefix. That is a real property of FNV-1a: a change in a
trailing byte only passes through one xor-multiply round in the left lane, so its avalanche into
the high-order hex digits is weak. The store does not care. It needs distinctness, not statistical
uniformity, and the second lane's rotated input mixing plus 128 total bits provide distinctness
with room to spare (the collision arithmetic is in
[Edge cases and failure modes](#edge-cases-and-failure-modes)). Nothing sorts, shards, or load
balances by these names; they are opaque handles.

## The on-disk store

The store is a directory of flat files. `CacheStore` (`src/git/github/mod.rs:2312`) is a single
`PathBuf`, discovered as `<cache root>/github`:

```rust
struct CacheStore {
    root: PathBuf,
}

impl CacheStore {
    fn discover() -> Option<Self> {
        cache_root().map(|root| Self {
            root: root.join("github"),
        })
    }
```

`discover` returning `Option` sets the tone for the whole module: every cache helper is
best-effort. If no cache root resolves (see
[Cache placement and relocation](#cache-placement-and-relocation)), all caching silently turns
off and every read goes to the network or to Git. There is no error path for "the cache is
unavailable" because the cache is an accelerator, never a source of truth.

The cache root also hosts a sibling directory, `<cache root>/tmp`, holding the disposable
`pr-<pid>-<id>.git` bare workspaces described in [the PR workspace page](./pr-workspace.md). Those
are not cache entries (they are removed on drop and swept after 24 hours), but placing them under
the same root means one environment variable relocates or isolates everything at once.

### Filenames: a double FNV over the key

The filename is the key hashed twice, by two related but independent 64-bit functions,
concatenated into 32 hex characters (`src/git/github/mod.rs:2381` and `2521`):

```rust
fn path(&self, key: &str) -> PathBuf {
    let (left, right) = stable_cache_hash(key.as_bytes());
    self.root.join(format!("{left:016x}{right:016x}.cache"))
}
```

```rust
fn stable_cache_hash(value: &[u8]) -> (u64, u64) {
    let mut left = 0xcbf2_9ce4_8422_2325_u64;
    let mut right = 0x8422_2325_cbf2_9ce4_u64;
    for byte in value {
        left ^= u64::from(*byte);
        left = left.wrapping_mul(0x0100_0000_01b3);
        right ^= u64::from(*byte).rotate_left(1);
        right = right.wrapping_mul(0x0100_0000_01b3).rotate_left(5);
    }
    (left, right)
}
```

The left lane is textbook FNV-1a: xor the byte in, multiply by the 64-bit FNV prime
`0x100000001b3`, starting from the standard offset basis `0xcbf29ce484222325`. FNV-1a is a
byte-serial multiplicative hash chosen here for three properties:

- **Stability.** The function is fully specified by two constants. It produces the same name on
  every platform, every build, and every release, which is what lets entries written by one
  version be read by the next (unless a key version bump deliberately retires them). A
  `HashMap`-style randomized hasher would be useless here.
- **Zero dependencies.** Eleven lines of arithmetic, no crate, no allocation.
- **Adequate distribution for opaque handles.** FNV's known weaknesses (poor avalanche in high
  bits for short inputs, exploitable collisions under adversarial input) do not matter for
  filenames that are never a security boundary and never a sorted structure. The store's
  correctness does not even depend on collision absence; a collision would merely make two keys
  share a file, and the magic-plus-parse validation on read degrades that to a cache miss for one
  of them.

The right lane is FNV-1a with two perturbations: the input byte is rotated left by one bit before
the xor, and the accumulator is rotated left by five bits after each multiply. The perturbations
make the two lanes genuinely different functions of the same input rather than one function
sampled twice, so the concatenated 128-bit name gets close to the collision resistance its width
suggests. The seed of the right lane is the left seed with its 32-bit halves swapped, visible in
the constants.

The test `cache_round_trips_private_metadata_and_uses_stable_keys`
(`src/git/github/mod.rs:2815`) pins the stability property: distinct keys map to distinct files,
and the same key maps to the same file across store instances.

### Entry format: magic, then payload

Every entry file starts with a 20-byte magic line and then holds the raw payload:

```text
offset  bytes  content
0       20     "quinjet-gh-cache-v1\n"   (CACHE_MAGIC, src/git/github/mod.rs:51)
20      n      payload bytes, format owned by the writer of this key
```

The magic serves three jobs at once:

- **Store versioning.** If the container format ever changes, a new magic makes every old file an
  unreadable miss rather than a misparse. This is the store-level analogue of the per-key version
  suffixes.
- **Foreign-file rejection.** A stray file dropped into the directory (an editor backup, a
  half-synced artifact) fails the prefix check and is ignored.
- **Self-description.** A `.cache` file identifies itself when inspected by hand, which matters
  for a directory of hash-named files.

The payload's internal format belongs entirely to the key's writer, and three sub-formats exist:

**1. Raw bodies.** Most entries store exactly the bytes the producer emitted: the TSV record from
`gh pr view`, the NUL-separated `--name-status` stream from Git, the patch bytes, the raw log
blob. No re-encoding, no compression, no framing. Cached bytes are deliberately fed back through
the same parsers as live bytes (see the `successful_status` replay trick in
[The immutable producers](#the-immutable-producers)), so the payload must be byte-identical to a
live read.

**2. Validator entries.** `validated_gh` stores the ETag on the first line and the body after it:

```text
offset  content
0       ETag string (no newline within)
k       '\n'
k+1     response body bytes
```

The doc comment gives the reason: "The entry holds the validator on its first line and the body
after it, so the two can never be stored out of step with each other." Storing them in separate
files would open a window where a crash leaves a new ETag beside an old body, and the next 304
would then confirm bytes the server never sent.

**3. Marker entries.** The conversation content entries prefix their TSV payload with a one-word
completeness marker (`src/git/github/conversation.rs:342`):

```rust
fn conversation_cache_entry(complete: bool, data: &[u8]) -> Vec<u8> {
    let mut entry = Vec::with_capacity(data.len().saturating_add(12));
    entry.extend_from_slice(if complete { b"complete" } else { b"partial" });
    entry.push(b'\n');
    entry.extend_from_slice(data);
    entry
}
```

A capped conversation read that had to stop before the oldest pages is still worth caching (the
newest activity is present), but the entry must remember that it is partial so the view can keep
saying "older activity was omitted" honestly on a cache hit. The marker is that memory. The same
first-line technique as the validator entries, applied to a different bit of metadata.

## Writing entries: atomic, private, bounded

`CacheStore::write` (`src/git/github/mod.rs:2348`) is short enough to quote whole, and every line
is doing correctness work:

```rust
fn write(&self, key: &str, data: &[u8], limit: usize) -> Result<()> {
    if data.len() > limit {
        return Ok(());
    }
    create_private_directory(&self.root)?;
    let destination = self.path(key);
    let id = CACHE_WRITE_ID.fetch_add(1, Ordering::Relaxed);
    let temporary = self
        .root
        .join(format!(".write-{}-{id}.tmp", std::process::id()));
    let mut options = OpenOptions::new();
    let _ = options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        let _ = options.mode(0o600);
    }
    let mut file = options.open(&temporary)?;
    file.write_all(CACHE_MAGIC)?;
    file.write_all(data)?;
    file.flush()?;
    drop(file);
    if destination.exists() {
        drop(fs::remove_file(&destination));
    }
    if let Err(error) = fs::rename(&temporary, &destination) {
        drop(fs::remove_file(&temporary));
        return Err(error.into());
    }
    self.prune();
    Ok(())
}
```

Step by step, with the reasoning each step encodes:

**1. Oversized data is silently not written.** `data.len() > limit` returns `Ok(())`, not an
error. The limit is per-key-family (1 MiB for patches, 8 MiB for listings and logs, 2 MiB for
metadata), and an oversized value is simply not worth keeping: the next reader will regenerate it.
Returning success matters because every call site is fire-and-forget; a cache write must never
fail a user-visible operation.

**2. The directory is (re)made private on every write.** `create_private_directory`
(`src/git/github/mod.rs:2511`) is `create_dir_all` followed by an unconditional `chmod 0700` on
Unix:

```rust
fn create_private_directory(path: &Path) -> Result<()> {
    fs::create_dir_all(path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}
```

Re-applying the mode every time means a directory that was created by an older build, restored
from a backup with loosened permissions, or pre-created by the user is tightened back to
owner-only before any new bytes land in it.

**3. The temp file name cannot collide across writers.** `.write-{pid}-{counter}.tmp` combines
the process id with a process-local `AtomicU64` (`CACHE_WRITE_ID`, `src/git/github/mod.rs:64`).
Two threads in one process get different counters; two processes get different pids. On top of
that, `create_new(true)` maps to `O_CREAT | O_EXCL`, so even a pathological collision fails the
open rather than interleaving two writers' bytes in one file.

**4. The mode is set at open time, not after.** `options.mode(0o600)` puts the permission bits on
the `open(2)` call itself. There is no window, however small, in which the file exists with
default permissions while holding repository content. The test at `src/git/github/mod.rs:2827`
asserts `mode & 0o077 == 0` on a written entry: no group bits, no world bits.

**5. Magic, payload, flush, close.** The writer produces the complete entry in the temp file
before the destination name is involved at all. `flush` pushes the userspace buffer to the OS.
Note what is deliberately absent: there is no `fsync`. A power failure can lose a just-written
entry (the rename or the data may not have hit the platter), and that is an accepted outcome
because a lost cache entry costs one refetch. What `fsync`-lessness can never cause here is a
torn entry at the destination name, because of the next step.

**6. Rename publishes atomically.** `fs::rename` within one directory is atomic on POSIX
filesystems: any concurrent reader sees either the complete old file or the complete new file,
never a prefix. The temp file lives in the same directory as the destination precisely so the
rename never crosses a filesystem boundary (a cross-device rename is not atomic and fails
outright on most platforms). The `destination.exists()` removal just before the rename exists for
Windows, where renaming onto an existing file historically fails; on Unix the rename would have
replaced it atomically anyway, and the tiny remove-then-rename window is harmless because a reader
finding no file simply misses.

**7. Failure cleans up.** A failed rename deletes the temp file so aborted writes cannot
accumulate. A crash between open and rename leaves a `.write-*.tmp` orphan, which is invisible to
reads (readers look up exact hashed names ending in `.cache`) and does not count against the
prune accounting (prune only considers `.cache` files); the orphan occupies its bytes until the
directory is cleared, an accepted cosmetic cost of not running a sweeper.

**8. Every successful write triggers pruning.** `self.prune()` runs inline, keeping the store
inside its budgets at all times rather than on a timer. The prune pass is examined in
[Eviction](#eviction-128-mib-2048-entries-oldest-first).

The public entry points wrap all of this in `Option`-tolerant, error-swallowing helpers
(`src/git/github/mod.rs:534`):

```rust
pub(crate) fn cache_write(key: &str, data: &[u8]) {
    cache_write_bounded(key, data, MAX_GH_METADATA_BYTES);
}

pub(crate) fn cache_write_bounded(key: &str, data: &[u8], limit: usize) {
    if let Some(cache) = CacheStore::discover() {
        drop(cache.write(key, data, limit));
    }
}
```

`drop(...)` on the `Result` is the whole error-handling policy for writes: there is none. A full
disk, a read-only mount, a permissions problem: all of them degrade Quinjet to a cacheless mode
without a single user-visible symptom beyond slower reloads.

## Reading entries: self-healing by construction

`CacheStore::read` (`src/git/github/mod.rs:2328`) mirrors the write path's paranoia:

```rust
fn read(&self, key: &str, limit: usize) -> Option<CacheEntry> {
    let path = self.path(key);
    let metadata = fs::metadata(&path).ok()?;
    if metadata.len() > limit as u64 + CACHE_MAGIC.len() as u64 {
        drop(fs::remove_file(path));
        return None;
    }
    let mut data = fs::read(path).ok()?;
    if !data.starts_with(CACHE_MAGIC) {
        return None;
    }
    drop(data.drain(..CACHE_MAGIC.len()));
    let age = metadata
        .modified()
        .ok()
        .and_then(|modified| SystemTime::now().duration_since(modified).ok())
        .unwrap_or_default();
    Some(CacheEntry { data, age })
}
```

**1. Stat before read.** The size check happens on metadata, before any bytes are loaded, so an
oversized file never costs its own size in memory even once.

**2. Oversized entries are deleted on sight.** This is the self-healing rule: if a future release
shrinks a limit (say a patch cap goes down), every entry written under the old, larger limit
becomes oversized relative to the new one and is physically removed the first time it is asked
for. The store converges to the current limits without any migration pass. The comparison allows
for the magic prefix (`limit + CACHE_MAGIC.len()`), so a payload exactly at the limit is legal.

**3. Wrong magic is a miss, not a deletion.** A file that does not start with `CACHE_MAGIC` is
ignored but left in place. The asymmetry with the oversized case is deliberate: an oversized entry
is definitely ours and definitely obsolete, whereas a wrong-magic file might not be ours at all,
and deleting foreign files from a directory the user can point anywhere via `QUINJET_CACHE_DIR`
would be rude. It will age out through prune's mtime ordering if it is a `.cache` file, or sit
inert otherwise.

**4. Age comes from mtime, with a defensive default.** `duration_since(modified)` fails if the
mtime is in the future (clock adjustments, files copied from another machine); `unwrap_or_default`
turns any failure into an age of zero, which reads as maximally fresh. For immutable entries the
default is trivially correct (any age is accepted). For TTL entries it errs toward serving the
entry, which is the right bias for a cache whose TTLs guard convenience metadata rather than
correctness (see [Edge cases and failure modes](#edge-cases-and-failure-modes) for the full clock
skew discussion).

**5. Freshness is filtered by the caller, one layer up.** `read` returns the entry with its age
and lets `cache_read_bounded` (`src/git/github/mod.rs:527`) apply the life:

```rust
pub(crate) fn cache_read_bounded(key: &str, life: CacheLife, limit: usize) -> Option<Vec<u8>> {
    CacheStore::discover()?
        .read(key, limit)
        .filter(|entry| life.accepts(entry.age))
        .map(|entry| entry.data)
}
```

Because the store never deletes an entry for being old, an expired TTL entry still exists on disk
after `accepts` rejects it, and the cache-through wrapper in the next section can reach past the
filter to serve it as an explicitly labeled stale answer when the network fails. Expiry as
implemented here is not "the entry is gone"; it is "the entry needs requalification," and a
network failure is one of the qualifying conditions.

The doc comment on the plain `cache_read` helper (`src/git/github/mod.rs:520`) names the audience
for these direct functions:

```rust
/// Direct cache access for readers that cannot express themselves as a single
/// `gh` invocation: a response judged by its body rather than its exit status,
/// or bytes produced by Git rather than by GitHub.
```

Two categories of reader bypass the cache-through wrapper and use these primitives directly: the
check list (whose `gh pr checks` exits non-zero for failing checks, so a useful body must be
recognized by content) and every Git-produced byte stream (`pr-files-v1`, `pr-numstat-v1`,
`pr-patch-v1`), which never involves `gh` at all.

## The cache-through read path

Most GitHub metadata flows through one wrapper, `checked_cached_gh_bounded`
(`src/git/github/mod.rs:1104`), which composes the cache, the subprocess, and the fallback policy
into a single ordered decision. Its core, lightly trimmed to the control flow:

```rust
let cache = CacheStore::discover();
let cached = cache
    .as_ref()
    .and_then(|cache| cache.read(cache_key, limit));
if (!refresh || life == CacheLife::Immutable)
    && let Some(entry) = cached.as_ref()
    && life.accepts(entry.age)
{
    return Ok(GhResponse {
        data: entry.data.clone(),
        disposition: CacheDisposition::Fresh,
    });
}

let output = match self.run_gh_bounded(args, limit) {
    Ok(output) => output,
    Err(error) => {
        if let Some(entry) = cached.as_ref() {
            return Ok(GhResponse {
                data: entry.data.clone(),
                disposition: CacheDisposition::Stale,
            });
        }
        return Err(error);
    }
};
if output.status.success() && !output.stdout_truncated {
    if let Some(cache) = cache.as_ref() {
        drop(cache.write(cache_key, &output.stdout, limit));
    }
    return Ok(GhResponse {
        data: output.stdout,
        disposition: CacheDisposition::Network,
    });
}
if let Some(entry) = cached {
    return Ok(GhResponse {
        data: entry.data,
        disposition: CacheDisposition::Stale,
    });
}
if output.stdout_truncated {
    bail!("{error_context}: GitHub CLI output exceeded the metadata limit");
}
bail!("{}", bounded_command_error(error_context, &output));
```

The decision ladder, in order:

**1. Fresh cache wins.** If the entry exists and its age passes `life.accepts`, return it as
`Fresh` without spawning anything. One subtlety in the guard:
`!refresh || life == CacheLife::Immutable`. The `refresh` flag (set by the explicit reload
control and by `--refresh` on the CLI) bypasses TTL entries, forcing a network read, but it
cannot bypass an immutable entry. There is nothing to refresh: the entry is a pure function of
its key, and refetching it would spend a request to receive the same bytes. This is why a
`--refresh` on `quinjet pr view` re-reads the five-minute metadata but still serves every
commit-keyed patch from disk.

**2. A failed spawn falls back to any cached entry, however old.** If `gh` cannot run at all (not
installed, network down at the DNS level, auth broken), the wrapper returns the cached bytes with
`CacheDisposition::Stale`. Note that this branch ignores `accepts` entirely: a three-day-old
five-minute entry is better than an error when the alternative is nothing.

**3. A clean, complete response is cached and returned as `Network`.** Both conditions matter:
`status.success()` filters errors, and `!stdout_truncated` filters responses that hit the pipe
cap and had the child killed (see the bounded runner in
[the API strategy page](./api-strategy.md)). A truncated body must never be cached, because a
cached truncation would faithfully serve its missing tail forever.

**4. A failed response with a cached entry degrades to `Stale`.** Same fallback as step 2, for
the case where `gh` ran but GitHub answered with an error.

**5. Only with no cache at all does the error surface.** Truncation gets its own message
("output exceeded the metadata limit"); anything else reports the child's stderr through
`bounded_command_error`.

The three-valued `CacheDisposition` (`src/git/github/mod.rs:213`) is how the answer's provenance
travels to the UI:

```rust
enum CacheDisposition {
    Network,
    Fresh,
    Stale,
}
```

`pull_request_lookup` (`src/git/github/mod.rs:721`) converts `Stale` into the user-visible
warning "GitHub is unavailable; showing stale cached metadata for #N" and sets
`from_cache = disposition != Network` on the snapshot, which ultimately drives the `cached`
indicator described [below](#the-cached-indicator).

### A worked offline session

The composed behavior is easiest to see end to end. Suppose a laptop viewed PR #30412 an hour ago
and is now on a train with no connectivity:

1. `quinjet pr view 30412` resolves the repository identity: the `repository\n{url}` entry is 1
   hour old against a 24-hour TTL, so it is `Fresh`. No subprocess.
1. PR metadata: the `pull-request-v3` entry is an hour old against a 5-minute TTL, so step 1
   fails; `gh pr view` fails to reach GitHub; step 2 serves the hour-old entry as `Stale`, and
   the warning line appears.
1. The workspace prepares. If both OIDs are local the whole diff path is network-free anyway; if
   not, `merge_base_from_api` finds its `pr-merge-base-v1` entry (Immutable, always fresh), and
   `pr-file-counts-v3` likewise. The file index and numstat entries (`pr-files-v1`,
   `pr-numstat-v1`) replay from disk.
1. Every previously viewed file's `pr-patch-v1` entry serves instantly. Unviewed files whose
   blobs are not local fail their lazy fetch, which is the one thing disk cannot fake.
1. The check list's 30-second entry is long expired; `gh pr checks` fails; the stale list is
   served. A settled job's `check-log-v1` entry serves its full log from disk.

The session degrades feature by feature instead of failing at the front door, and everything the
reader already paid for remains readable.

## Validated reads: ETags and 304

The conversation streams sit between the two poles: their content changes (so immutable content
keys alone are not enough) but usually has not changed since the last poll (so refetching pages
is usually waste). The bridge is HTTP validation, implemented once in `Repository::validated_gh`
(`src/git/github/mod.rs:605`) and explained by its doc comment:

```rust
/// A validated read: GitHub is asked whether the answer changed, and answers
/// `304 Not Modified` when it did not. That reply carries no body and costs
/// nothing against the rate limit, which is what lets an unchanged thread be
/// re-checked as often as it is worth checking.
///
/// The entry holds the validator on its first line and the body after it, so
/// the two can never be stored out of step with each other.
```

The mechanism, following the code:

**1. Load the validator entry and split it.** The entry under the caller's key (for example
`conversation-timeline-validator-v2\n{url}\n{number}`) is read with `CacheLife::Immutable` and
split at the first newline into `(etag, body)` by `split_validator`
(`src/git/github/mod.rs:655`).

**2. Replay the ETag as `If-None-Match`.** The request becomes `gh api -i` (the `-i` includes
response headers in stdout) with `-H "If-None-Match: {etag}"` when a validator exists:

```rust
let mut request = vec![OsString::from("api"), OsString::from("-i")];
if let Some(validator) = validator.as_ref() {
    request.push(OsString::from("-H"));
    request.push(OsString::from(format!("If-None-Match: {validator}")));
}
request.extend(args);
```

**3. A 304 answers from the stored body.** If the status line contains ` 304`, the cached body is
returned with `unchanged: true, complete: true`. Per GitHub's documented behavior for
[conditional requests](https://docs.github.com/en/rest), a 304 response does not count against
the rate limit, which is what makes the 20-second conversation poll floor affordable: an
unchanged thread costs zero rate-limit budget per check.

**4. A 200 is stored only when complete.** The fresh response is split into head and body;
completeness is computed as `!output.stdout_truncated && !has_next_page(head)`, and the ETag is
stored, joined to the body it validates, only under that condition:

```rust
let complete = !output.stdout_truncated && !has_next_page(head);
if let Some(etag) = header_value(head, "etag").filter(|_| complete) {
    let mut entry = etag.into_bytes();
    entry.push(b'\n');
    entry.extend_from_slice(body);
    cache_write(key, &entry);
}
```

The `filter(|_| complete)` is a one-line fix for a subtle correctness trap: a first page that has
a `rel="next"` continuation, or that was cut by the 2 MiB pipe cap, is a fragment. If its ETag
were stored, a later 304 would "validate" the fragment as the whole answer, and the conversation
would permanently miss its tail while claiming freshness. The tests
`only_a_single_page_answer_is_worth_a_validator` and
`a_cache_entry_keeps_its_validator_beside_the_body_it_validates`
(`src/git/github/mod.rs:3168` and `3224`) pin both halves of the rule. The `truncated` field on
`ValidatedRead` did not exist in the first version of this code; its absence enabled a
cache-poisoning race the adversarial review caught, described with the other findings
[below](#staleness-findings-from-the-adversarial-review).

**5. Pagination hints ride along.** `last_page` parses the `rel="last"` page number out of the
`Link` header so the timeline reader can walk pages newest-first. How the conversation combines
the validator layer with its stamped content cache and the bounded page loop is the subject of
[the conversation and checks page](./conversation-and-checks.md); from the cache's perspective the
contract is only this: validator entries are immutable, joined ETag-and-body, and only ever
written whole.

## The immutable producers

Each immutable key family has one producer function, and each producer enforces the same two
rules in its own dialect: only complete data is cached, and cached bytes replay through the exact
code path live bytes take. This section walks every family.

### The merge base: pr-merge-base-v1

`merge_base_from_api` (`src/git/github/mod.rs:1288`) asks the GitHub compare API one question
whose answer replaces an entire deepening fetch ladder (see
[merge bases and history](../git-internals/merge-bases-and-history.md) for why local merge-base
computation needs deep history that a shallow workspace does not have). The caching skeleton:

```rust
let key = format!(
    "pr-merge-base-v1\n{}\n{base}\n{head}",
    repository.url.trim_end_matches('/')
);
if let Some(cached) = cache_read(&key, CacheLife::Immutable) {
    let cached = String::from_utf8_lossy(trim_ascii(&cached)).into_owned();
    if is_commit_oid(&cached) {
        return Some(cached);
    }
}
```

Three properties worth naming:

- **Input validation gates the key.** The function returns `None` before building any key unless
  both `base` and `head` pass `is_commit_oid` (40 or 64 ASCII hex characters) and the repository
  has a name. Garbage can neither pollute the cache nor reach a `gh` argv.
- **Output validation gates the hit.** A cached value is used only if it still parses as a commit
  OID. A corrupted entry degrades to a miss and a refetch, never to a bogus revision handed to
  `git fetch`.
- **The merge base of two OIDs is itself immutable.** Both commits seal their entire ancestries,
  so the best common ancestor is a pure function of the pair. GitHub answering the question
  instead of a local graph walk changes where the answer comes from, not what it is. One caveat
  the review surfaced: immutability of the answer does not protect against asking about the wrong
  pair, which is the hint-mismatch finding covered
  [below](#staleness-findings-from-the-adversarial-review).

The network path runs `gh api repos/{owner}/{name}/compare/{base}...{head}` with
`--jq .merge_base_commit.sha`, writes the validated SHA back under the key, and any failure at
all simply returns `None`, letting the fetch ladder in
[the PR workspace](./pr-workspace.md) take over.

### Per-file counts: pr-file-counts-v3

`pull_request_file_counts_from_api` (`src/git/github/mod.rs:1238`) exists because of a cost
asymmetry documented in its doc comment:

```rust
/// Per-file additions and deletions from the pull-request files endpoint.
/// In the blob-less disposable workspace a local `--numstat` would download
/// every changed blob just to count lines; GitHub already knows the totals.
```

In a `blob:none` workspace (see
[shallow and partial clone](../git-internals/shallow-and-partial-clone.md)), counting lines
locally means materializing every changed blob over the network first. The pulls files endpoint
already holds the totals, so PR #49 moved the read there. The caching contract:

```rust
let key = format!(
    "pr-file-counts-v3\n{}\n{}\n{base}\n{head}",
    repository.url.trim_end_matches('/'),
    pull_request.number
);
if let Some(data) = cache_read_bounded(&key, CacheLife::Immutable, MAX_PR_PATH_BYTES) {
    return Some(parse_api_file_counts(&data));
}
```

The network path pages through `repos/{owner}/{name}/pulls/{number}/files?per_page=100` for up to
`MAX_FILE_COUNT_PAGES = 64` pages (6,400 files), accumulating TSV records. Its cache discipline
shows the only-complete rule under pagination:

- A page whose body was pipe-truncated aborts the whole function with `None`; nothing partial is
  cached or even returned.
- The accumulated body is written back only when `complete` (the loop saw a page without a next
  link) and the total fits `MAX_PR_PATH_BYTES` (8 MiB).
- An incomplete-but-untruncated accumulation (the 64-page cap ran out first) is still *returned*,
  because partial counts usefully fill most headers, but it is *not cached*, because a cached
  partial would freeze the gap forever.

The `-v3` in the key is a scar with a story: version 1 of this key omitted the base commit and
keyed only the head, which the adversarial review flagged as a genuine staleness hole. The full
account is in [the review findings](#staleness-findings-from-the-adversarial-review).

### The file index and its counts: pr-files-v1 and pr-numstat-v1

These two entries hold raw Git output, not API responses: the NUL-separated
`git diff --name-status -z --find-renames {merge_base} {head} --` stream and its
`--numstat -z` twin (see [git-diff](https://git-scm.com/docs/git-diff) for the formats and
[the diff pipeline](../diff/pipeline.md) for the parsing). `changed_files_in_repository`
(`src/git/github/mod.rs:1981`) demonstrates the replay technique that keeps cached and live bytes
on one code path:

- On a cache hit, the raw bytes are wrapped in a synthetic `BoundedOutput` with
  `successful_status()` (`src/git/github/mod.rs:2124`, fabricating exit code 0), so the exact
  same parser that consumes a live subprocess's stdout consumes the cached bytes. There is no
  second deserialization format to keep in sync, no version skew between "cache schema" and
  "wire schema": the cache schema *is* the wire schema.
- On a live run, stdout is capped at `MAX_PR_PATH_BYTES` (8 MiB); the entry is written only when
  the run was not truncated. A truncated listing is trimmed to its last complete NUL record and
  used for this session, flagged `truncated`, but never persisted.

`numstat_counts` (`src/git/github/mod.rs:2094`) applies the identical discipline to the numstat
bytes under `pr-numstat-v1\n{merge_base}\n{head}`, and its doc comment states the product reason
the read exists at all:

```rust
/// Read exact per-file totals alongside the changed-path listing. One extra
/// `--numstat` pass over the same range lets every file header render its real
/// `+n -n` immediately, so the list never fills in unevenly as patches load.
```

Note the division of labor established by PR #49 and preserved since: the API counts serve the
disposable blob-less workspace (where local numstat would trigger the blob storm), while
`numstat_counts` serves the opened-repository path (where every blob is already on disk and one
local pass is cheap). Both produce the same `HashMap<PathBuf, DiffLineCounts>` shape, and both
cache immutably under their respective keys.

### Patches: pr-patch-v1 and the 1 MiB ceiling

The per-file patch cache is the highest-traffic immutable family and the only one with its own
size ceiling, explained by its doc comment (`src/git/github/mod.rs:40`):

```rust
/// A single file's patch is cached only if it is small enough that one file
/// cannot crowd out the rest of a pull request.
const MAX_CACHED_PATCH_BYTES: usize = 1024 * 1024;
```

The arithmetic behind the ceiling: the whole store budgets 128 MiB across 2,048 entries. Without
a per-patch bound, a handful of generated-file patches (lockfiles, minified bundles, vendored
trees) could occupy the entire byte budget, evicting hundreds of small patches that real reading
sessions actually revisit. At 1 MiB per patch, even a pathological PR needs 128 cached patches to
fill the byte budget, and the oldest-first prune then rotates fairly. An over-1-MiB patch is
still rendered (the live read caps at 8 MiB), it just is not persisted; reopening that file costs
one `git diff` again.

`PreparedPullRequest::diff_file` (`src/git/github/mod.rs:402`) shows the full read-through:

```rust
let key = patch_cache_key(&self.merge_base, &self.head, &file.path);
if let Some(patch) = cache_read_bounded(&key, CacheLife::Immutable, MAX_CACHED_PATCH_BYTES)
{
    return Ok(pull_request_file_document(
        &patch,
        &self.pull_request,
        file,
        false,
    ));
}
let (patch, truncated) = diff_selected_paths(
    self.repository.path(),
    &self.merge_base,
    &self.head,
    std::slice::from_ref(&file.path),
)?;
if !truncated {
    cache_write_bounded(&key, &patch, MAX_CACHED_PATCH_BYTES);
}
```

with `patch_cache_key` (`src/git/github/mod.rs:2137`) formatting
`pr-patch-v1\n{merge_base}\n{head}\n{path}`. Two details:

- A cache hit builds its document with `truncated = false` unconditionally, which is sound
  because a truncated patch is never written: presence in the cache proves completeness.
- The batched sibling `diff_files` (`src/git/github/mod.rs:440`) writes each complete section of
  a combined multi-file patch into this same per-file key as a side effect. Background prefetch
  therefore populates exactly the entries a later foreground click will hit; the interplay is
  covered in
  [Interaction with prefetch and progressive loading](#interaction-with-prefetch-and-progressive-loading).
  A section cut by the 8 MiB combined-patch cap is the one exception: it is either retried alone
  in a later batch or surfaced marked truncated, but never cached.

### Conversation snapshots: the stamp as a logical clock

The two conversation content keys embed the PR's `updatedAt` in what the code calls the stamp
(`src/git/github/conversation.rs:167`):

```rust
let stamp = format!(
    "{}\n{}\n{}",
    pull_request.base_repository.url.trim_end_matches('/'),
    pull_request.number,
    pull_request.updated_at
);
```

`conversation-timeline-v2\n{stamp}` and `conversation-comments-v2\n{stamp}` are therefore not
caches of "the conversation" but caches of "the conversation as of this stamp." GitHub moves
`updatedAt` on any activity, so the stamp behaves as a logical clock: a new comment does not
invalidate the old entry, it changes the question, and the old entry quietly becomes unreachable
garbage for the pruner. Within one stamp, a repeat read (the 20-second poll, a pane switch, a
process restart) costs zero requests:

- Layer 1: the stamped content entry hits, `from_cache = true`, no network.
- Layer 2: on a stamp the store has not seen, the ETag validator (previous section) usually
  answers 304, costing one free request.
- Layer 3: only genuinely new activity pays for pages, and the bounded newest-first page loop
  keeps even that capped.

The completeness marker (`complete`/`partial` first line) from
[the entry format section](#the-on-disk-store) makes a capped snapshot cacheable without lying:
`truncated = !complete` is restored on every hit, keeping the "older activity was omitted" banner
truthful across cache round trips. Only a pipe-truncated read (killed `gh`, ragged final record)
is never cached at all.

### Finished check runs: check-steps-v1 and check-log-v1

A completed GitHub Actions job is frozen: its step list and its log archive can never change, and
a re-run allocates a new job id. Both reads therefore cache immutably once a run has settled:

- `check-steps-v1\n{repository}\n{job}\n{life:?}` holds the steps TSV from the jobs API, read
  through the standard cache-through wrapper at the 2 MiB metadata bound.
- `check-log-v1\n{repository}\n{job}` holds the raw log blob, up to `MAX_CHECK_LOG_BYTES`
  (8 MiB), read and written through the direct bounded helpers because a log is far too large for
  the metadata limit.

The `{life:?}` component in the steps key deserves a pause: the `Debug` rendering of the
`CacheLife` value is part of the key, so a running job's `Ttl(0ns)` entries and the same job's
later `Immutable` entries are distinct cache lines. Without it, the last poll of a running job
(steps still showing an in-progress state) could be written moments before the job settles, and
the first settled read would then hit that stale in-progress snapshot under an immutable life and
trust it forever. Two lives, two keys, no crossover. The running-job half of this design is the
next section.

The write condition in `check_run_raw_log` (`src/git/github/checks.rs:364`) compresses the whole
policy into one line:

```rust
if life == CacheLife::Immutable && !output.stdout_truncated && !output.stdout.is_empty() {
    super::cache_write_bounded(&key, &output.stdout, MAX_CHECK_LOG_BYTES);
}
```

Settled only, complete only, nonempty only. A settled run read once is answered from disk forever
after, which is what the background warm lane exploits
([Interaction with prefetch](#interaction-with-prefetch-and-progressive-loading)).

## The clocked producers

The three TTL families are small, but each one shows a different wrinkle of clock-based caching
done carefully.

### Repository identity: a day

`resolve_github_repository` (`src/git/github/mod.rs:1026`) turns a sanitized remote URL into an
`owner/name` identity via `gh repo view`, cached under `repository\n{identity}` for
`REPOSITORY_CACHE_TTL` (24 hours). The identity component is either the sanitized URL itself or,
for the no-remote inference fallback, `inferred\n{repo root}\n{GH_REPO}`, so two checkouts (or
two values of the `GH_REPO` override) never share an inference. Notably, github.com URLs never
reach this path at all: `repository_from_remote_url` (`src/git/github/mod.rs:1607`) derives the
identity offline with zero subprocess, and only enterprise hosts and odd URLs pay the `gh` call.
The cache line exists for exactly the reads that cost something.

### Pull-request metadata: five minutes

`pull_request_metadata` (`src/git/github/mod.rs:824`) is the canonical cache-through consumer:

```rust
let response = self.checked_cached_gh(
    &format!(
        "pull-request-v3\n{}\n{number}",
        repository.url.trim_end_matches('/')
    ),
    CacheLife::Ttl(PULL_REQUEST_CACHE_TTL),
    refresh,
    pull_request_view_args(repository, number),
    "unable to load pull request",
)?;
```

The payload is the 18-field TSV record produced by `gh pr view --json ... --jq`, small enough
that the five-minute TTL costs almost nothing in staleness and saves a subprocess plus a network
round trip on every navigation that touches the PR within the window: switching panes, reopening
the picker, a CLI invocation following a TUI session. The `refresh` flag rides in from the
explicit reload control and from the adaptive poll, so the TTL is a floor for passive reads, not
a ceiling on freshness.

This is also the entry the stale-fallback path most visibly serves: on any network failure the
previous TSV comes back as `Stale`, `pull_request_lookup` attaches the warning "GitHub is
unavailable; showing stale cached metadata for #N", and the snapshot is flagged as cached rather
than silently passed off as current.

### The check list: thirty seconds, keyed by head

`pull_request_checks` (`src/git/github/checks.rs:203`) cannot use the cache-through wrapper
because `gh pr checks` speaks through exit codes: 1 means checks failed, 8 means checks are
pending, and both come with a perfectly good body. The wrapper would treat those as failures and
serve stale. So the function reads and writes the cache manually around content-judged
acceptance:

```rust
let key = format!(
    "checks-v1\n{}\n{}\n{}",
    pull_request.base_repository.url.trim_end_matches('/'),
    pull_request.number,
    pull_request.head_oid
);
if !refresh
    && let Some(cached) = super::cache_read(&key, CacheLife::Ttl(CHECK_LIST_CACHE_TTL))
{
    return Ok(PullRequestChecks {
        checks: parse_pull_request_checks(&cached)?,
        from_cache: true,
    });
}
```

and later, after accepting exit status 0, 1, or 8 with a nonempty body:

```rust
let checks = parse_pull_request_checks(&output.stdout)?;
super::cache_write(&key, &output.stdout);
```

The `head_oid` in the key is the identity trick applied to a clocked entry: a force-push makes
the old check list unreachable instantly instead of surviving up to 30 seconds against a commit
it no longer describes. And because the TTL is so short, the check list is deliberately excluded
from the `cached` indicator's definition (next but one section): an indicator that flickered off
every 30 seconds would communicate nothing.

## Never caching a running job

Invariant 12 ends with the rule that has no exceptions: "A run still in progress is never cached,
because re-reading it is what tails it." The mechanism is the life-selection branch at the top of
`pull_request_check_log` (`src/git/github/checks.rs:266`):

```rust
let life = if check.status.is_running() {
    CacheLife::Ttl(Duration::ZERO)
} else {
    CacheLife::Immutable
};
```

Everything downstream flows from that one choice:

**1. Reads never hit the cache while running.** `Ttl(Duration::ZERO)` accepts only age zero, so
`check_run_raw_log` skips its cache probe entirely (the probe is gated on
`life == CacheLife::Immutable`) and every call refetches the log endpoint. The endpoint serves
whatever the runner has written so far, so the 8-second log poll floor turns repetition into
tailing: each refetch returns a longer prefix of the same growing log, and the view appends.

**2. Writes never happen while running.** The write condition quoted in the previous section
requires `life == CacheLife::Immutable`. A partial log written to disk would be worse than
useless: it would either require an invalidation story (which the immutable side exists to
avoid) or serve a frozen prefix of a job that has long since moved on.

**3. The steps read still writes, but into a separate keyspace.** `check_run_steps` goes through
the standard wrapper with the `Ttl(0)` life, which never serves fresh, but the wrapper still
writes each response. That gives the running job's steps a disk copy under the
`check-steps-v1\n...\nTtl(0ns)` key that exists purely as stale-fallback material: if the network
drops mid-run, the wrapper's failure branch serves the last fetched step list instead of blanking
the pane. The `{life:?}` key component guarantees this transient copy can never be mistaken for
the settled run's immutable entry.

**4. Why a TTL would be wrong, not just worse.** Suppose running logs were cached for, say, five
seconds to soften the poll. Then the tail becomes a stutter: two polls inside the window return
identical bytes, the view appears stalled, and the reader cannot distinguish "job is quiet" from
"cache is serving." Worse, the transition to settled would race the last cached partial: a job
finishing inside the window could have its final lines masked by a fresh-looking partial entry.
`Ttl(Duration::ZERO)` makes the running state a pure pass-through, and the settled state a pure
disk read, with no third mode between them.

**5. The 404 window is part of the same story.** For the first seconds of a job, before the log
blob exists at all, the endpoint answers 404; `log_not_published`
(`src/git/github/checks.rs:381`) maps 404 and 410 (retention expired) to an empty log rather
than an error, and `log_pending` tells the view to say the runner has not written anything yet.
Nothing about that window is cacheable either, and nothing is cached.

The warm-up path respects the same line. `prefetch_check_run_logs`
(`src/git/github/checks.rs:293`) filters to settled runs before warming:

```rust
checks
    .iter()
    .filter(|check| !check.status.is_running() && check.job_id().is_some())
    .take(MAX_PREFETCHED_CHECK_LOGS)
    .take_while(|_| wanted())
    .filter(|check| self.pull_request_check_log(pull_request, check).is_ok())
    .count()
```

with the doc comment making the request-budget argument explicit: "Runs still in progress are
skipped: their output is not cacheable, and re-reading it here would spend requests the live tail
is about to spend anyway." The cap of `MAX_PREFETCHED_CHECK_LOGS = 32` settled jobs and the
`wanted()` cancellation hook belong to the warm-lane design described in
[the concurrency page](../rendering/concurrency.md); from the cache's perspective the warm lane
is just an eager reader whose every successful read lands in `check-steps-v1` and `check-log-v1`
exactly as a foreground read would.

## Eviction: 128 MiB, 2,048 entries, oldest first

Immutable entries never expire, so something else must bound the store. That something is
`prune` (`src/git/github/mod.rs:2413`), run inline after every successful write:

```rust
fn prune(&self) {
    let Ok(entries) = fs::read_dir(&self.root) else {
        return;
    };
    let mut files = entries
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let path = entry.path();
            if path.extension().and_then(OsStr::to_str) != Some("cache") {
                return None;
            }
            let metadata = entry.metadata().ok()?;
            Some((
                metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH),
                metadata.len(),
                path,
            ))
        })
        .collect::<Vec<_>>();
    files.sort_by_key(|(modified, ..)| *modified);
    let mut total = files.iter().map(|(_, bytes, _)| *bytes).sum::<u64>();
    let mut count = files.len();
    for (_, bytes, path) in files {
        if count <= MAX_CACHE_ENTRIES && total <= MAX_CACHE_BYTES {
            break;
        }
        if fs::remove_file(path).is_ok() {
            count = count.saturating_sub(1);
            total = total.saturating_sub(bytes);
        }
    }
}
```

The policy in one sentence: list every `.cache` file with its mtime and size, sort ascending by
mtime, and delete from the oldest end until both budgets hold. The details:

**1. Two budgets, both enforced.** `MAX_CACHE_BYTES = 128 MiB` bounds disk usage;
`MAX_CACHE_ENTRIES = 2_048` bounds directory size (keeping every `read_dir` scan, including this
one and the recents scavenger, cheap). Whichever budget is exceeded drives deletion, and deletion
continues until both are satisfied. With mostly small metadata entries the entry cap binds first:
2,048 TSV records of a few hundred bytes never approach 128 MiB. With patch-heavy usage the byte
cap binds first: at the 1 MiB patch ceiling, 128 maximal patches alone fill the byte budget.

**2. Oldest write first, not least recently used.** The sort key is mtime, and reads do not touch
mtime, so this is write-age (FIFO) eviction rather than LRU. That is a deliberate simplicity
trade: touching a file on every read would turn every cache hit into a disk write (and a metadata
write at that, hostile to network filesystems and read-only mounts), for an eviction refinement
that barely matters at this scale. Write age is also a decent staleness proxy for this workload:
the entries written longest ago belong to the pull requests viewed longest ago.

**3. Rewrites refresh position.** Writing an entry again (a TTL refresh, a revalidated ETag body,
a re-fetched conversation stamp) goes through the temp-and-rename path, giving the destination a
new mtime and moving it to the young end of the queue. Active PRs therefore keep their metadata
resident while abandoned ones drift toward eviction, which recovers a little of LRU's benefit for
free.

**4. Pruning is best-effort and race-tolerant.** Every failure (`read_dir`, `metadata`,
`remove_file`) is skipped over. Two processes pruning concurrently both make progress; a file
deleted under one pruner's feet just fails its `remove_file` and is not double-counted.

**5. The cost is bounded by the entry cap.** A prune pass over at most 2,048 directory entries is
one `read_dir` walk, one sort of 2,048 tuples, and however many unlinks the budgets demand.
Running it after every write is far cheaper than letting the directory grow unbounded and paying
on a timer.

The budgets also define what the cache *cannot* do, which the pre-stack analysis stated
memorably: everything expensive was session-scoped, patches over 1 MiB never disk-cached, and
with at most 2,048 entries "a 20k-file PR can cache at most ~10% of its patches." The stack's
answer was not to raise the budgets but to lower what a session needs: API-derived counts instead
of blob materialization, an API-resolved merge base instead of a deepening ladder, and batch
reads that fill the patch cache in the order the reader consumes it. The budgets have been
sufficient in practice; the working cache observed during the optimization session sat at 21 MB
against the 128 MiB cap.

## Cache placement and relocation

`cache_root` (`src/git/github/mod.rs:2482`) resolves the cache's home once per access, in strict
priority order:

```rust
fn cache_root() -> Option<PathBuf> {
    if let Some(path) = env::var_os("QUINJET_CACHE_DIR").filter(|path| !path.is_empty()) {
        return Some(PathBuf::from(path));
    }
    #[cfg(windows)]
    {
        if let Some(path) = env::var_os("LOCALAPPDATA").filter(|path| !path.is_empty()) {
            return Some(PathBuf::from(path).join("quinjet").join("cache"));
        }
    }
    if let Some(path) = env::var_os("XDG_CACHE_HOME").filter(|path| !path.is_empty()) {
        return Some(PathBuf::from(path).join("quinjet"));
    }
    #[cfg(target_os = "macos")]
    {
        if let Some(path) = env::var_os("HOME").filter(|path| !path.is_empty()) {
            return Some(
                PathBuf::from(path)
                    .join("Library")
                    .join("Caches")
                    .join("quinjet"),
            );
        }
    }
    env::var_os("HOME")
        .filter(|path| !path.is_empty())
        .map(|path| PathBuf::from(path).join(".cache").join("quinjet"))
}
```

In table form:

| Priority | Source | Resulting root |
|---|---|---|
| 1 | `QUINJET_CACHE_DIR` (nonempty) | the value, verbatim |
| 2 | Windows: `LOCALAPPDATA` | `%LOCALAPPDATA%\quinjet\cache` |
| 3 | `XDG_CACHE_HOME` (nonempty) | `$XDG_CACHE_HOME/quinjet` |
| 4 | macOS: `HOME` | `~/Library/Caches/quinjet` |
| 5 | `HOME` | `~/.cache/quinjet` |

If nothing resolves (a daemon environment with no `HOME`), `cache_root` returns `None` and every
cache helper short-circuits: Quinjet runs correctly with zero caching rather than inventing a
location.

Under the root live two directories:

- `github/`: the entry store this page describes, hash-named `.cache` files.
- `tmp/`: the disposable `pr-<pid>-<id>.git` bare workspaces, removed on drop and swept when
  older than 24 hours ([the PR workspace page](./pr-workspace.md)).

`QUINJET_CACHE_DIR` is the operational lever, and it earns its keep in three distinct uses:

**1. Cold-cache isolation for measurement.** Pointing the variable at a throwaway directory
isolates a run completely, which is exactly how the optimization stack was benchmarked. From the
session notes: `QUINJET_CACHE_DIR=$(mktemp -d) quinjet ...` "points every cache (metadata,
immutable patch/conversation/counts entries, and the disposable pr-*.git workspaces) at a
throwaway root," quoted as "exactly how I benchmarked the before/after numbers." The
[benchmarking page](../benchmarking.md) leans on this heavily.

**2. Relocation.** Machines with small home partitions, shared CI runners, and containerized
environments can point the cache anywhere writable. Because temp files are created inside the
target directory, the atomic-rename guarantee survives relocation to any single filesystem.

**3. Blunt invalidation.** There is deliberately no `quinjet cache clear` verb and no per-repo
subdirectory scheme (both were proposed during the session and consciously deferred). The
supported answer is `rm -rf` on the root, which is always safe: every entry is a re-fetchable
answer, nothing under the root is a source of truth, and the next run rebuilds what it needs. The
filenames being opaque hashes means selective per-repository deletion by name is impossible, a
real limitation traded for the simplicity of the flat store.

The cache root is distinct from the state root (`~/.local/state/quinjet` and relatives), which
holds the recent-projects list: state is meant to survive, cache is meant to be deletable, and
keeping them under different roots keeps that distinction enforceable with a single `rm`.

## The cached indicator

Correctness by construction covers what is stored; the indicator covers what the reader is told.
Invariant 12b requires that the view "says `cached` when what is on screen came from disk." The
decision lives in `App::pull_request_served_from_cache` (`src/app.rs:2975`):

```rust
/// Whether the pull request itself was answered from disk rather than the
/// network. Check state is deliberately held for only thirty seconds, so
/// including it here made the answer almost always false and the label
/// never appeared at all.
pub(crate) const fn pull_request_served_from_cache(&self) -> bool {
    self.pull_request.is_some()
        && self.pull_request_from_cache
        && self.pull_request_conversation.from_cache
        && !self.pull_request_refreshing()
}
```

The definition is a conjunction over provenance flags that each subsystem reports honestly:

- `pull_request_from_cache` derives from the metadata read's `CacheDisposition`
  (`from_cache = disposition != Network`), so both `Fresh` and `Stale` count as cached.
- `PullRequestConversation::from_cache` is true only "when nothing had to be transferred: either
  the thread was already held for this update stamp, or GitHub confirmed it had not changed."
  Note that a 304 revalidation counts as cached, which is the honest reading: the bytes on screen
  came from disk, GitHub merely co-signed them.
- The check list is deliberately excluded, for the reason the doc comment preserves: a 30-second
  TTL means the checks are nearly always network-fresh, and requiring them would suppress the
  label almost permanently, making it useless.
- Any in-flight read suppresses the label in favor of activity reporting.

The rendering (`src/ui/mod.rs:1328`) gives refreshing priority over cached in the sidebar title:

```rust
let cache = if app.pull_request_refreshing() {
    "  · refreshing"
} else if app.pull_request_served_from_cache() {
    "  · cached"
} else {
    ""
};
```

and the content pane's PR title applies the same precedence with its own loading label. The test
`the_pane_says_whether_it_is_refreshing_or_showing_a_cached_answer` (`src/ui/mod.rs:8246`) pins
the tri-state behavior: a refreshing pane never says cached, a disk-served pane says cached with
no refresh glyph, and a network-fresh pane says neither.

The indicator matters to the caching design for a non-obvious reason: it is what makes aggressive
caching socially acceptable. A tool that silently serves disk answers invites distrust the first
time a reader suspects staleness; a tool that labels provenance lets the reader calibrate, and
the explicit reload control (also invariant 12b) gives them the escape hatch. The label plus the
`--refresh` flag are the human-facing halves of the `CacheDisposition` enum.

## The cache as its own index: recent pull requests

The recents feature stores one explicit entry, `recent-pull-requests-v1`, a JSON
`Vec<RecentPullRequest>` capped at `MAX_RECENT_PULL_REQUESTS = 20` and rewritten
most-recent-first on every PR open (`record_recent_pull_request`, `src/git/github/mod.rs:571`).
What makes it interesting for this page is the fallback: `recent_pull_requests`
(`src/git/github/mod.rs:545`) tops the list up by scavenging the cache directory itself.

`CacheStore::cached_pull_requests` (`src/git/github/mod.rs:2386`) scans the store for entries
that *look like* PR metadata:

- Only `.cache` files no larger than `MAX_RECENT_CACHE_ENTRY_BYTES` (384 KiB) are candidates
  (a PR metadata record is title plus body plus fixed fields, bounded well under that; logs and
  patches are filtered out by size alone).
- Candidates are sorted newest-mtime-first and at most `MAX_RECENT_CACHE_SCAN = 256` are
  inspected, so the scan stays cheap even against a full 2,048-entry store.
- `cached_pull_request_at` (`src/git/github/mod.rs:2447`) recognizes a PR record purely by
  shape: strip the magic, take the first nonempty line, parse it as an 18-field TSV, require
  field 0 to parse as a `u64` and field 7 to be a URL ending in `/pull/{number}`. Anything else
  (a patch, a log, a validator entry whose first line is an ETag) fails one of those checks and
  is skipped.

No index file maps hashes back to keys; none is needed, because the entries are self-describing
enough to be recognized. The cache is its own index. The design accepts the corollary costs: a
pruned metadata entry silently drops out of the reconstructed recents, and the recents list is
therefore best-effort below its explicit top-20. Both are the right shape for a convenience
picker backed by a cache that is allowed to forget.

## Why credentials are never cached

Invariant 12a states it as four words, "credentials never are," and the enforcement is
structural rather than a filter: no credential ever enters the layers that write to disk.

**1. Authentication lives in `gh`, not in Quinjet.** Every GitHub request is executed by the
GitHub CLI, which holds its tokens in its own storage and injects them into its own HTTP calls.
Quinjet spawns `gh` with argv and environment (`GH_PROMPT_DISABLED=1`, `GH_PAGER=cat`,
`GH_NO_UPDATE_NOTIFIER=1`, `NO_COLOR=1`) and reads stdout. Tokens never pass through Quinjet's
address space, so they cannot appear in anything Quinjet writes. The cache stores response
bodies; a response body from these endpoints is PR metadata, TSV records, patches, or logs,
never an authentication artifact.

**2. URLs are stripped before they become keys or argv.** Remote URLs are the one place a secret
could ride in: `https://user:token@github.com/owner/repo.git` is a real pattern in CI
environments. `remote_url_for_gh` (`src/git/github/mod.rs:1584`) removes userinfo, query, and
fragment from every scheme URL and canonicalizes scp-style `git@host:path` to `ssh://host/path`
before the URL is grouped, resolved, passed to `gh`, or embedded in a cache key. The test
`strips_credentials_before_passing_remote_urls_to_gh` (`src/git/github/mod.rs:2731`) pins it.
Consequently the `repository\n{identity}` and `pull-request-v3\n{url}\n...` keys are built from
sanitized URLs, and a token in a remote URL never reaches the disk in either a filename hash
input or an entry body.

**3. ETags are fingerprints, not secrets.** The one header the cache does persist is the `ETag`
in validator entries. An ETag is a content fingerprint the server hands to any authorized
client; it grants nothing and identifies nothing beyond the response version it validates.

**4. The modes are defense in depth for content, not a substitute.** Even with no credentials on
disk, the cache holds repository content (patches, file listings, logs, conversation text) at
rest outside any repository, possibly from private repositories. That is why invariant 12a ends
"Because patches are repository content at rest outside the repository, the cache root stays
owner-only," and why the store applies `0o700`/`0o600` on every write rather than trusting the
umask. The privacy modes protect the content that *is* cached; the structural exclusion protects
the credentials that are not. The two rules are independent, and both are tested.

**5. Git's side is covered by the same structure.** The Git subprocesses run with
`GIT_TERMINAL_PROMPT=0`, so a fetch against an auth-requiring remote fails rather than prompting,
and credential helpers remain Git's business in Git's own configuration. Quinjet neither reads
nor stores what the helpers produce; the disposable workspace's fetches authenticate exactly as
the user's own `git fetch` would.

## A full lifecycle trace

A cold immutable read derives a key from every content identity, misses the private store, runs
one bounded producer, validates that the response is complete, writes a temporary neighbor, and
renames it into its hashed destination. A warm read derives the same key and returns the stored
bytes without consulting a clock. A force-push, new conversation stamp, or different job identity
derives another key, so the old entry remains correct for its old question and ordinary pruning
eventually reclaims it.

## Test coverage as a specification

The cache tests assert stable hashing, magic validation, private Unix modes, validator and body
pairing, recent pull-request discovery, per-entry ceilings, and fallback disposition. App and
GitHub tests then cover the boundaries the store cannot prove alone: running logs bypass immutable
caching, moved heads reindex the diff, stale metadata never replaces a newer generation, and
parsed documents evict oldest entries under their independent in-memory budget.

## Staleness findings from the adversarial review

The optimization stack that produced most of these keys (PRs 46 through 50, later 52, 54,
and 55) was adversarially reviewed before merging, and the review's cache-related findings are the
best available stress test of the correctness-by-construction claim. Each finding is a precise
lesson about what "the key names the content" actually requires. They are presented here as they
were found, against the pre-fix code, with the shipped fix and the design lesson.

### Finding: a stale merge-base hint paired with a fresh head

**The bug.** `fetch_pull_request` used the compare-API merge base computed from the metadata
snapshot's `base_oid` and `head_oid`, while the head it returned came from the just-fetched
`refs/pull/N/head`. A force-push landing between the metadata read and the workspace fetch
produced a mismatched pair: the merge base of the *old* head (M_old) paired with the *new* head
(H_new). The changed-file listing for that pair would then be computed and cached immutably under
`pr-files-v1\nM_old\nH_new`.

**Why the cache itself stayed honest.** This finding rewards a careful reading. The cached entry
was not a violation of the immutability contract: `git diff M_old H_new` is deterministic, so the
bytes stored under that key really are the eternal answer to that key's question. The corruption
was one level up: the *pair itself* was wrong for the pull request being displayed. Immutable
keying guarantees that an entry matches its key; it cannot guarantee that the key being asked is
the right question. Key construction is upstream of key correctness.

**The fix.** The hint is now used only when the fetched head still equals the metadata snapshot's
head OID: `preferred_fetched_commit` resolves the exact advertised OID inside the workspace, and
the merge-base shortcut at `src/git/github/mod.rs:1834` returns early only when that resolution
matches. A force-push in the window makes the shortcut decline, and the fallback ladder computes
the merge base against whatever head was actually fetched, keeping the pair coherent. The pair
that reaches `changed_files_in_repository`, and therefore every `pr-files-v1`, `pr-numstat-v1`,
and `pr-patch-v1` key, is now taken from one consistent observation.

### Finding: the counts key omitted the base identity

**The bug.** The first version of the API file-counts key was keyed by repository URL, PR number,
and head OID only. But per-file additions and deletions are a function of *both* ends of the
comparison: retarget the PR to a different base branch, or reset the base branch, and GitHub's
counts change while the head OID (and thus the key) stays the same. An `Immutable` entry whose
answer could change under a fixed key is precisely the failure the whole design exists to
prevent. The sibling `pr-merge-base-v1` key had included `base_oid` from the start, so the two
caches could diverge on retarget: fresh merge base, stale counts.

**The fix.** The key gained the base commit and a version bump:
`pr-file-counts-v3\n{url}\n{number}\n{base}\n{head}`. Old v1-keyed entries became unreachable
(see the next section for why that is the whole migration story), and the invariant was restored:
every input that can change the answer is spelled in the key.

**The lesson, stated as a checklist.** For every immutable key, enumerate the inputs of the
computation and check them off against the key fields. `pr-patch-v1` passes: merge base, head,
path, and a pinned flag set fully determine a patch. `check-log-v1` passes: a job id names a
frozen artifact. The counts key failed because one input (the base) was implicit in "the PR as
currently configured," and PR configuration is mutable. Anything mutable referenced by a key must
be resolved to an immutable observation *before* keying, never after.

### Finding: a truncated validated page could be cached as complete

**The bug.** In the first version of `validated_gh`, `ValidatedRead` had no `truncated` field.
The bounded runner kills `gh` when stdout crosses the 2 MiB cap, but there is a race: `gh` can
exit successfully at the exact moment the cap is reached, yielding a clean exit status over a cut
body. A page-one conversation read cut mid-record would then either fail TSV parsing (failing the
whole conversation load) or, if the cut landed inside the final field, parse cleanly and be
cached under the stamp key with the `complete` marker: up to roughly a page of comments silently
missing, served as complete until the next `updatedAt` change moved the stamp.

**The fix.** Truncation became a first-class field on `ValidatedRead`, the ETag store gained the
`filter(|_| complete)` guard quoted earlier, and a truncated or failed first page now degrades to
the bounded page loop instead of either failing or lying. The conversation content cache keeps
its own honesty bit (the `complete`/`partial` marker) so even a legitimately capped read is
stored with its incompleteness recorded.

**The lesson.** Completeness is an input too. An immutable key names an answer; a partial answer
under that key is a different value wearing the right name. Every producer in the current code
therefore carries an explicit completeness signal through to the cache decision:
`!stdout_truncated` for the wrapper and the Git streams, `complete` for paged accumulations, the
marker line for capped conversations, and nonemptiness for logs.

### Findings one level up: the render cache

The same review round confirmed a cluster of staleness bugs in a different cache entirely: the
in-memory overview-row layout cache in the UI (a failed lookup never invalidated cached rows, a
theme change left rows in the old palette, a repeated checks error froze the summary, an
auto-expand mutated state without invalidating, a running step's elapsed label froze). Those
belong to [the viewport page](../rendering/viewport.md) rather than here, but they earn a mention
for the contrast: the disk cache came through the review with key-construction findings, while
the render cache's findings were all invalidation findings. The disk cache has no invalidation
logic to get wrong, which is the design working exactly as intended; where it was wrong, it was
wrong about identity, and identity bugs are findable by reading a key next to its computation.
That auditability is itself an argument for the construction.

## Version bumps as schema migration

Several keys wear version suffixes: `pull-request-v3`, `pr-file-counts-v3`,
`conversation-timeline-v2`, `pr-merge-base-v1`, `checks-v1`. The suffix is the entire schema
migration system, and its semantics are worth spelling out because they are easy to mistake for
decoration.

**1. A bump is a rename of the question.** Changing `pr-file-counts-v1` to a key that includes
the base OID changes every hashed filename derived from it. Old entries are not deleted, not
converted, not even looked at: they simply stop being addressed, because no current code
constructs their keys. They become unreachable bytes that age toward the oldest end of the mtime
order and fall to the pruner as new writes demand space. The worst case cost of a bump is
bounded by the store budget itself: at most 128 MiB of orphans, reclaimed incrementally, with
zero migration code.

**2. Bumps answer both schema changes and semantics changes.** `pull-request-v3` reflects the
18-field TSV record shape: a build reading 18 fields must never parse a 16-field entry written
by an older build, and distinct keys make cross-version reads structurally impossible rather
than "handled." `pr-file-counts-v3` reflects a semantics change: the key's meaning grew an input.
Both kinds of change get the same one-character treatment.

**3. The alternative was worse in every direction.** In-band schema versioning (a version byte
inside the entry) would require every reader to parse before trusting, migration code for each
transition, and a story for downgrade (an old build reading a new entry). Cross-version
compatibility for a cache is all cost: the payoff of a successful migration is saving one
refetch per entry, and the price of a failed one is a correctness bug. Refetching is cheap by
construction here, so the design buys correctness with bandwidth.

**4. The store magic is the outermost ring of the same scheme.** `quinjet-gh-cache-v1\n` versions
the container format (magic, payload, hashing scheme) the way key suffixes version each payload
schema. A future incompatible store change bumps the magic and orphans everything at once, which
is acceptable for the same reason single-key bumps are.

**5. The oversized-delete rule handles the third kind of change.** Schema changes get key bumps;
limit *reductions* get the read-path rule that deletes any entry larger than the current limit.
Between the two, every kind of format evolution has a self-healing answer that involves no
migration pass, no startup scan, and no version negotiation.

## Interaction with prefetch and progressive loading

The cache does not fill itself; the prefetch machinery fills it, and the order in which it does
so went through a documented evolution across the stack. The cache's role in that story is to
make every background byte pay forward.

**1. Batched reads write per-file entries as a side effect.** `diff_files`
(`src/git/github/mod.rs:440`) answers a batch of paths with one `git diff` invocation, splits the
combined patch at its `diff --git` boundaries, and writes each complete section into its own
`pr-patch-v1` entry (bounded by the 1 MiB ceiling). A batch requested by background prefetch
therefore leaves behind exactly the entries a later foreground selection will hit: the prefetch
pipeline and the click-to-open path converge on the same cache lines. Only the final section of a
truncated combined patch is ever suspect, and that section is either retried alone or surfaced
marked truncated, never cached.

**2. The ordering evolution: #50, then #55.** PR #50 introduced size-tiered ordering: on huge
pull requests (at the time, 100,000 or more total changed lines, or 1,000 or more files),
prefetch candidates were sorted by estimated patch size ascending, so the byte budget filled with
the smallest files first and the cache accumulated the most files per request. PR #55 replaced
that ordering with viewport-anchored wrap-around fill: the batch walk now starts at the first
file visible in the Files tree and wraps around the whole index, so the patches that land first
(in memory and in the cache) are the ones the reader is looking at, and the smallest-first tier
constants were removed along with the old 400-file prefetch stop. The current behavior, per
invariant 5: background prefetch walks the whole index up to 4,096 files, starting at the file
the Files tree is showing and wrapping around the rest in order, sizing each batch by per-file
count estimates to stay under the 8 MiB patch read.

**3. The estimates that size the batches come from cached counts.** Each batch takes at most
`PULL_REQUEST_PREFETCH_BATCH = 32` files and stops adding files when
`estimated_patch_bytes` would push the batch past the
`PULL_REQUEST_PREFETCH_BYTE_BUDGET = 6 MiB` estimate budget (`src/app.rs:33` onward). The
estimate is `(additions + deletions) * 80 + 4_096` bytes per file
(`PULL_REQUEST_PATCH_LINE_ESTIMATE = 80`), falling back to
`PULL_REQUEST_PATCH_FALLBACK_ESTIMATE = 512 KiB` for a file with no counts. Those counts are the
very ones cached under `pr-file-counts-v3` or `pr-numstat-v1`: the cache feeds the scheduler that
fills the cache. The 6 MiB estimated budget keeps the real combined patch comfortably under the
8 MiB `MAX_DIFF_BYTES` truncation cap, so batches rarely produce the uncacheable truncated tail
case at all. The full scheduling story, including the anchor computation and the workspace-keyed
replies, is on [the prefetch page](./prefetch.md); the render-side consequences are on
[the progressive loading page](../rendering/progressive-loading.md).

**4. Warm reads fill the check-log cache the same way.** The warm lane's
`prefetch_check_run_logs` pass is nothing but the foreground log read run eagerly over up to 32
settled jobs, so every warmed job lands in `check-steps-v1` and `check-log-v1` and "selecting a
check then costs a disk read rather than a round trip." The lane isolation that keeps this
warming from ever delaying an interactive read is described in
[the concurrency page](../rendering/concurrency.md).

**5. Session lifetimes make the disk cache the only cross-session memory.** Invariant 14: the
terminal keeps a session per worker lane and pays for a prepared pull request once, while a CLI
subcommand builds a session, prepares, reads, and drops it, relying on the immutable per-file
caches instead. For `quinjet pr diff 30412 <path>` run twice in a row, the second invocation's
workspace preparation is served by `pr-merge-base-v1`, `pr-file-counts-v3`, and `pr-files-v1`
hits, and the patch itself by `pr-patch-v1`, so the repeat costs process startup plus disk reads
rather than fetches. The cache is what makes the drop-everything session model affordable.

## Measured effect

Every number in this section is quoted from the optimization session's benchmark notes, measured
against oven-sh/bun PR #30412 ("Rewrite Bun in Rust": 2,188 changed files, +1,009,257 added
lines) from a shallow `blob:none` clone, with cold runs isolated via `QUINJET_CACHE_DIR` pointed
at a fresh temporary directory. The full methodology lives on
[the benchmarking page](../benchmarking.md); this section extracts what the numbers say about the
cache specifically.

From the first verification round, at the top of the original five-PR stack:

- "Metadata in 1.7s" (`pr view` against bun#30412, cold).
- "The rewrite PR enumerates all 2,188 files with real counts in 18.5s cold" (`pr files`, cold
  cache, includes workspace prepare).
- Warm re-run of the index: 0.04s.
- Single-file patches: 0.1s.
- "the 1,100-entry conversation in 21s with the newest activity preserved."

From the second verification round, after the review fixes and restack, on the final binary:

- "Final numbers on the bun PR: cold index 6.3s, warm 0.04s, conversation 26s with the honest
  truncation notice."

And after installing the top-of-stack build locally, with warm metadata and the real cache:

- "Smoke-tested from the bun clone: `q pr files 30412` lists all 2,188 files of the 1M-line
  rewrite PR in 1.4s."

Reading the numbers through the cache design:

**1. The cold/warm gap is the immutable tier working.** 18.5s (later 6.3s) cold against 0.04s
warm is the difference between paying for metadata, workspace fetches, and enumeration versus
answering `pr-files-v1` and the counts entries from disk. The warm number is essentially process
startup plus a few file reads, which is the designed floor: an immutable hit costs a stat, a
read, and a parse.

**2. The cold-side improvement came from asking cheaper questions, not caching harder.** The
18.5s-to-6.3s movement arrived with the review-fix round, in a codebase where the dominant costs
had already moved to API metadata (counts from the pulls files endpoint, merge base from the
compare API). Cold time is bounded by what must genuinely be fetched; the cache's job is to make
sure it is fetched once.

**3. The conversation regression was purchased honesty.** 21s to 26s on the conversation is
recorded in the session as the cost of the fixed code degrading honestly rather than caching a
gapped page-one read: the pre-fix number was faster partly because it could cache an incomplete
answer as complete. The extra five seconds is what the completeness guard costs on a
1,100-entry thread, paid once per stamp; every subsequent read within the stamp is a disk hit,
and an unchanged thread revalidates for one free 304.

**4. Per-file reads sit at interactive latency either way.** 0.1s single-file patches on the
cold path (a path-scoped `git diff` in the workspace) and effectively instant on a `pr-patch-v1`
hit mean file navigation never blocks on the cache being warm; warmth converts a fast operation
into a free one, and prefetch makes most of them warm before they are asked for.

## Design alternatives and why they lost

Every piece of this design displaced an alternative. Recording the losers and their reasons is
the cheapest insurance against relitigating them.

**1. An embedded database (SQLite, LMDB, sled).** A single-file store would offer transactions,
real LRU, and per-repository queries. It lost on every axis that matters here: it adds a heavy
dependency to a tool that otherwise shells out to `git` and `gh`; it concentrates corruption
risk in one file whose recovery story is the library's, not the filesystem's; it needs its own
locking discipline across the several Quinjet processes that may share a cache root, where the
flat store gets multi-process safety for free from `O_EXCL` temp files and atomic rename; and it
makes the cache opaque to inspection and to the always-safe `rm -rf`. Flat files made the store's
entire failure model "a file is missing or ignored," which composes with best-effort helpers
into a system with no cache-induced error states at all.

**2. Human-readable filenames.** Encoding keys into names (percent-escaping newlines and
slashes) would make the directory browsable and per-repo deletion possible. It lost to three
hard problems the hash avoids: filename length limits (keys embed URLs and repository-relative
paths of unbounded length), byte-set restrictions across filesystems, and the risk class of
attacker-influenced path bytes reaching filename construction. The hash is also what makes every
name exactly 38 characters, keeping directory operations uniform. The cost, acknowledged above,
is that selective deletion by name is impossible.

**3. Per-repository subdirectories.** Proposed during the session as the enabler for a
`quinjet cache clear --repo <url>` verb, and consciously deferred. It would complicate the
pruner (budgets per directory or a two-level walk), the scavenger, and the key-to-path mapping,
to serve a maintenance operation that `rm -rf` on the whole root already covers safely. If the
verb is ever built, the subdirectory layout is the obvious shape for it; nothing in the current
key design blocks the migration, since a layout change is a store-magic bump away.

**4. TTLs for everything.** The uniform-TTL cache is the industry default and was effectively
the pre-stack state of affairs at this scale: everything expensive was session-scoped or
short-lived, and the pre-stack analysis found reopening a huge PR repeated the full fetch and
enumeration. Uniform TTLs force a single trade between staleness and traffic; the split lets
immutable content take the infinite-TTL branch that is actually correct for it, and confines the
trade to three small metadata reads. The general technique comparison lives in
[the techniques catalog](../techniques.md).

**5. Validating everything with ETags.** Validation without content keys would keep every entry
honest at one round trip per read, which is exactly the latency the immutable tier exists to
delete. ETags earn their place only where content genuinely drifts under a stable question (the
conversation streams) and are subordinate there to the stamped content cache that answers
repeat reads for free.

**6. Delegating to the GitHub CLI's response cache.** `gh` has a per-URL TTL cache of its own.
It lost for the same reasons as uniform TTLs, plus coverage: it cannot express immutable OID
keys, cannot serve explicitly labeled stale answers on failure, offers no size governance, and
covers none of the Git-produced streams (`pr-files-v1`, `pr-numstat-v1`, `pr-patch-v1`) that
dominate the byte budget. Owning the store also makes the provenance tri-state possible, which
`gh`'s transparent caching would hide.

**7. Touch-on-read LRU.** Covered in [the eviction section](#eviction-128-mib-2048-entries-oldest-first):
a metadata write per cache hit buys a marginally better eviction order that a 128 MiB budget
over PR-viewing workloads does not need.

**8. Caching running-job output with a short TTL.** Rejected on correctness, not cost: it
converts the tail into a stutter and races the settle transition. The
[running-job section](#never-caching-a-running-job) carries the full argument.

**9. An in-memory cache layer above the disk.** The parsed-document budget in the app
(32 MiB of `DiffDocument`s) already serves the role of a hot tier for the current session, and
the disk tier's read cost is a stat plus a bounded read. A general memory cache between them
would duplicate the document cache's job while complicating invalidation (the one problem this
design refuses to have).

## Edge cases and failure modes

A catalog of the store's behavior at the boundaries, each traceable to a specific line quoted
earlier.

**1. No resolvable cache root.** `cache_root()` returns `None`; `CacheStore::discover` returns
`None`; every helper short-circuits. Quinjet runs fully cacheless: correct, slower, silent.

**2. Read-only or full cache filesystem.** Writes fail inside `cache.write` and are dropped by
the fire-and-forget wrappers. Reads of existing entries continue to work. The pruner's failed
unlinks are skipped. No user-visible error in any of these cases.

**3. Clock skew.** An mtime in the future makes `duration_since` fail, and the age defaults to
zero: the entry reads as brand new. For immutable entries this is exactly correct. For TTL
entries, a backwards clock jump can extend an entry's effective life by the size of the jump,
bounded by the largest TTL (24 hours) in effect; the guarded reads are convenience metadata, so
the failure mode is a stale repo identity or PR title, both already bounded by design. The
converse skew (mtime far in the past) only accelerates expiry and pruning, which is always safe.

**4. Two processes, one store.** Invariant 14 allows concurrent Quinjet processes. Temp names
embed pid and counter, `create_new` makes collision an error rather than interleaving, rename is
last-writer-wins with both candidates being valid entries for the same key, and pruning is
idempotent under races. The one benign anomaly: both processes can fetch the same miss
concurrently and write twice; the second rename simply replaces an identical entry.

**5. Hash collisions.** Two keys sharing a 128-bit name would alias one file. With the store
capped at 2,048 entries, a birthday estimate gives roughly `2_048^2 / 2^129` collision
probability, on the order of one in `10^32`: not a design consideration, and even on collision
the failure is contained, because the reader of the losing key gets bytes that fail its own
magic-plus-parse validation path and degrade to a miss (for TSV and OID-validated entries) or,
at worst, a wrong-but-well-formed payload for same-format keys, which the next write overwrites.

**6. Pruning races a read.** `read` stats, then reads. A pruner (or the oversized-delete rule in
another process) can unlink between the two; `fs::read` fails and the read returns `None`, a
clean miss.

**7. Entry exactly at the limit.** The size check allows `limit + CACHE_MAGIC.len()`, so a
payload of exactly `limit` bytes round-trips. One byte over is never written by this build and
is deleted on read if an older build wrote it.

**8. Orphaned temp files.** A crash between open and rename leaves `.write-*.tmp` files that
readers never address and the pruner never counts. They are bounded in practice by crash
frequency, invisible to correctness, and removed by any manual cache wipe. This is the accepted
cost of not running a startup sweeper over the store.

**9. Coarse mtime granularity.** Filesystems with second-granularity mtimes make the prune
order approximate among entries written within the same second, and can make a
same-second TTL check borderline. Both effects are harmless: prune order among near-simultaneous
entries is arbitrary anyway, and the shortest real TTL is 30 seconds.

**10. `QUINJET_CACHE_DIR` pointed at a foreign directory.** The store only ever creates, reads,
and deletes files matching its own naming inside `github/`, ignores wrong-magic files rather
than deleting them, and applies `0o700` to directories it creates. Pointing the variable at a
directory with unrelated `.cache` files is still inadvisable (the pruner considers any `.cache`
file fair game by mtime), which is why the documented pattern is a dedicated directory, ideally
a fresh one per isolation run.

**11. A `Ttl(Duration::ZERO)` entry on a network failure.** The running-job steps entry is
written but never fresh. When the network fails mid-run, the cache-through wrapper's failure
branch serves it as `Stale`: the reader sees the last known step list instead of an empty pane,
labeled as cached. This is the smallest, strangest member of the stale-fallback family, and it
falls out of the same five-line ladder with no special code.

**12. The cache outliving the repository.** Entries keyed by OIDs or job ids for a repository
the user deletes locally, or a PR that is closed, remain valid answers to their questions and
simply age out through pruning as new activity displaces them. Merged and closed PRs are not
polled at all (invariant 11), so their entries stop being refreshed and drift to the old end of
the queue on their own. No reference counting, no garbage collection beyond the budgets: the
store forgets at the rate it learns, which for a cache is the only forgetting that matters.

## Optimization review matrix

Use this matrix during performance reviews. Each row combines a cost lens, repository context, and observable signal without claiming that every combination needs a standalone benchmark.

| ID | Review condition | Evidence to capture |
| ---: | --- | --- |
| 1 | Check latency for Caching: Correctness by Construction in a small local repository | Record time to first useful rows |
| 2 | Check latency for Caching: Correctness by Construction in a small local repository | Record steady frame cost |
| 3 | Check latency for Caching: Correctness by Construction in a small local repository | Record bytes accepted from child output |
| 4 | Check latency for Caching: Correctness by Construction in a small local repository | Record Git and gh process count |
| 5 | Check latency for Caching: Correctness by Construction in a small local repository | Record maximum retained document bytes |
| 6 | Check latency for Caching: Correctness by Construction in a small local repository | Record cache disposition and complete key |
| 7 | Check latency for Caching: Correctness by Construction in a small local repository | Record stale reply rejection |
| 8 | Check latency for Caching: Correctness by Construction in a small local repository | Record visible state after failure |
| 9 | Check latency for Caching: Correctness by Construction in a monorepo with many changed paths | Record time to first useful rows |
| 10 | Check latency for Caching: Correctness by Construction in a monorepo with many changed paths | Record steady frame cost |
| 11 | Check latency for Caching: Correctness by Construction in a monorepo with many changed paths | Record bytes accepted from child output |
| 12 | Check latency for Caching: Correctness by Construction in a monorepo with many changed paths | Record Git and gh process count |
| 13 | Check latency for Caching: Correctness by Construction in a monorepo with many changed paths | Record maximum retained document bytes |
| 14 | Check latency for Caching: Correctness by Construction in a monorepo with many changed paths | Record cache disposition and complete key |
| 15 | Check latency for Caching: Correctness by Construction in a monorepo with many changed paths | Record stale reply rejection |
| 16 | Check latency for Caching: Correctness by Construction in a monorepo with many changed paths | Record visible state after failure |
| 17 | Check latency for Caching: Correctness by Construction in a pull request containing generated files | Record time to first useful rows |
| 18 | Check latency for Caching: Correctness by Construction in a pull request containing generated files | Record steady frame cost |
| 19 | Check latency for Caching: Correctness by Construction in a pull request containing generated files | Record bytes accepted from child output |
| 20 | Check latency for Caching: Correctness by Construction in a pull request containing generated files | Record Git and gh process count |
| 21 | Check latency for Caching: Correctness by Construction in a pull request containing generated files | Record maximum retained document bytes |
| 22 | Check latency for Caching: Correctness by Construction in a pull request containing generated files | Record cache disposition and complete key |
| 23 | Check latency for Caching: Correctness by Construction in a pull request containing generated files | Record stale reply rejection |
| 24 | Check latency for Caching: Correctness by Construction in a pull request containing generated files | Record visible state after failure |
| 25 | Check latency for Caching: Correctness by Construction in a deeply diverged branch | Record time to first useful rows |
