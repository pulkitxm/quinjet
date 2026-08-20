# The Object Model: Git's Content-Addressable Store from Bytes Up

Every optimization in Quinjet's pull-request pipeline ultimately rests on one property of Git's
object store: an object ID is a cryptographic hash of the object's bytes, so the same ID always
names the same content, everywhere, forever. This page builds that store up from raw bytes: the
`"type size\0content"` hashing rule, the four object types byte by byte, SHA-1 and SHA-256 formats,
loose object storage, and the lookup path through packs, alternates, and promisor remotes. It then
walks through the exact Quinjet code that leans on each property: the `cat-file -e` probe that makes
local-branch pull-request previews network-free, the OID-keyed cache entries that never expire, the
`objects/info/alternates` trick that lends the opened repository's objects to the disposable PR
workspace, and the decision to read per-file line counts from the GitHub API rather than
materializing blobs inside a `blob:none` workspace.

## Contents

- [Content addressing in one rule](#content-addressing-in-one-rule)
- [The four object types, byte by byte](#the-four-object-types-byte-by-byte)
- [SHA-1, SHA-256, and what counts as an OID](#sha-1-sha-256-and-what-counts-as-an-oid)
- [Loose object storage](#loose-object-storage)
- [The object lookup path](#the-object-lookup-path)
- [cat-file: the plumbing window into the store](#cat-file-the-plumbing-window-into-the-store)
- [Immutability as an engineering property](#immutability-as-an-engineering-property)
- [Quinjet: the network-free fast path](#quinjet-the-network-free-fast-path)
- [Quinjet: caches keyed by OID never expire](#quinjet-caches-keyed-by-oid-never-expire)
- [Quinjet: borrowing the opened store through alternates](#quinjet-borrowing-the-opened-store-through-alternates)
- [Quinjet: API counts instead of blob materialization](#quinjet-api-counts-instead-of-blob-materialization)
- [Design alternatives and why they lost](#design-alternatives-and-why-they-lost)
- [Edge cases and failure modes](#edge-cases-and-failure-modes)
- [Where to go next](#where-to-go-next)

## Content addressing in one rule

Git's database has exactly one storage rule. To store any piece of content, Git:

1. Prepends a header of the form `<type> <size-in-bytes>` followed by a NUL byte (0x00).
1. Hashes the header plus the content with the repository's hash function (SHA-1 or SHA-256).
1. Uses the hex form of that hash as the object's name, its object ID (OID).
1. Stores the zlib-compressed header-plus-content under a path derived from the OID.

The header is part of the hashed bytes, which is why the same content stored as a blob and as
something else would produce different IDs, and why an object's type and size can be recovered
without decompressing the whole body.

The rule is easy to reproduce with nothing but a shell. The three most useful anchor values, each
verifiable on any machine:

```console
$ printf 'blob 0\0' | sha1sum
e69de29bb2d1d6434b8b29ae775ad8c2e48c5391  -
$ printf 'tree 0\0' | sha1sum
4b825dc642cb6eb9a060e54bf8d69288fbee4904  -
$ printf 'blob 12\0hello world\n' | sha1sum
3b18e512dba79e4c8300dd08aeb37f8e728b8dad  -
```

The first value is the ID of every empty file ever committed to any SHA-1 Git repository. The
second is the ID of the empty tree. The third is the ID of a blob holding the twelve bytes
`hello world\n`, and `git hash-object` agrees:

```console
$ printf 'hello world\n' | git hash-object --stdin
3b18e512dba79e4c8300dd08aeb37f8e728b8dad
```

Four consequences follow from this one rule, and the rest of this page is really an exploration of
them:

**1. Identity is content.** Two objects with the same ID have the same bytes. There is no version
counter, no timestamp, no origin field. An OID fetched from a server, computed locally, or read
from a cache entry all name the same bytes or the hash function is broken.

**2. Objects are immutable.** You cannot change an object; you can only write a different object
with a different ID. "Amending" a commit writes a new commit object. "Force-pushing" a branch moves
a ref to point at different objects. The objects themselves never change.

**3. Storage is automatically deduplicated.** A file that appears identically in ten thousand
commits is stored once, because every commit's tree ultimately references the same blob ID.

**4. Verification is free.** Reading an object and rehashing it proves integrity. A corrupted
loose file or pack entry is detected the moment it is read, not at some later audit.

Quinjet's whole caching and fetching strategy is an exercise in taking these consequences
seriously: if an OID names its content forever, then a cache entry keyed by OIDs can never be
stale, a locally present OID never needs the network, and any repository holding both endpoint
commits of a diff can produce byte-identical patch text.

## The four object types, byte by byte

Git stores exactly four object types: `blob`, `tree`, `commit`, and `tag`. (Packfiles add two
pseudo-types for delta encoding, `ofs-delta` and `ref-delta`; those are wire and storage artifacts,
not objects, and are covered in [packfiles and deltas](./packfiles-and-deltas.md).) Everything a
repository contains, every file, directory, revision, and release marker, is one of these four.

This section uses a small demonstration repository built with pinned author and committer values,
so every hash shown is real and reproducible:

```bash
git -c init.defaultBranch=main init --quiet /tmp/quinjet-object-demo
cd /tmp/quinjet-object-demo
printf 'hello world\n' > hello.txt
git add hello.txt
GIT_AUTHOR_DATE='2026-08-20T12:00:00+05:30' GIT_COMMITTER_DATE='2026-08-20T12:00:00+05:30' \
GIT_AUTHOR_NAME='Quinjet Docs' GIT_AUTHOR_EMAIL='docs@example.invalid' \
GIT_COMMITTER_NAME='Quinjet Docs' GIT_COMMITTER_EMAIL='docs@example.invalid' \
git -c commit.gpgsign=false commit --quiet --message 'demo: one file'
```

That commit produces exactly three objects, one of each container type plus the blob:

```console
$ find .git/objects -type f | sort
.git/objects/3b/18e512dba79e4c8300dd08aeb37f8e728b8dad
.git/objects/68/aba62e560c0ebc3396e8ae9335232cd93a3f60
.git/objects/c7/160ef5bcae6b4482af701d182e92364f672fb8
```

### Blobs

A blob is file content and nothing else. No name, no mode, no timestamp; those all live in the
tree entries that point at the blob. The stored bytes are:

```text
"blob" SP <decimal content length> NUL <content bytes>
```

For `hello.txt` the hashed byte sequence is `blob 12\0hello world\n`, twenty bytes total, and its
SHA-1 is the `3b18e5...` value computed above. Because the name is not part of the blob, renaming a
file creates no new object at all; only the tree changes. This is also why Git detects renames by
similarity rather than by recording them: the object model has nowhere to record a rename, so
`--find-renames` reconstructs them after the fact. See
[diff algorithms](../diff/algorithms.md) for how that scoring works.

A blob has no internal structure. Git does not know or care whether the bytes are UTF-8, UTF-16, a
PNG, or a 2 GiB tarball; line-ending conversion and filters happen on the way in and out of the
index, never inside the store.

### Trees

A tree is a directory listing: a sequence of entries, each pointing at a blob (a file) or another
tree (a subdirectory). The stored format is:

```text
"tree" SP <decimal length> NUL
<entry>*

entry = <mode as ASCII octal, no leading zeros> SP <name bytes> NUL <raw hash bytes>
```

Two details of the entry format matter:

- The hash is stored raw, not hex: 20 bytes in a SHA-1 repository, 32 bytes in a SHA-256
  repository. Tree size is where the two formats diverge on disk.
- Entries are sorted by name, but directories sort as if their name ended with `/`. The name
  `foo` as a directory sorts after the file `foo.txt` (because `foo/` is greater than `foo.txt`
  byte-wise), while the file `foo` sorts before it. An unsorted tree is invalid; `git fsck` flags
  it, and a tree with the same entries in a different order would hash differently, breaking
  deduplication.

The demo repository's root tree, dumped raw:

```console
$ git cat-file tree 'HEAD^{tree}' | xxd
00000000: 3130 3036 3434 2068 656c 6c6f 2e74 7874  100644 hello.txt
00000010: 003b 18e5 12db a79e 4c83 00dd 08ae b37f  .;......L.......
00000020: 8e72 8b8d ad                             .r...
```

Thirty-seven bytes: the ASCII text `100644 hello.txt`, a NUL, then the twenty raw bytes of the
blob's SHA-1 (`3b 18 e5 12 ...`). Rehashing with the header proves the tree's own ID:

```console
$ { printf 'tree 37\0'; git cat-file tree 'HEAD^{tree}'; } | sha1sum
68aba62e560c0ebc3396e8ae9335232cd93a3f60  -
```

which matches `git rev-parse 'HEAD^{tree}'` exactly.

Modes are drawn from a tiny fixed set; Git does not store arbitrary permission bits:

| Mode bytes | Meaning | Points at |
|---|---|---|
| `100644` | Regular file | blob |
| `100755` | Executable file | blob |
| `120000` | Symbolic link (target path is the blob content) | blob |
| `40000` | Directory (printed as `040000` by porcelain) | tree |
| `160000` | Gitlink (submodule commit reference) | commit in another repository |

The pretty-printer restores readability, resolving each raw hash to hex and each mode to a type.
Note that `-p` output separates the hash from the name with a tab character (rendered as spaces
here):

```console
$ git cat-file -p 'HEAD^{tree}'
100644 blob 3b18e512dba79e4c8300dd08aeb37f8e728b8dad    hello.txt
```

Trees compose recursively: a repository with `src/main.rs` has a root tree containing one `40000`
entry named `src` whose hash names a second tree, which contains a `100644` entry named `main.rs`.
Changing `main.rs` therefore writes a new blob, a new `src` tree, and a new root tree: the change
bubbles up the path to the root, and everything off that path is shared with the previous version
by reference. This "bubble up, share the rest" shape is what makes Git snapshots cheap, and it is
the structural reason a `git diff` between two commits only needs to open the subtrees whose hashes
differ.

### Commits

A commit binds a root tree to history and authorship. Its body is line-oriented ASCII headers, a
blank line, and the free-form message:

```text
"commit" SP <decimal length> NUL
"tree" SP <hex tree oid> LF
("parent" SP <hex commit oid> LF)*
"author" SP <name> SP "<" <email> ">" SP <unix seconds> SP <tz offset> LF
"committer" SP <name> SP "<" <email> ">" SP <unix seconds> SP <tz offset> LF
(optional headers: "encoding", "gpgsig", others) LF
LF
<message bytes>
```

The demo commit, printed and then re-hashed manually:

```console
$ git cat-file -p HEAD
tree 68aba62e560c0ebc3396e8ae9335232cd93a3f60
author Quinjet Docs <docs@example.invalid> 1787207400 +0530
committer Quinjet Docs <docs@example.invalid> 1787207400 +0530

demo: one file
$ git cat-file -s HEAD
185
$ { printf 'commit 185\0'; git cat-file commit HEAD; } | sha1sum
c7160ef5bcae6b4482af701d182e92364f672fb8  -
```

Observations worth carrying forward:

- A root commit simply has zero `parent` lines; a merge commit has two or more. Quinjet's history
  parser reads the parent list from `%P` output rather than from raw objects, but the underlying
  data is exactly this header list. See [merge bases and history](./merge-bases-and-history.md).
- Timestamps are integer seconds plus a timezone offset. The sub-second precision mismatch that
  Quinjet's check-log step attribution has to bridge (runner log lines carry sub-second precision,
  the steps API reports whole seconds) does not originate here, but this is why nothing in Git
  itself can disambiguate two commits created in the same second: object identity, not time, is
  the ordering primitive.
- The commit references its tree and parents by hex OID inside the body. The commit's own hash
  therefore covers its entire reachable snapshot and history: change any byte of any file in any
  ancestor and the commit ID changes. This is the Merkle property examined below in
  [Immutability as an engineering property](#immutability-as-an-engineering-property).

### Annotated tags

The fourth type wraps another object with a name, a tagger, and a message:

```text
"tag" SP <decimal length> NUL
"object" SP <hex oid> LF
"type" SP <"commit" | "tree" | "blob" | "tag"> LF
"tag" SP <tag name> LF
"tagger" SP <name> SP "<" <email> ">" SP <unix seconds> SP <tz offset> LF
LF
<message bytes, optionally followed by a signature block>
```

From the demo repository:

```console
$ git cat-file -p v1
object c7160ef5bcae6b4482af701d182e92364f672fb8
type commit
tag v1
tagger Quinjet Docs <docs@example.invalid> 1787207400 +0530

demo tag
```

A lightweight tag, by contrast, is not an object at all: it is only a ref (a name in `refs/tags/`)
pointing directly at a commit. This distinction matters when peeling: `v1^{commit}` must
dereference the tag object to reach the commit, which is exactly what the `^{commit}` suffix in
Quinjet's `cat-file -e` probe does, covered below. Tag objects can nest (a tag of a tag), and the
peel operator dereferences until it reaches the requested type or fails.

### The complete picture for one commit

The three objects of the demo repository form this graph:

```text
refs/heads/main
      |
      v
commit c7160ef5...   "demo: one file"
      |
      | tree
      v
tree 68aba62e...     [100644 hello.txt -> 3b18e5...]
      |
      | entry
      v
blob 3b18e512...     "hello world\n"
```

Nothing points upward. A blob does not know which trees reference it; a tree does not know which
commits use it as a root; a commit does not know which refs or child commits point at it. All
traversal is parent-ward and content-ward, which is why answering "which commits touched this
file" requires walking history, while answering "what did the project look like at this commit" is
a direct descent. Quinjet's PR pipeline only ever needs the second kind of question: given two
commit OIDs, descend both trees and compare. That question needs no refs, no reflogs, and no
history beyond the two commits themselves, a fact the whole fetch strategy in
[shallow and partial clone](./shallow-and-partial-clone.md) exploits.

## SHA-1, SHA-256, and what counts as an OID

### Two hash widths, one object model

Git supports two object formats. The historical one hashes with SHA-1: 160-bit digests, 40 hex
characters. Repositories initialized with `--object-format=sha256` (recorded as
`extensions.objectFormat = sha256` in the repository config) hash with SHA-256: 256-bit digests,
64 hex characters. The object model is unchanged; only the hash function, the raw hash width
inside tree entries and pack indexes, and therefore every OID differ. The same demonstration
content in a SHA-256 repository:

```console
$ git init --object-format=sha256 --quiet /tmp/quinjet-sha256-demo
$ cd /tmp/quinjet-sha256-demo
$ printf 'hello world\n' | git hash-object --stdin -w
0bd69098bd9b9cc5934a610ab65da429b525361147faa7b5b922919e9a23143d
$ printf 'blob 12\0hello world\n' | sha256sum
0bd69098bd9b9cc5934a610ab65da429b525361147faa7b5b922919e9a23143d  -
```

The header rule is identical; only the digest changes. The two well-known empty-object anchors in
SHA-256 form:

```console
$ printf 'blob 0\0' | sha256sum
473a0f4c3be8a93681a267e3b1e9a7dcda1185436fe141f7749120a303721813  -
$ printf 'tree 0\0' | sha256sum
6ef19b41225c5369f1c104d45d8d85efa9b057b53b14b4b9b939dd74decc5321  -
```

A single object store holds one format. A SHA-1 OID has no meaning inside a SHA-256 repository and
vice versa; interoperability between the formats is handled at the boundary (compatibility
mappings), not by mixing digests in one store.

### Hardened SHA-1

Stock Git does not use plain SHA-1. Since practical chosen-prefix collisions were demonstrated
against SHA-1, Git links a collision-detecting variant (the `sha1dc` implementation) that
recognizes the known cryptanalytic attack patterns during hashing and refuses to process inputs
exhibiting them. For an engineering consumer such as Quinjet the operational summary is: within a
repository, OID equality can be treated as byte equality. Every cache key and every network-free
decision below rests on that treatment.

### How Quinjet recognizes a full OID

Because both formats are live in the ecosystem, Quinjet's validation accepts both widths and
nothing else. From `src/git/mod.rs`:

```rust
fn is_full_oid(value: &str) -> bool {
    matches!(value.len(), 40 | 64) && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}
```

The GitHub-side twin, `is_commit_oid` in `src/git/github/mod.rs`, applies the same rule (length
exactly 40 or 64, all ASCII hex digits) to OIDs arriving from API responses before they are ever
placed into a git argv, a cache key, or a refspec. Two properties of this check are deliberate:

**1. Full width only.** Git happily resolves abbreviated OIDs, but abbreviations are ambiguous by
construction: a 12-character prefix that is unique today can collide tomorrow after a fetch.
Nothing ambiguous may serve as a cache key or as an immutability witness, so Quinjet never treats
a short hash as an identity. Short forms appear only in display fields (`%h`, `short_id`), never
in keys or probes.

**2. Syntactic, not semantic.** The check proves the string is shaped like an OID, not that the
object exists or is a commit. Existence and type are a separate question with a separate tool,
`git cat-file -e <oid>^{commit}`, described next. Splitting the two keeps the cheap string check
in guard position (no subprocess for garbage input) and the subprocess probe in decision position.

## Loose object storage

### zlib framing

A freshly written object is stored "loose": one file per object, holding the zlib-compressed
header-plus-content. The demo blob on disk:

```console
$ xxd .git/objects/3b/18e512dba79e4c8300dd08aeb37f8e728b8dad
00000000: 7801 4bca c94f 5230 3462 c848 cdc9 c957  x.K..OR04b.H...W
00000010: 28cf 2fca 49e1 0200 4411 0689            (./.I...D...
```

Twenty-eight bytes on disk for twenty bytes of hashed content; zlib framing costs a few bytes and
wins them back on anything less random than this example. The first byte `0x78` is the zlib CMF
byte (deflate, 32 KiB window); the second byte varies with compression level. Inflating recovers
exactly the hashed bytes, header included:

```console
$ python3 -c "import zlib,sys; \
    sys.stdout.buffer.write(zlib.decompress(open(
        '.git/objects/3b/18e512dba79e4c8300dd08aeb37f8e728b8dad','rb').read()))" | xxd
00000000: 626c 6f62 2031 3200 6865 6c6c 6f20 776f  blob 12.hello wo
00000010: 726c 640a                                rld.
```

Because the header sits at the front of the compressed stream, `git cat-file -t` and `-s` can
answer type and size by inflating only the first few bytes, without ever decompressing a large
body. This is one half of why existence and metadata probes are so much cheaper than content
reads; the other half is that an existence probe does not need to inflate anything at all, only to
find the object.

### Fan-out directories

The loose store shards by the first two hex characters of the OID: object `3b18e512...` lives at
`objects/3b/18e512...` (directory `3b`, file name the remaining 38 or 62 characters). The 256
possible subdirectories keep any single directory's entry count 256 times smaller than the object
count, which mattered enormously on the filesystems Git was born on and still bounds directory
scan costs today. The same two-hex-character fan-out idea reappears as a 256-entry binary table at
the front of every pack index, where it brackets the binary search; see
[packfiles and deltas](./packfiles-and-deltas.md).

### Write once, then never again

Loose object writes are atomic and idempotent:

- Git writes the compressed bytes to a temporary file in the objects directory and `rename`s it to
  its final OID-derived name. A reader can never observe a half-written object; it either finds
  the complete file or no file.
- If the destination already exists, the write can be skipped entirely: by the content-addressing
  rule, an existing file with that name already holds byte-identical content. Loose object files
  are typically written read-only as a guard against accidental modification.
- Nothing ever updates a loose object in place. The only mutations the objects directory sees are
  additions (new objects) and deletions (pruning unreachable objects during `gc`).

Quinjet's own cache mirrors this discipline deliberately: entries are written to a
`.write-<pid>-<counter>.tmp` file and renamed into place, and immutable entries are never
rewritten, only evicted (`src/git/github/mod.rs`, write path around line 2348). When a design
works for Git's object store it tends to be the right design for any content-keyed store, and
[caching](../github/caching.md) documents how far Quinjet takes the parallel.

### Why loose objects alone do not scale

A repository that only ever wrote loose objects would store every version of every file at full
(compressed) size: ten thousand revisions of a 1 MiB file would cost roughly ten thousand
compressed megabytes, and every fetch would transfer objects one at a time. Git therefore
periodically repacks loose objects into packfiles, which delta-compress similar objects against
each other and serve as the wire format for fetch and push. The division of labor is: loose for
recent, mutable-workload writes; packs for bulk history and transfer. The details, including why
fetching a pull request's history is cheap in bytes while inflating its blobs is not, belong to
[packfiles and deltas](./packfiles-and-deltas.md); this page only needs the lookup consequences,
next.

## The object lookup path

When any Git command needs an object, the resolution order is fixed and layered. Understanding it
is the key to understanding both why Quinjet's disposable PR workspace is so cheap and where its
bytes actually come from. For an OID lookup, Git consults:

1. Loose objects in the repository's own `objects/` fan-out directories.
1. The repository's own packfiles, via their `.idx` indexes (fan-out table, then binary search).
1. Every object store listed in `objects/info/alternates`, applying the same loose-then-packs
   search in each.
1. For a partial clone, the promisor machinery: if the object is provably "promised" by a promisor
   remote, fetch it on demand over the network, then retry the local lookup.

Only when all four layers fail does the lookup error out, which for a commit probe such as
`cat-file -e` becomes a non-zero exit status rather than a message.

### Own store first

Steps 1 and 2 are the common case and are purely local. A loose lookup is a single `stat` of the
fan-out path. A pack lookup reads the 256-entry fan-out table at the front of each `.idx` to
bracket a binary search over sorted OIDs; with the multi-pack-index present, one index answers for
many packs. Both are microsecond-scale operations that touch no configuration and no network. The
performance framing that matters for Quinjet: probing for an object's existence is not measurably
more expensive than spawning the `git` process that performs the probe, so process spawn cost, not
lookup cost, is the budget item. That is why Quinjet performs exactly one spawned probe per OID at
decision time and never probes speculatively in loops.

### Alternates: one store lending objects to another

`objects/info/alternates` is a plain text file inside an object store. Each line names another
objects directory (absolute, or relative to the borrowing store's `objects/` directory). Every
lookup that misses locally is retried against each listed store, recursively: an alternate may
itself have alternates, though Git ignores chains nested more than five levels deep, and a
malformed or missing path is skipped rather than treated as an error.

The mechanism is deliberately primitive. There is no negotiation, no locking, no notification
channel between the two stores; the borrower simply reads the lender's files. That primitiveness
is what makes it robust: lending a store costs one line of text and zero coordination, and works
between any two repositories on the same filesystem.

A minimal demonstration, lending the demo repository's objects to a freshly created, completely
empty bare repository:

```console
$ git init --bare --quiet /tmp/quinjet-alt-demo.git
$ cd /tmp/quinjet-alt-demo.git
$ git cat-file -e c7160ef5bcae6b4482af701d182e92364f672fb8; echo "exit=$?"
exit=1
$ printf '/tmp/quinjet-object-demo/.git/objects\n' > objects/info/alternates
$ git cat-file -e c7160ef5bcae6b4482af701d182e92364f672fb8; echo "exit=$?"
exit=0
$ git cat-file -p c7160ef5bcae6b4482af701d182e92364f672fb8 | head -1
tree 68aba62e560c0ebc3396e8ae9335232cd93a3f60
$ find objects -type f
objects/info/alternates
```

One written line, and a repository containing zero objects of its own can read, type-check, and
pretty-print the lender's commit. Nothing was copied; the borrower's own object directories remain
empty. This exact maneuver, executed by `borrow_local_objects` in `src/git/github/mod.rs`, is how
Quinjet's disposable PR workspace starts life already knowing every object the opened repository
knows; the full treatment is in
[Quinjet: borrowing the opened store through alternates](#quinjet-borrowing-the-opened-store-through-alternates).

The classic hazard of alternates runs in the other direction: if the *borrower* holds refs to
objects that exist only in the lender, and the lender runs `git gc`, the lender may prune objects
it considers unreachable, corrupting the borrower. The safe pattern, and the one Quinjet uses, is
to treat the borrowed store as a read-only accelerator: the borrower's correctness must never
depend on the lender retaining anything, only its speed.

### Promisor objects and lazy fetch

A partial clone (created or fetched with `--filter=blob:none` and relatives) knowingly omits
objects. The packs received from such a fetch are marked as promisor packs, and the remote they
came from is recorded as a promisor remote. The contract: any object referenced by a promisor pack
but absent locally is *promised*, and Git may fetch it from the promisor remote on first use
instead of declaring the repository corrupt.

The consequence for the lookup path is that step 4 turns a local miss into a network round trip.
Reading a promised blob triggers a fetch of that blob; commands that touch many missing blobs can
trigger many fetches (modern Git batches the prefetch for several commands, but the transferred
bytes are the same). This is the single most important cost asymmetry in Quinjet's PR pipeline:

- Tree walks, existence probes, and `--name-status` diffs run entirely on commits and trees, which
  a `blob:none` fetch does transfer. They stay local and fast.
- Anything needing file *content*, including `git diff --patch` and, crucially,
  `git diff --numstat` (which must load both blob versions to count lines), forces promised blobs
  to materialize over the network.

Quinjet arranges the layers so that step 4 fires as rarely as possible: alternates (step 3) serve
blobs the opened repository already has, the API supplies numbers Git would otherwise compute from
blob content, and only the blobs of patches actually rendered are ever fetched. The fetch-side
mechanics live in [shallow and partial clone](./shallow-and-partial-clone.md); the API side in
[GitHub API strategy](../github/api-strategy.md).

### What a lookup failure means

A failed lookup is a strong, cheap signal, and Quinjet uses it as one. Because the four layers are
exhaustive, a non-zero exit from `cat-file -e` means "this object is not obtainable here without a
fetch", which is exactly the question "can I diff this pull request without the network" reduced to
one bit. There is no cheaper way to ask it: no ref enumeration, no history walk, no configuration
inspection can answer more directly than the object store itself.

## cat-file: the plumbing window into the store

`git cat-file` is the plumbing command that exposes the object store one object at a time; its
manual page is at [git-cat-file](https://git-scm.com/docs/git-cat-file). The four single-object
modes:

| Flag | Question | Output | Exit status |
|---|---|---|---|
| `-e` | Does this object exist (and match the requested peel)? | none | 0 if yes, non-zero if no |
| `-t` | What type is it? | `blob`, `tree`, `commit`, or `tag` | 0 or error |
| `-s` | How large is its content in bytes? | decimal size | 0 or error |
| `-p` | Pretty-print its content | type-dependent text | 0 or error |

`-e` is the odd one out and the most valuable for automation: it prints nothing at all. The entire
answer is the exit status, which means no output parsing, no locale sensitivity, no buffer
management. It is the cheapest question the object store can answer.

### Peeling with the caret suffix

Revision syntax allows an object name to carry a peel suffix: `<oid>^{commit}` means "dereference
until a commit is reached, or fail". For a commit OID it is the identity; for an annotated tag it
follows the `object` header (repeatedly, for nested tags); for a blob or tree it fails. Combined
with `-e`, the peel turns an existence probe into an existence-and-type probe. The observed
behaviors, from the demo repository:

```console
$ git cat-file -e 'c7160ef5bcae6b4482af701d182e92364f672fb8^{commit}'; echo "exit=$?"
exit=0
$ git cat-file -e '0123456789012345678901234567890123456789^{commit}'; echo "exit=$?"
fatal: Not a valid object name 0123456789012345678901234567890123456789^{commit}
exit=128
$ git cat-file -e '3b18e512dba79e4c8300dd08aeb37f8e728b8dad^{commit}'; echo "exit=$?"
error: 3b18e512dba79e4c8300dd08aeb37f8e728b8dad^{commit}: expected commit type, but the
object dereferences to blob type
fatal: Not a valid object name 3b18e512dba79e4c8300dd08aeb37f8e728b8dad^{commit}
exit=128
```

Three distinct cases, one common property: only the fully successful case exits zero. A missing
object with a bare OID exits 1 silently; with a peel suffix, resolution fails earlier and exits
128 with a message on stderr. A present object of the wrong type also exits 128. A consumer that
checks only "success or not", as Quinjet does, collapses all failure shapes into the same correct
answer: this OID cannot serve as a diff endpoint here.

### Batch modes, and why Quinjet does not use them

`cat-file --batch` and `--batch-check` accept object names on stdin, one per line, and stream
answers back, amortizing process startup across thousands of queries. They are the right tool when
a consumer needs metadata or content for many objects: a server-side hook validating a push, an
indexer walking every blob.

Quinjet deliberately does not run one. Its query pattern is two existence probes per pull-request
open, at a decision point, from short-lived worker threads. A persistent `--batch` child would
save two process spawns per PR at the cost of owning a long-lived subprocess per worker: lifetime
management, stdin/stdout framing, liveness detection after the child dies, and cleanup on every
exit path, in six worker threads (see [concurrency](../rendering/concurrency.md)). The
spawn-per-question model also composes with Quinjet's universal safety net: every child runs under
`run_bounded_command` with capped pipes and kill-on-overflow (documented in
[plumbing and porcelain](./plumbing-and-porcelain.md)), a guarantee that is simple precisely
because each child answers one question and exits.

## Immutability as an engineering property

Content addressing makes immutability a theorem rather than a convention. This section states the
property precisely and derives the license it grants, because every Quinjet section that follows
is an application of one of these derivations.

### The Merkle argument

A commit's OID covers, transitively, every byte of every object reachable from it. The commit body
contains its root tree's OID; the tree contains its entries' OIDs; and so on to the blobs, and
backward through every `parent` header. Any change anywhere in that closure changes a hash, which
changes the containing object's bytes, which changes its hash, cascading to the commit. Formally,
the object graph is a Merkle DAG, the same construction underlying content-addressed systems
generally: possession of a root hash is possession of a tamper-evident commitment to the entire
reachable structure.

Two operational readings:

**1. An OID is a complete description of a snapshot.** To compare two project states it is
sufficient to know two commit OIDs and to hold their reachable objects. No refs, no clocks, no
provenance. This is why Quinjet's PR diff machinery works identically in three very different
homes: the opened repository, a disposable bare workspace, and a cache directory that holds only
derived bytes. The inputs are just two OIDs everywhere.

**2. Equality short-circuits recursion.** If two trees have equal OIDs, their entire subtrees are
identical and need not be walked. `git diff` between two commits descends only into subtrees whose
hashes differ, so the cost of a tree-level diff scales with the size of the *change*, not the size
of the repository. A one-file PR against a million-file monorepo walks a handful of trees.

### A diff is a pure function of two OIDs

Given a fixed Git implementation and fixed diff options, the bytes of
`git diff <oid-A> <oid-B>` are a deterministic function of the two OIDs: the OIDs pin the input
content exactly, and the diff algorithm is deterministic. The same holds for the derived listings
Quinjet indexes with, `--name-status` and `--numstat` over the same range. Which repository runs
the command is irrelevant, provided it holds the objects.

This purity claim has honest fine print, and Quinjet's cache design respects it:

- The function is pure *given the algorithm and options*. A different Git version could break ties
  in the underlying edit-script computation differently, and rename detection (`--find-renames`)
  scores similarity with thresholds that have evolved. A cached patch produced by one Git build
  and read under another is still a *correct* patch for that OID pair, merely not guaranteed to be
  byte-identical to what the newer build would emit. Since Quinjet caches the bytes themselves and
  replays them through its own parser, correctness survives; only byte-for-byte reproducibility is
  version-relative.
- Purity is per-repository-format: the OID pair fixes content only within one hash function's
  namespace.
- `git replace` can graft substitutions over objects (see
  [Edge cases and failure modes](#edge-cases-and-failure-modes)); a repository using it has opted
  out of the plain reading of object identity.

### What immutability licenses

**1. Caches keyed by OIDs never expire.** If the key of a cache entry contains every input OID of
the computation that produced it, the entry cannot go stale: the same key can never map to
different correct bytes. Staleness is a property of *names* (a branch moved), never of *content*
(a commit changed), because content cannot change. The only lifecycle such an entry needs is
eviction for space. Quinjet encodes this distinction as an enum, `CacheLife::Immutable` versus
`CacheLife::Ttl`, examined in
[Quinjet: caches keyed by OID never expire](#quinjet-caches-keyed-by-oid-never-expire).

**2. Local possession makes network use optional.** If both endpoint OIDs of a diff resolve in the
local lookup path, every byte the diff needs is already on disk, and any fetch would transfer
objects that hash to things already present. The network can be skipped not as an optimization
gamble but as a provable no-op. This is the fast path in
[Quinjet: the network-free fast path](#quinjet-the-network-free-fast-path).

**3. Mutation is detected by key change, not by invalidation.** When a PR receives a force-push,
its head OID changes, so every OID-keyed question Quinjet asks becomes a *different question* with
a different cache key, naturally missing the cache and naturally leaving the old entries behind as
garbage for eviction. No invalidation protocol, no cache-coherence traffic, no risk of serving the
old head's patch for the new head. The checks cache key includes `head_oid` for exactly this
reason, and the conversation cache achieves the same effect with the `updated_at` stamp GitHub
moves on any activity; see [caching](../github/caching.md).

**4. Any store may answer.** Because objects are location-independent, bytes may be served from
whichever store is cheapest: the opened repository via alternates, the disposable workspace's own
packs, or the derived-bytes cache on disk. Correctness never depends on which one answered.

The remainder of this page walks these four licenses through the code that exercises them.

## Quinjet: the network-free fast path

The first decision Quinjet makes when preparing a pull-request diff is whether it can avoid the
network entirely. The gate is two object-store probes.

### The gate: has_commit

From `src/git/mod.rs`:

```rust
pub(crate) fn has_commit(&self, oid: &str) -> bool {
    is_full_oid(oid)
        && self
            .run([
                OsString::from("cat-file"),
                OsString::from("-e"),
                OsString::from(format!("{oid}^{{commit}}")),
            ])
            .is_ok_and(|output| output.status.success())
}
```

Every line is doing deliberate work:

- `is_full_oid(oid)` first: the OIDs arrive from GitHub metadata (`baseRefOid`, `headRefOid` from
  `gh pr view`), and although they should always be full hex OIDs, nothing external is trusted
  into an argv unchecked. A malformed value fails the string test and never reaches Git. This also
  guarantees the probe is about an *identity*, never an abbreviation or a ref name that could
  resolve differently tomorrow.
- `cat-file -e` rather than `rev-parse --verify`: `-e` produces no stdout at all, so there is
  nothing to parse and nothing to bound. The exit status is the entire protocol.
- The `^{commit}` peel: the probe asks not just "is this object here" but "is it here and does it
  peel to a commit". A hypothetical blob or tag sneaking in under a commit field fails the probe
  instead of failing later inside `merge-base` or `diff` with a stranger error.
- `.is_ok_and(|output| output.status.success())`: spawn failures, missing-object exit 1, and
  peel-failure exit 128 all collapse to `false`. The three failure shapes observed in the
  demonstration above need no distinguishing here; any of them means "take the fetch path".

Like every repository read, the probe runs with `LC_ALL=C`, `GIT_OPTIONAL_LOCKS=0`, and
`GIT_TERMINAL_PROMPT=0` through `Repository::run` (src/git/mod.rs:1292-1309), so it can never
localize, lock, or prompt; the full environment contract is documented in
[plumbing and porcelain](./plumbing-and-porcelain.md).

### The decision tree

`prepare_pull_request_diff` in `src/git/github/mod.rs` puts the gate in front of everything else:

```rust
let (repository, merge_base, head, api_counts) =
    if self.has_commit(&pull_request.base_oid) && self.has_commit(&pull_request.head_oid) {
        progress(PullRequestProgress::FindingMergeBase);
        (
            PreparedRepository::Opened(self.root().to_path_buf()),
            self.merge_base(&pull_request.base_oid, &pull_request.head_oid)?,
            pull_request.head_oid.clone(),
            None,
        )
    } else {
        progress(PullRequestProgress::PreparingRepository);
        let merge_base_hint = self.merge_base_from_api(pull_request);
        let api_counts = self.pull_request_file_counts_from_api(pull_request);
        let temporary = TemporaryBareRepository::new()?;
        temporary.borrow_local_objects(self);
        let (merge_base, head) = fetch_pull_request(
            &temporary.path,
            pull_request,
            merge_base_hint.as_deref(),
            &mut progress,
        )?;
        (
            PreparedRepository::Temporary(temporary),
            merge_base,
            head,
            api_counts,
        )
    };
```

When both probes pass, the prepared workspace is `PreparedRepository::Opened`: the opened
repository itself, by path, with no fetch, no temporary directory, and no GitHub request of any
kind. The merge base is computed locally with `git merge-base <base_oid> <head_oid>`
(`Repository::merge_base`, src/git/github/mod.rs:852-863), file enumeration runs
`git diff --name-status -z` against the local object store, and per-file counts come from a local
`--numstat` pass, which is harmless here precisely because the blobs are local (the
`api_counts = None` in the fast-path tuple is what later routes `changed_files_in_repository` to
its numstat fallback). Note the merge base is *computed*, not assumed: even on the fast path the
diff is `merge-base...head`, the three-dot semantics GitHub itself displays, as unpacked in
[merge bases and history](./merge-bases-and-history.md).

When either probe fails, the slow branch builds the disposable workspace, and the order of its
first four statements is itself an object-model argument: ask the API for the merge base and the
per-file counts (both cached immutably, both usable before any object exists locally), create the
bare repository, and lend it the opened store's objects *before* fetching, so the fetch negotiation
already sees every locally present object. Sections below take each of those statements in turn.

### Which pull requests hit the fast path

The probes pass whenever both commits are already obtainable in the opened repository's lookup
path, which covers more ground than it might first appear:

- **The reader's own PR.** The head commit was created locally; the base branch is a
  remote-tracking ref that the last `git fetch` updated. Both OIDs resolve instantly.
- **A merged PR.** Merging made the head reachable from the base branch, so an up-to-date clone
  holds every object of every merged PR. Reviewing history through merged PRs is entirely local.
- **A colleague's PR after a routine fetch.** `git fetch --all --prune` (which is exactly what
  Quinjet's Fetch operation runs) updates remote-tracking refs; if the head ref lives in the same
  repository, its objects arrive too.
- **Any PR previously prepared in this repository?** No, and the distinction is instructive: the
  disposable workspace borrows objects *from* the opened repository through alternates, but never
  writes anything *into* it. Objects fetched into a disposable workspace vanish with it on `Drop`.
  The opened repository receives no objects, no refs, no configuration, nothing; that is invariant
  9's closing guarantee ("The opened repository receives no checkout, branch, ref, index, or
  worktree mutation", ARCHITECTURE.md). Re-opening a fork PR re-fetches; what makes the second
  open fast anyway is the immutable byte cache, not the object store.

The corresponding ARCHITECTURE.md text (invariant 9) opens with the fast path stated as policy:
"PR patches first use immutable base/head OIDs already present in the opened repository, which
makes local-branch PR previews network-free."

### The proof is a test, not a claim

The property "the fast path performs no network I/O" is pinned by a test rather than asserted in
prose. `locally_available_pr_objects_avoid_disposable_fetches` (src/git/github/mod.rs:2946-2986)
constructs a pull request whose base and head OIDs both exist in a local test repository, sets the
base repository URL to a deliberately unreachable host (`https://invalid.example.test/...`), and
asserts that preparing the workspace and producing a file diff completes in under 2 seconds. If
any code path touched the URL, the run would hang on an unreachable host and blow the bound. The
test encodes the object-model lesson directly: given both OIDs locally, the URL may be garbage,
because identity is content and content is present.

### Cost accounting

The fast path's total external cost for opening a PR diff is a handful of short-lived local
processes: two `cat-file -e` probes, one `merge-base`, one `--name-status` listing, one
`--numstat` listing, then one `git diff` per file actually rendered (batched for prefetch). Every
one of them is bounded by the caps in `src/git/mod.rs` (8 MiB patch reads, 8 MiB / 16,384-entry
indexes) and runs against local disk. Nothing in the sequence can block on credentials, prompts,
or remote latency, which is what entitles the UI to treat "preview a PR of a local branch" as an
interaction on the same footing as viewing a local commit. The remaining latency story is about
process spawns and parsing, which the pipeline pages
([diff pipeline](../diff/pipeline.md), [prefetch](../github/prefetch.md)) pick up.

## Quinjet: caches keyed by OID never expire

License 1 from the immutability section, made concrete: every cache entry whose key contains the
OIDs that determine its content is marked immutable and is never revalidated, refreshed, or
expired. It can only be evicted for space.

### CacheLife: the two lifecycles

From `src/git/github/mod.rs`:

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

The enum forces every cache write in the codebase to answer one classifying question: is this
entry's identity in its key? If yes, `Immutable`, and `accepts` returns true at any age. If no,
the writer must choose a TTL, and only three classes of genuinely time-varying reads have one:
repository identity (24 hours), pull-request metadata (5 minutes), and the check list (30
seconds). ARCHITECTURE.md invariant 12 states the split as doctrine: "Entries whose key contains
their identity are immutable and never expire ... A new head or a new comment therefore asks a
different question rather than aging an old answer, so a stale read is impossible and only
eviction applies."

There is a subtle interaction with manual refresh worth noting: the cached-read wrapper
`checked_cached_gh` honors a caller's `refresh` flag only for TTL entries. A refresh cannot bypass
an `Immutable` entry, because rereading it could only produce the same bytes; the flag would spend
a request to learn nothing.

### The immutable key inventory of the diff pipeline

Five key families carry the pull-request diff pipeline, all immutable, all containing the OIDs
that pin their bytes:

| Key template | Content | Size cap | Producer |
|---|---|---|---|
| `pr-merge-base-v1\n{repo_url}\n{base}\n{head}` | merge-base OID from the compare API | 2 MiB | `merge_base_from_api` |
| `pr-file-counts-v3\n{repo_url}\n{number}\n{base}\n{head}` | per-file counts TSV from the files endpoint | 8 MiB | `pull_request_file_counts_from_api` |
| `pr-files-v1\n{merge_base}\n{head}` | raw `--name-status -z` bytes | 8 MiB | `changed_files_in_repository` |
| `pr-numstat-v1\n{merge_base}\n{head}` | raw `--numstat -z` bytes | 8 MiB | `numstat_counts` |
| `pr-patch-v1\n{merge_base}\n{head}\n{path}` | one file's unified diff bytes | 1 MiB | `diff_file` / `diff_files` |

Reading the key shapes closely reveals the reasoning:

- The three Git-derived families (`pr-files-v1`, `pr-numstat-v1`, `pr-patch-v1`) contain *only*
  OIDs and, for patches, a path. No repository URL. This is license 4 (any store may answer) taken
  literally: the bytes of a diff between two commits are a function of the commits, and OIDs are
  globally unique identifiers of content, so tagging the entry with which clone happened to run
  the command would only fragment the cache. A workspace prepared for the same PR tomorrow, or the
  opened repository serving the fast path, or a different worktree of the same clone, all read and
  write the same entries.
- The two API-derived families do carry the repository URL (and, for counts, the PR number),
  because their content comes from a remote service's view of a specific repository and endpoint,
  not from a pure function of objects. Their immutability rests on a different argument: the
  *question* named by the key includes both OIDs, and GitHub's answer about a fixed base/head pair
  does not change, because the commits it describes cannot.
- The version prefixes (`-v1`, `-v3`) are the schema-evolution escape hatch. When PR #55 changed
  the counts record from three TSV fields to four (adding `status` so pure renames could be told
  apart from count-less records), the key moved from `pr-file-counts-v2` to `pr-file-counts-v3`,
  orphaning every v2 entry at once. Immutable entries cannot be invalidated, so a format change
  must change the question instead; the stranded v2 entries simply age out through eviction.

The merge-base entry deserves a special note because it caches a *derived relationship* rather
than content: the merge base of two commits is determined by their ancestry, and ancestry is part
of the commits' immutable identity (the `parent` headers are inside the hashed bytes), so the
answer to "what is the merge base of A and B" is as frozen as A and B themselves. One cached
40-or-64-character answer permanently replaces the API call, and with it the entire adaptive
deepening ladder that a cold workspace would otherwise need; the ladder is dissected in
[merge bases and history](./merge-bases-and-history.md).

### The single-file read path

`PreparedPullRequest::diff_file` (src/git/github/mod.rs:402-434) shows the read side end to end:

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

with the key built by a three-line function whose shape is the whole design
(src/git/github/mod.rs:2137-2139):

```rust
fn patch_cache_key(merge_base: &str, head: &str, path: &Path) -> String {
    format!("pr-patch-v1\n{merge_base}\n{head}\n{}", path.display())
}
```

Properties of this path worth spelling out:

**1. A cache hit spawns nothing.** The early return happens before any `git` process exists. On a
warm cache, paging through a pull request's files is pure disk reads and parsing.

**2. Truncated bytes are never cached.** A patch that hit the 8 MiB pipe cap is delivered to the
reader (marked truncated) but not written to the cache. An immutable entry is trusted forever, so
only complete answers may become entries; a truncated one would freeze the truncation for the
lifetime of the OID pair.

**3. The 1 MiB per-entry ceiling is an eviction-fairness rule, not a correctness rule.**
`MAX_CACHED_PATCH_BYTES` exists, per its doc comment, so that "one file cannot crowd out the rest
of a pull request" inside the shared 128 MiB / 2,048-entry budget. An oversized patch is simply
recomputed on demand; the object store makes recomputation always possible, so caching is a pure
optimization that can decline any individual entry.

**4. Batch production feeds the same keys.** `diff_files`, the batched variant behind background
prefetch, writes each complete per-file section it splits out of a combined patch into the same
`pr-patch-v1` keys as a side effect. A file whose patch arrived in a background batch is
afterwards indistinguishable from one fetched singly; a later single-file open is disk-only. The
batching itself (one `git diff` for up to 32 paths, split at `diff --git` boundaries) belongs to
[the diff pipeline](../diff/pipeline.md) and [prefetch](../github/prefetch.md).

### Replaying cached bytes through the live parser

The changed-file index takes the byte-caching idea one step further: what it caches is not a
parsed structure but the raw NUL-separated output of `git diff --name-status -z`, and a cache hit
is replayed through the very same parsing path as live output. From `changed_files_in_repository`
(src/git/github/mod.rs:1981-2089):

```rust
let key = format!("pr-files-v1\n{merge_base}\n{head}");
let cached = cache_read_bounded(&key, CacheLife::Immutable, MAX_PR_PATH_BYTES);
let output = if let Some(data) = cached {
    BoundedOutput {
        status: successful_status(),
        stdout: data,
        stderr: Vec::new(),
        stdout_truncated: false,
    }
} else {
    // run git, cache the complete output, or parse the truncated bytes uncached
```

where `successful_status` fabricates a zero exit status for the synthetic child
(src/git/github/mod.rs:2124-2135):

```rust
/// A status that reports success, for feeding cached bytes back through the
/// same path a real command's output takes.
fn successful_status() -> ExitStatus {
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        ExitStatus::from_raw(0)
    }
    // (windows variant identical in shape)
}
```

The alternative design, caching the parsed `Vec<PullRequestFile>` as serialized structures, was
implicitly rejected, and the reasons compound:

- One parser means one set of parser bugs. Cached and live data cannot drift apart, because there
  is no second deserialization path to drift.
- The cached artifact is exactly what Git produced, so its meaning is defined by Git's documented
  output format, not by an internal struct layout that refactoring could change silently.
- Cache versioning stays in the key (bump `-v1`) rather than in a serialization format.

This "cache the bytes, replay the parse" pattern repeats across the codebase: the numstat entry
holds raw `--numstat -z` bytes fed to `parse_numstat`, the metadata entry holds the TSV record
`gh pr view` printed, and the conversation entries hold raw TSV pages. The cache is a transcript
of past command output, not a database.

### Eviction is the only lifecycle

Because immutable entries never expire, the cache needs no clock-driven maintenance at all; it
needs only bounds. After every write, the store prunes oldest-mtime-first until it is back under
2,048 entries and 128 MiB (src/git/github/mod.rs:2413-2444). The practical texture of that policy
for OID-keyed entries:

- Superseded entries (the patches of a force-pushed-away head, orphaned v2 count entries) are not
  hunted down; they lose the recency contest naturally and fall off the end.
- A busy week of reviewing large PRs cycles the cache; a quiet repository keeps entries for as
  long as they fit. Neither case needs configuration.
- Deleting the cache directory at any moment is always safe, because every entry is a replayable
  transcript of a recomputable command. The store is an accelerator, never a source of truth; if
  `cache_root()` resolves to nothing, every helper silently degrades to the uncached path.

Entry hygiene, atomic writes, private file modes, and the `QUINJET_CACHE_DIR` override are
documented with the rest of the store in [caching](../github/caching.md).

## Quinjet: borrowing the opened store through alternates

When the fast path's probes fail, Quinjet must build a workspace that holds both diff endpoints.
The naive reading of "must fetch the PR" is "must transfer the PR's objects over the network"; the
object model says otherwise. Many of the needed objects usually exist a few directories away, in
the opened repository, and the alternates mechanism from
[the lookup path](#the-object-lookup-path) lets the fresh workspace read them for the cost of
writing one line of text.

### The disposable workspace in brief

`TemporaryBareRepository::new` (src/git/github/mod.rs:1689-1726) runs
`git init --bare --quiet <cache_root>/tmp/pr-<pid>-<counter>.git`: a bare repository (no working
tree will ever be checked out; only object and ref storage is needed), under the cache root's
`tmp` directory with mode 0700, deleted recursively on `Drop`. Stale directories left by crashed
processes are swept when older than 24 hours. The full lifecycle, refspec choreography, and fetch
ladder are the subject of [the PR workspace](../github/pr-workspace.md) and
[shallow and partial clone](./shallow-and-partial-clone.md); what matters here is the store: a
brand-new, completely empty object database, exactly like the bare repository in the alternates
demonstration above.

### borrow_local_objects

Immediately after creating the workspace, and before any fetch, `prepare_pull_request_diff` calls
this method (src/git/github/mod.rs:1732-1745):

```rust
/// Let the disposable workspace read the opened repository's objects. A
/// merged or locally built pull request usually already has most of its
/// blobs on disk under other refs, so lazy blob reads resolve from the
/// local store instead of the network. The opened repository is only read.
fn borrow_local_objects(&self, repository: &Repository) {
    let Ok(common) = repository.git_common_dir() else {
        return;
    };
    let objects = common.join("objects");
    if !objects.is_dir() {
        return;
    }
    let info = self.path.join("objects").join("info");
    drop(fs::write(
        info.join("alternates"),
        format!("{}\n", objects.display()),
    ));
}
```

Fourteen lines, and half of them are graceful surrender. Every step is best-effort: if the common
directory cannot be resolved, if the objects directory does not exist, if the write fails, the
method returns silently and the workspace proceeds without the borrow, merely slower. Nothing
about correctness depends on the alternates link existing; it is a pure accelerator, which is the
only safe role for a mechanism this primitive.

The path written is not `.git/objects` of the session's working directory but the *common*
directory's objects, resolved by `git_common_dir` (src/git/mod.rs:923-939), which runs
`git rev-parse --git-common-dir` and canonicalizes the answer. The distinction matters for linked
worktrees: a worktree's private `.git` file points at a per-worktree administrative directory,
while objects live once in the shared common directory. Resolving through Git rather than path
convention means a PR opened from any worktree of a clone borrows the same, complete object store.
(The same common-directory resolution also feeds the recent-projects identity key and the
filesystem watcher; see [refs, index, and worktrees](./refs-index-and-worktrees.md).)

### What the borrow actually saves

The lent objects act at two distinct moments of the workspace's life:

**1. During fetch negotiation.** A Git fetch is a negotiation: the client tells the server which
objects it wants and which it already has, and the server sends a pack containing, ideally, only
the difference. The workspace's fetches are shallow and filtered, which already minimizes the
transfer, but every object that resolves locally through the alternates is an object the pack
never needs to contain. For the common case that motivated the change in PR #55 (a merged pull
request, or one built from a local branch whose OIDs have since moved so the fast-path probe
missed), most reachable objects are already in the opened store, and the pack shrinks toward the
genuinely novel commits.

**2. During lazy blob reads.** The workspace fetches with `--filter=blob:none`
(src/git/github/mod.rs:1876-1909), so its own packs contain commits and trees but no file
content. Every `git diff --patch` for a selected file must materialize two blob versions, and the
lookup path decides where they come from: workspace packs (no), loose (no), *alternates* (very
often yes), promisor fetch over the network (last resort). Each blob served by the alternates
is one fewer network round trip at the moment a reader is actively waiting to see a file.

The interplay of the layers is worth restating as the lookup-path table it really is:

| Object needed | Own packs (blob:none fetch) | Alternates (opened repo) | Promisor fetch |
|---|---|---|---|
| PR head commits, trees | yes | often | fallback |
| Merge-base commit, trees | yes (depth-1 fetch) | often | fallback |
| Changed-file blobs | never (filtered out) | often | on demand, per rendered file |

The design goal is visible in the rightmost column: the network column should be reached only for
blobs, only for files actually rendered, and only when the local ecosystem genuinely lacks the
bytes.

### Safety analysis

Alternates come with the gc hazard described earlier: a borrower whose refs depend on the lender
is corrupted if the lender prunes. Quinjet's usage sits carefully on the safe side of that line:

- The borrower is disposable and short-lived (dropped when the PR view closes, swept after 24
  hours if orphaned), so there is no long-lived repository whose integrity depends on the loan.
- The workspace's own refs (`refs/quinjet/base`, `refs/quinjet/head`, `refs/quinjet/merge-base`)
  are backed by its own fetched packs; the alternates only shortcut reads. If the opened
  repository pruned an object mid-session, the lookup would fall through to the promisor layer and
  fetch it, degrading latency, not correctness.
- The loan is strictly read-only from the lender's perspective. Nothing in the workspace's
  lifecycle writes into the opened repository's object store, keeps handles that would block its
  gc, or creates refs in it. The doc comment's closing sentence, "The opened repository is only
  read", is the module's contract with invariant 9.

### Why not the built-in sharing flags

Git ships two porcelain-level ways to achieve similar sharing, and both were the wrong shape here:

- `git clone --shared` (or `--reference`) sets up the same alternates file, but through a clone,
  which insists on copying refs, checking connectivity, and optionally checking out a working
  tree. The workspace wants none of that: it starts empty and fetches precisely two or three refs
  under `refs/quinjet/*`.
- `git worktree add` shares objects *and* refs *and* index machinery with the parent repository,
  exactly the entanglement invariant 9 forbids: a worktree is a mutation of the opened
  repository's administrative area, visible to the user's own Git commands.

Writing the alternates file directly takes the one desirable byte-sharing property and leaves
every coupling behind. It is also the only variant that composes with `git init --bare` plus
hand-rolled fetches, where no clone-time flag ever gets a chance to run.

## Quinjet: API counts instead of blob materialization

The changed-file index that drives the Files tree needs, for every file, a status letter and an
additions/deletions pair. The status letters come from `git diff --name-status -z`, which is
tree-only and cheap everywhere. The counts are the trap, and PR #49 is the story of stepping out
of it.

### What numstat actually costs

`git diff --numstat` reports per-file added and deleted line counts. Counting lines is not a
metadata operation: for each changed path, Git must load *both* blob versions, split them into
lines, and run the diff algorithm to classify insertions and deletions. There is no shortcut
through tree entries; a tree knows its blobs' OIDs and modes, not their line structure. So the
cost of numstat over N changed files is the cost of materializing up to 2N blobs plus N diffs.

In an ordinary repository that cost is invisible: the blobs are local, and diffing them is
microseconds each. In a `blob:none` partial workspace, materializing a blob means a promisor fetch
over the network. Modern Git softens the blow by batching the missing-object prefetch for diff
commands into fewer round trips, but batching changes only the trip count, not the freight: every
changed blob pair still crosses the wire. For a wide pull request, a single innocent-looking
`--numstat` would transfer content for every changed file, before the reader had asked to see even
one of them, in a workspace whose entire design premise was "transfer no blob until its file is
rendered". As the PR body put it: "Read per-file additions and deletions from the pulls files
endpoint so a blob-less PR workspace no longer downloads every changed blob just to show counts."

### GitHub already knows the totals

GitHub computes each pull request's per-file counts server-side and serves them from the
[REST pulls files endpoint](https://docs.github.com/en/rest) (`repos/{owner}/{repo}/pulls/{number}/files`),
100 files per page. The server holds full clones; line counting is as cheap for it as for any
local repository, and the numbers describe the same immutable OID pair the workspace will diff.
Reading them is a metadata request, rate-limit-priced like any listing page and transferring a few
dozen bytes per file instead of the file itself.

`pull_request_file_counts_from_api` (src/git/github/mod.rs:1238-1283) is the whole mechanism. Its
head, verbatim:

```rust
/// Per-file additions and deletions from the pull-request files endpoint.
/// In the blob-less disposable workspace a local `--numstat` would download
/// every changed blob just to count lines; GitHub already knows the totals.
fn pull_request_file_counts_from_api(
    &self,
    pull_request: &PullRequest,
) -> Option<HashMap<PathBuf, DiffLineCounts>> {
    let base = pull_request.base_oid.trim();
    let head = pull_request.head_oid.trim();
    let repository = &pull_request.base_repository;
    if !is_commit_oid(base) || !is_commit_oid(head) || repository.name_with_owner.is_empty() {
        return None;
    }
    let key = format!(
        "pr-file-counts-v3\n{}\n{}\n{base}\n{head}",
        repository.url.trim_end_matches('/'),
        pull_request.number
    );
    if let Some(data) = cache_read_bounded(&key, CacheLife::Immutable, MAX_PR_PATH_BYTES) {
        return Some(parse_api_file_counts(&data));
    }
    let endpoint = format!(
        "repos/{}/pulls/{}/files?per_page=100",
        repository.name_with_owner, pull_request.number
    );
    let jq = ".[] | [.filename, (.additions|tostring), (.deletions|tostring), .status] | @tsv";
```

The signature already encodes the fallback contract: the return type is `Option`, and `None`
means "I decline; use local numstat". The guard declines for anything that is not a pair of full
commit OIDs with a known repository, which keeps the immutability argument airtight: the cache key
about to be built must name fixed content, so unfixed inputs never reach it.

The paging loop reads pages `1..=MAX_FILE_COUNT_PAGES` (64 pages, so up to 6,400 files' counts at
`per_page=100`) through the general `api_page` helper, which PR #49 extracted from the
conversation module for exactly this reuse: run `gh api -i`, split the response head from the
body, read continuation from the `Link` header, and trim a pipe-truncated body back to whole
newline-terminated records. A failed or truncated page aborts with `None` (a silently incomplete
count table is worse than an honest fallback), the accumulated TSV is cached only when the loop
saw the last page, and the parsed map is returned even when the 64-page cap stopped early,
because counts for the first 6,400 files are still useful, they are just not worth caching as if
complete.

### Parsing, and the zero-zero problem

The TSV records flow through `parse_api_file_counts` (src/git/github/mod.rs:1918-1943):

```rust
if additions == 0 && deletions == 0 && status != "renamed" {
    continue;
}
```

This skip rule is the residue of two real-world lessons, one per PR:

**1. GitHub reports 0/0 for files it did not count (#49).** Very large files, generated files,
and binary content come back with `additions: 0, deletions: 0`. Storing those as real counts
would render a confident `+0 -0` on files that in truth have unknown, possibly enormous, changes.
The fix in #49 (its commit message calls them "countless records", records *without counts*) was
to drop them, so the UI falls back to its unknown-counts placeholder. Note the interplay with the
`DiffLineCounts.binary` flag: local numstat marks binary files explicitly (a `-` in either
column), while the API path has no equivalent signal, so `binary: false` is set uniformly and
binary files simply land in the dropped 0/0 class.

**2. A pure rename is genuinely 0/0 (#55).** A rename with no content edits has exactly zero
added and deleted lines, and dropping its record made the UI show the loading placeholder forever
for a file whose true answer was known. PR #55 added `.status` to the jq projection, widened the
TSV to four fields, and exempted `status == "renamed"` from the drop, bumping the cache key from
`pr-file-counts-v2` to `pr-file-counts-v3` so entries recorded under the wrong rule could never be
replayed. The test pinning the behavior asserts a `renamed` 0/0 record parses to
`DiffLineCounts { additions: 0, deletions: 0, binary: false }` with the comment "a pure rename
really has zero changed lines".

The files dropped as count-less are not abandoned. Since #55, when any file's patch arrives
(fetched singly or in a background batch), `App::backfill_pull_request_counts` (src/app.rs:5881)
counts the added and removed lines of the parsed document and fills in the header of any file
whose counts were still `None`, never overwriting counts already known. The API provides the fast
approximate cover; the patches themselves provide the exact stragglers. The rendering side of
that handshake, skeleton placeholders (`+·· -··`) included, is covered in
[progressive loading](../rendering/progressive-loading.md).

### The fallback ladder in one line

`changed_files_in_repository` receives the optional API map and resolves the source with a single
expression (src/git/github/mod.rs:1996):

```rust
let counts = api_counts.unwrap_or_else(|| numstat_counts(repository, merge_base, head));
```

Reading the two branches against the two workspace kinds shows the economics:

- **Opened-repository fast path:** `api_counts` is `None` by construction (the fast-path tuple in
  `prepare_pull_request_diff` passes `None`), so counts come from local numstat. Correct choice:
  the blobs are local, numstat is nearly free, and skipping the API keeps the fast path at zero
  network requests. It also self-repairs when GitHub's own counts would have been the 0/0
  placeholders, since local numstat always counts real content.
- **Disposable workspace:** `api_counts` is `Some` whenever the endpoint answered, so numstat, the
  blob-materializing read, is never run against the blob-less store. Only if the API declined
  (malformed OIDs, network failure, a truncated page) does the workspace fall back to local
  numstat, accepting the blob transfer as the price of showing counts at all. The fallback is
  itself cached under `pr-numstat-v1\n{merge_base}\n{head}`, so the price is paid at most once per
  OID pair.

`numstat_counts` on the GitHub side (src/git/github/mod.rs:2094-2120) mirrors the index read
exactly: same revision range, same `--find-renames`, `-z` NUL termination, the 8 MiB
`MAX_PR_PATH_BYTES` cap, cache write only for complete successful output. Its doc comment states
the display rationale shared by both sources: "One extra `--numstat` pass over the same range lets
every file header render its real `+n -n` immediately, so the list never fills in unevenly as
patches load." The same headers-before-patches discipline governs local diffs too (invariants 8
and 8a), where the numstat argv is literally derived from the name-status argv by swapping one
token; see [the diff pipeline](../diff/pipeline.md).

### Counts are also the scheduler's ruler

The counts earn their keep twice. Beyond header rendering, they are the sizing input for
background prefetch. `estimated_patch_bytes` (src/app.rs:7052-7060):

```rust
fn estimated_patch_bytes(counts: Option<DiffLineCounts>) -> usize {
    counts.map_or(PULL_REQUEST_PATCH_FALLBACK_ESTIMATE, |counts| {
        counts
            .additions
            .saturating_add(counts.deletions)
            .saturating_mul(PULL_REQUEST_PATCH_LINE_ESTIMATE)
            .saturating_add(4_096)
    })
}
```

Every file's expected patch size is `(additions + deletions) * 80` bytes plus a 4,096-byte
per-file overhead, or a conservative 512 KiB (`PULL_REQUEST_PATCH_FALLBACK_ESTIMATE`) when counts
are unknown. Batches are filled until 32 files (`PULL_REQUEST_PREFETCH_BATCH`) or until the next
file's estimate would push the batch past the 6 MiB estimated budget
(`PULL_REQUEST_PREFETCH_BYTE_BUDGET`), a margin chosen to keep the real combined patch comfortably
under the hard 8 MiB pipe cap that would otherwise truncate it. Background fill visits at most
4,096 files (`MAX_PREFETCHED_PULL_REQUEST_FILES`).

The *order* in which candidates meet that budget has its own history, and it is a useful record of
how a scheduling idea matures:

- **PR #50 (superseded): smallest-first size tiers.** Once a pull request crossed 100,000 changed
  lines or 1,000 files, candidates were sorted ascending by `estimated_patch_bytes` before batch
  filling, so the byte budget bought the greatest possible *number* of completed files per batch
  and "most of the tree opens instantly". The counts from #49 were what made the sort possible at
  all; without them every file would have carried the same 512 KiB fallback estimate and the sort
  would have been a no-op.
- **PR #55 (current): viewport-anchored wrap-around.** The size sort and its two threshold
  constants were deleted. `request_pull_request_prefetch` now starts at
  `prefetch_anchor_index()`, the first file visible in the Files tree at the current scroll
  offset, and walks the index in order from there, wrapping around to cover everything before the
  anchor. Smallest-first optimized a global statistic (files complete) while the reader stared at
  a specific screenful that might contain none of them; anchoring optimizes what is actually in
  front of the reader, and the wrap-around still completes the whole index. The counts did not
  lose their scheduling job, they kept the batch-sizing half and handed the ordering half to the
  viewport.

Both halves of that evolution, along with the mailbox slot that keeps batches from ever
displacing a waiting reader's preview, are detailed in [prefetch](../github/prefetch.md) and
[progressive loading](../rendering/progressive-loading.md).

### The invariant, updated by the change

PR #49 amended ARCHITECTURE.md invariant 9 with the clause that now reads: "per-file line counts
come from the pull-request files endpoint instead of a blob-materializing local numstat". The
sentence is a compressed form of this entire section: the object model prices blob content as the
expensive resource in a partial workspace, metadata endpoints price the same numbers as cheap, and
the pipeline was rearranged to buy them in the cheap market.

## A full trace: two openings of one pull request

The sections above each isolate one mechanism. This one runs them in order, twice, as the code
does: once for a pull request whose commits are local, once for a fork PR seen for the first time.
Both traces start at the same place, `prepare_pull_request_diff`, immediately after `gh pr view`
metadata (cached for 5 minutes under `pull-request-v3\n{repo_url}\n{number}`) has supplied
`base_oid` and `head_oid`.

### Trace A: the reader's own branch

1. `is_full_oid(base_oid)` and `is_full_oid(head_oid)` pass in nanoseconds; two
   `git cat-file -e <oid>^{commit}` probes exit 0 against the opened repository. Decision made:
   `PreparedRepository::Opened`, zero network.
1. `git merge-base <base_oid> <head_oid>` prints the merge-base OID. The diff range is now a pair
   of fixed OIDs, `merge_base` and `head_oid`, and every cache key below is derived from them.
1. `changed_files_in_repository` checks `pr-files-v1\n{merge_base}\n{head}`. First open of this
   pair: miss. `git diff --name-status -z --find-renames <merge_base> <head> --` runs against
   local trees only (no blob is opened for a name-status walk), and its complete output is cached
   immutably.
1. `api_counts` is `None` on this path, so `numstat_counts` checks
   `pr-numstat-v1\n{merge_base}\n{head}`: miss, run `git diff --numstat -z`, blobs are local so
   this is cheap, cache the bytes. Every file header now knows its real `+n -n`.
1. The index (files, statuses, counts, truncation flag) returns to the app; collapsed headers
   render immediately.
1. The reader selects a file: `diff_file` checks `pr-patch-v1\n{merge_base}\n{head}\n{path}`,
   misses, runs one path-scoped `git diff --patch`, caches the complete patch (up to 1 MiB), and
   parses it for display.
1. Background prefetch fills batches of up to 32 files / 6 MiB estimated from the viewport anchor
   outward, one `git diff` per batch, writing every complete section into the same per-file patch
   keys.
1. The reader closes and reopens the PR an hour later: steps 3, 4, and 6 are now all cache hits.
   The only Git processes spawned are the two probes and the merge-base call, and the only
   network traffic of the entire session remains the metadata read.

### Trace B: a fork PR, cold

1. The same two probes exit non-zero (the head commit exists only in the fork). Decision made:
   disposable workspace.
1. Before any object moves, two API questions are asked, both keyed by the OID pair and cached
   immutably: the compare API for the merge base (`pr-merge-base-v1\n...`), and the files
   endpoint for per-file counts (`pr-file-counts-v3\n...`), up to 64 pages of 100 records.
1. `git init --bare --quiet .../tmp/pr-<pid>-<n>.git` creates an empty store;
   `borrow_local_objects` writes its `objects/info/alternates` line pointing at the opened
   repository's common `objects` directory.
1. The PR head is fetched: `git fetch --quiet --force --no-tags --filter=blob:none --depth=64`
   of `+refs/pull/<n>/head:refs/quinjet/head`. Commits and trees arrive; blobs are promised, not
   sent; anything already present via the alternates is not sent either.
1. With the merge-base hint from step 2, one more fetch at `--depth=1` pins
   `+<hint>:refs/quinjet/merge-base`: a single commit and its trees. If the fetched head still
   matches the advertised `head_oid`, the base branch's history is never fetched at all. (Without
   the hint, the adaptive deepening ladder takes over; see
   [merge bases and history](./merge-bases-and-history.md).)
1. `changed_files_in_repository` runs in the workspace. The name-status walk touches only trees,
   so `blob:none` costs nothing here, and the output is cached under the same
   `pr-files-v1\n{merge_base}\n{head}` key Trace A would use, because the bytes are a pure
   function of the OID pair.
1. Counts come from the step-2 API map; local numstat is never run, and no blob is materialized
   for the index. Headers render with real counts, except for files GitHub declined to count,
   which show the skeleton placeholder until their patches arrive and backfill them.
1. Selecting a file runs the path-scoped `git diff --patch` in the workspace. Now, and only now,
   two blob versions for that file must exist: the lookup path tries the workspace packs, the
   alternates, and finally a promisor fetch for whatever is genuinely missing. The finished patch
   is cached under the shared per-file key.
1. Prefetch batches proceed exactly as in Trace A; each batch's lazy blob traffic is bounded by
   the batch's contents, and each completed file's patch becomes a permanent immutable entry.
1. Closing the PR view drops `TemporaryBareRepository`, deleting the workspace and its packs.
   What survives is precisely the derived-bytes cache: files listing, numstat or API counts,
   merge base, and every complete patch.
1. Reopening the same PR later rebuilds a workspace (the fetches rerun, cheap and shallow), but
   every listing and patch read is served from the immutable cache; the reader sees the diff at
   disk speed while the workspace warms behind it.

The two traces answer to the same reading: the object model fixes what is expensive (blob
transfer), what is free (identity checks against local stores), and what is permanent (any bytes
derived from an OID pair). The pipeline's job is to route every question to the cheapest layer
that can answer it.

## Design alternatives and why they lost

Each subsection names a road not taken and the object-model reasoning that closed it. These are
reconstructions of live trade-offs, not straw men; several are the standard choice in other tools.

### Linking a Git library instead of spawning processes

Quinjet never links libgit2, gitoxide, or any other in-process Git implementation. Everything is
a spawned `git` subprocess, and the object store is one of the strongest reasons this holds up:

- **Authority.** The store's format is defined by Git's implementation in practice: loose zlib
  framing, pack index quirks, alternates resolution order, promisor semantics, replace refs. An
  in-process reimplementation must track all of it forever; `git cat-file` and `git diff` *are*
  the reference implementation, version-matched to whatever repository format the user's own Git
  writes.
- **Isolation.** A library parsing a corrupt or adversarial object corrupts or crashes the TUI
  process. A subprocess doing the same dies alone, and `run_bounded_command` turns even a
  runaway output stream into a killed child and a truncation flag. The process boundary is a
  memory-safety and resource boundary the object model's "verify on read" philosophy pairs well
  with.
- **Cost profile.** The apparent downside is process spawn latency, but the pipeline's design
  systematically amortizes it: one process per *question*, and batching where questions multiply
  (32 files per prefetch diff). The store-side operations Quinjet needs (existence probes,
  tree-walk listings, path-scoped diffs) are each singular enough that the spawn is the smallest
  term.

### Probing existence with rev-parse

`git rev-parse --verify --quiet <oid>^{commit}` answers nearly the same question as
`cat-file -e`. It lost the probe job on output: `rev-parse --verify` prints the resolved OID on
stdout, which the caller must then read, bound, and discard, while `-e` prints nothing by
contract. For a pure boolean asked from a worker thread, the no-output tool is strictly simpler,
and the difference in intent is self-documenting: `rev-parse` exists to *resolve* names,
`cat-file -e` exists to *test* objects. Quinjet does use `rev-parse --verify` where the resolved
value is actually wanted, for example pinning `preferred_fetched_commit` inside the workspace.

### A plain shallow fetch without the blob filter

A `--depth=64` fetch without `--filter=blob:none` would make the workspace self-sufficient: every
blob of every fetched commit present locally, numstat and patches all local, no promisor
machinery. It loses on the numbers that matter: the transfer would include the full content of
every file in the head commit's tree (a checkout's worth of blobs for the tip, plus deltas across
the shallow window), when the reader may open three files of a 2,000-file PR. `blob:none` defers
exactly the deferrable bytes, alternates recover the locally present ones for free, and the
lazy-fetch cost lands only on rendered files. The fallback still exists in one place: servers
without `uploadpack.allowFilter` reject filtered fetches, and `fetch_ref` retries the identical
command without the filter, accepting the fatter transfer where the thin one is impossible.

### Copying or hardlinking objects into the workspace

Instead of alternates, the workspace could copy (or hardlink) the opened repository's objects
directory. Copying is strictly worse on every axis: proportional to store size in time and disk
(a large monorepo's object store is many gigabytes), racy against concurrent gc in the source,
and wasteful for a workspace that will read a tiny reachable slice. Hardlinking fixes the disk
cost but not the enumeration cost, breaks across filesystems (the cache root and the repository
frequently live on different mounts), and still snapshots a directory listing that gc can
invalidate. The alternates file costs one line, adapts to the lender's current state on every
lookup, and shares the pack indexes too, which a naive file copy of loose objects would miss.

### Namespacing the byte caches by repository

Adding the repository URL to `pr-files-v1`, `pr-numstat-v1`, and `pr-patch-v1` keys would feel
safer: no two projects could ever share entries. The object model says the safety is illusory and
the cost real. Illusory, because OIDs are content hashes: a colliding key requires the same
merge-base and head OIDs, which means the same commits, and the diff of the same commits is the
same regardless of which URL they were fetched from (forks are the everyday case: base and fork
are different URLs holding identical objects). Real, because the fast path and the workspace path
would stop sharing entries with each other and with sibling worktrees, multiplying cold reads.
The API-derived keys keep the URL precisely because their content is a service's answer, not a
pure function of objects; the split in key shapes is the design.

### Time-to-live on everything

A uniform TTL cache (say, five minutes on all entries) is the default architecture in most API
clients, and it would have been dramatically simpler than the `CacheLife` split. It loses twice:

- **It expires what cannot change.** Re-deriving a patch for a fixed OID pair after five minutes
  spends a subprocess (or, in the workspace case, possibly a network blob fetch) to reproduce
  bytes the cache already held. Under a TTL regime the whole "reopen a huge PR instantly"
  property evaporates on a clock.
- **It trusts what can.** Whatever TTL is chosen is a window in which a moved branch, a new
  comment, or a fresh force-push is silently misreported. The immutable regime has no such
  window: mutable facts are either genuinely time-varying with a deliberately short clock
  (check list, 30 seconds) or converted into immutable questions by putting the changing part
  into the key (`head_oid` in the checks key, `updated_at` in the conversation key).

The deep version of this argument, including the stale-on-error path that serves expired TTL
entries when the network is down, lives in [caching](../github/caching.md).

### Keeping local numstat and batching the blob fetch

PR #49's problem admitted another fix: keep `--numstat` in the workspace and lean on Git's
partial-clone prefetch batching so the promised blobs arrive in one round trip instead of
thousands. This is a genuine improvement over naive lazy fetching, and it still lost, because it
optimizes the trip count while the freight is the problem: every changed blob pair crosses the
wire before a single header can show its counts, on a code path whose entire purpose is to show
headers *before* content. The API read moves the counting to the machine where the blobs already
live. The residual price, GitHub's refusal to count some files, was paid with the 0/0 drop rule
plus patch-time backfill, which bounds the inaccuracy window to exactly the files GitHub could
not count, instead of taxing every PR open with a full blob transfer.

## Edge cases and failure modes

The object model is clean; the world around it is not. This section collects the boundary
conditions the Quinjet code visibly defends against, and the ones it consciously accepts.

### Abbreviated object names

Git resolves unique OID prefixes everywhere humans type, but prefix uniqueness is a property of
one store at one moment: a fetch can land a new object that makes yesterday's unique prefix
ambiguous. Quinjet therefore bans abbreviations from every load-bearing position by construction:
`is_full_oid` gates the probe, `is_commit_oid` gates API-supplied OIDs before they reach argv,
cache keys, or refspecs, and short IDs exist only as display strings. A related discipline
applies to revision arguments generally: anything that is not a full OID must be a vetted ref
(`resolve_revision` accepts only `refs/heads/`, `refs/remotes/`, `refs/tags/`, or `HEAD`), which
is a safety story told in [plumbing and porcelain](./plumbing-and-porcelain.md).

### Replace refs can shadow identity

`git replace` lets a repository declare "when asked for object X, serve object Y", and most
commands honor the substitution unless `GIT_NO_REPLACE_OBJECTS` is set. This is a deliberate,
local opt-out of content addressing: in a repository using replace refs, an OID no longer
strictly names its bytes. Quinjet does not disable replacement; a user who has grafted their
history has asked every Git tool, Quinjet included, to see the grafted view, and the diffs
Quinjet renders will match the diffs `git` prints in that repository. The cache keys still hold:
entries derive from whatever bytes Git served for the OID pair in that configuration, and a
repository that later drops its replace refs would at worst re-derive on the next miss. The case
is noted here for honesty rather than defense: it is the one standard mechanism by which the
"OID equals bytes" premise can be locally bent, and it bends identically for every consumer of
the repository.

### Shallow boundaries: present commits, absent parents

In a shallow clone, a commit at the boundary exists and passes `cat-file -e`, but its parents do
not exist; the shallow file marks where history was cut. Two Quinjet behaviors account for this:

- The fast-path probes test only the two endpoint commits, but the fast path then runs
  `git merge-base`, which walks ancestry. In a shallow opened repository the walk can fail or
  produce no answer even though both probes passed; `Repository::merge_base` surfaces this as a
  hard error ("Git did not return a pull-request merge base") rather than guessing.
- Inside the workspace, shallowness is the *normal* state, and the missing-ancestry failure mode
  is handled structurally: `git merge-base` returning non-zero is read as "deepen further" by
  `try_merge_base`, driving the ladder through depths 64, 256, 1,024, 4,096, 16,384 before
  refusing an unbounded fetch. The API hint exists to skip the ladder entirely.

### Corruption is detected at read time

A loose object whose bytes do not hash to its file name, or whose zlib stream is damaged, fails
the moment any command inflates it; Git's read path verifies rather than trusts. For Quinjet this
surfaces as a failed subprocess with a fatal message on stderr, which the bounded runner captures
(128 KiB of stderr kept) and converts into an error string for the affected read; other panes and
lanes continue. The derived-bytes cache adds its own shallow integrity layer in the same spirit:
entries must begin with the `quinjet-gh-cache-v1\n` magic, oversized files are deleted on sight,
and a torn write is impossible by the tmp-and-rename protocol.

### The lender prunes mid-session

The alternates loan means a `git gc` in the opened repository can delete an object the disposable
workspace was about to read (safely from the lender's own perspective: the object was unreachable
from *its* refs). The workspace's exposure is one failed lazy read, and the layered lookup turns
it into latency rather than failure: an object missing from workspace packs and alternates but
referenced by a promisor pack is fetched from the remote. The genuinely unrecoverable case, an
object absent everywhere including the remote, means the remote itself rewrote history mid-open,
and it surfaces as a failed diff for the affected file, scoped by the per-file read model to that
file's document.

### The opened repository garbage-collects during a fast-path session

The same race exists without alternates: the fast path pins `PreparedRepository::Opened` after
two probes, and a concurrent `git gc` in that repository could in principle prune an unreachable
PR head between the probe and a later path-scoped diff. Two facts bound the exposure. First, the
common fast-path cases (own branch, merged PR, fetched remote-tracking ref) involve commits
reachable from real refs, which gc does not prune. Second, every per-file read is an independent
subprocess: a pruned object fails that one read with a Git error, the UI shows the error for that
file, and reselecting the PR re-runs preparation, whose probes now fail and route to the
workspace path. No held file descriptors, no long-lived state, nothing to corrupt.

### No cache directory at all

`cache_root()` can resolve to nothing (no `QUINJET_CACHE_DIR`, no platform cache location). Every
cache helper is `Option`-shaped and best-effort, so the entire immutable-cache layer silently
disappears: reads miss, writes are dropped, and the pipeline behaves like a first run forever.
The workspace parent directory falls back to `env::temp_dir()`. Nothing in correctness notices;
the design treats the cache as a transcript that may or may not be retained, which is the only
sound posture for a store the object model makes fully recomputable.

### Binary files and the API's silent zeros

Local numstat marks binary files explicitly: a `-` in either column parses to
`DiffLineCounts { binary: true }`, and the UI renders a binary marker instead of counts. The
files endpoint has no such marker in the fields Quinjet projects, and reports zeros for binary
content, so on the API path binary files fall into the dropped 0/0 class and render the unknown
placeholder instead. The asymmetry is accepted: the placeholder is honest ("counts unknown"),
patch arrival corrects what can be corrected, and the alternative, a per-file content probe to
classify binariness, would reintroduce exactly the blob materialization the API path exists to
avoid.

### SHA-256 repositories

Every OID-shaped check in the codebase accepts 64-hex names alongside 40-hex ones, so a SHA-256
repository flows through probes, keys, and refspecs unchanged; the cache keys are
width-agnostic strings and the two namespaces cannot collide (a 40-character key and a
64-character key are different keys). The practical limits are ecosystem-side, not
Quinjet-side: hosting and interop for SHA-256 repositories remain the constraining factor, and a
mixed-format fetch is not a thing the object model defines. Quinjet inherits exactly Git's
position here by never parsing or constructing objects itself.

### Truncated listings and the last record boundary

The 8 MiB caps on listings mean a monstrous index can arrive cut mid-record. Every NUL-separated
consumer repairs the cut the same way: discard bytes after the last NUL, parse whole records,
and set the truncation flag (`changed_files_in_repository` at src/git/github/mod.rs:2019-2030,
with `total_files` then taking `max(changed_files_from_api, parsed)` so the header count stays
honest). Line-oriented consumers pop back to the last newline. The rule is universal: a cap may
cost completeness, never parseability, and truncated bytes are never cached, so a cap hit today
cannot freeze a partial answer into an immutable entry tomorrow.

### Unique workspace names under concurrency

Multiple Quinjet processes (a TUI session and a `quinjet pr diff` subcommand, say) can prepare
workspaces concurrently under the same cache root. Directory names embed both the process ID and
a per-process atomic counter (`pr-<pid>-<counter>.git`), collisions are re-tried up to 16 times,
and each process deletes only its own workspace on drop. The 24-hour sweep for orphans scans at
most 256 entries and matches only the `pr-*.git` shape, so it can never reap a foreign directory
that wandered into the tmp root.

## Where to go next

This page covered the store itself. Its immediate neighbors deepen each mechanism that was only
sketched here:

- [Packfiles and deltas](./packfiles-and-deltas.md): how objects travel and compress, why fetches
  are cheap in bytes while blob inflation is not, and what a promisor pack physically is.
- [Shallow and partial clone](./shallow-and-partial-clone.md): protocol v2, want/have negotiation,
  filters, deepening, and the workspace's full fetch ladder.
- [Merge bases and history](./merge-bases-and-history.md): the commit DAG, why a PR diff is
  three-dot, and the compare-API resolution that replaces local ancestry walks.
- [Refs, index, and worktrees](./refs-index-and-worktrees.md): the mutable naming layer above the
  immutable store, and the lock-avoidance rules for reading it live.
- [Plumbing and porcelain](./plumbing-and-porcelain.md): the machine-interface contract every
  command in this page relied on, from `-z` termination to capped pipes.
- [The PR workspace](../github/pr-workspace.md) and [prefetch](../github/prefetch.md): the
  consumer side of everything the store makes possible, batch by batch.
- [Caching](../github/caching.md): the disk store that turns object immutability into permanent
  answers, entry format and eviction included.
- [Techniques](../techniques.md): the catalog view, where immutable-key caching, API-derived
  metadata, and capped pipes appear alongside their siblings from other subsystems.

The group hub is [git-internals](./README.md); the section-wide catalog is
[optimization techniques](../techniques.md).

## Optimization review matrix

Use this matrix during performance reviews. Each row combines a cost lens, repository context, and observable signal without claiming that every combination needs a standalone benchmark.

| ID | Review condition | Evidence to capture |
| ---: | --- | --- |
| 1 | Check latency for The Object Model: Git's Content-Addressable Store from Bytes Up in a small local repository | Record time to first useful rows |
| 2 | Check latency for The Object Model: Git's Content-Addressable Store from Bytes Up in a small local repository | Record steady frame cost |
| 3 | Check latency for The Object Model: Git's Content-Addressable Store from Bytes Up in a small local repository | Record bytes accepted from child output |
| 4 | Check latency for The Object Model: Git's Content-Addressable Store from Bytes Up in a small local repository | Record Git and gh process count |
| 5 | Check latency for The Object Model: Git's Content-Addressable Store from Bytes Up in a small local repository | Record maximum retained document bytes |
| 6 | Check latency for The Object Model: Git's Content-Addressable Store from Bytes Up in a small local repository | Record cache disposition and complete key |
| 7 | Check latency for The Object Model: Git's Content-Addressable Store from Bytes Up in a small local repository | Record stale reply rejection |
| 8 | Check latency for The Object Model: Git's Content-Addressable Store from Bytes Up in a small local repository | Record visible state after failure |
| 9 | Check latency for The Object Model: Git's Content-Addressable Store from Bytes Up in a monorepo with many changed paths | Record time to first useful rows |
| 10 | Check latency for The Object Model: Git's Content-Addressable Store from Bytes Up in a monorepo with many changed paths | Record steady frame cost |
| 11 | Check latency for The Object Model: Git's Content-Addressable Store from Bytes Up in a monorepo with many changed paths | Record bytes accepted from child output |
| 12 | Check latency for The Object Model: Git's Content-Addressable Store from Bytes Up in a monorepo with many changed paths | Record Git and gh process count |
| 13 | Check latency for The Object Model: Git's Content-Addressable Store from Bytes Up in a monorepo with many changed paths | Record maximum retained document bytes |
| 14 | Check latency for The Object Model: Git's Content-Addressable Store from Bytes Up in a monorepo with many changed paths | Record cache disposition and complete key |
| 15 | Check latency for The Object Model: Git's Content-Addressable Store from Bytes Up in a monorepo with many changed paths | Record stale reply rejection |
| 16 | Check latency for The Object Model: Git's Content-Addressable Store from Bytes Up in a monorepo with many changed paths | Record visible state after failure |
| 17 | Check latency for The Object Model: Git's Content-Addressable Store from Bytes Up in a pull request containing generated files | Record time to first useful rows |
| 18 | Check latency for The Object Model: Git's Content-Addressable Store from Bytes Up in a pull request containing generated files | Record steady frame cost |
| 19 | Check latency for The Object Model: Git's Content-Addressable Store from Bytes Up in a pull request containing generated files | Record bytes accepted from child output |
| 20 | Check latency for The Object Model: Git's Content-Addressable Store from Bytes Up in a pull request containing generated files | Record Git and gh process count |
| 21 | Check latency for The Object Model: Git's Content-Addressable Store from Bytes Up in a pull request containing generated files | Record maximum retained document bytes |
| 22 | Check latency for The Object Model: Git's Content-Addressable Store from Bytes Up in a pull request containing generated files | Record cache disposition and complete key |
| 23 | Check latency for The Object Model: Git's Content-Addressable Store from Bytes Up in a pull request containing generated files | Record stale reply rejection |
| 24 | Check latency for The Object Model: Git's Content-Addressable Store from Bytes Up in a pull request containing generated files | Record visible state after failure |
| 25 | Check latency for The Object Model: Git's Content-Addressable Store from Bytes Up in a deeply diverged branch | Record time to first useful rows |
| 26 | Check latency for The Object Model: Git's Content-Addressable Store from Bytes Up in a deeply diverged branch | Record steady frame cost |
| 27 | Check latency for The Object Model: Git's Content-Addressable Store from Bytes Up in a deeply diverged branch | Record bytes accepted from child output |
| 28 | Check latency for The Object Model: Git's Content-Addressable Store from Bytes Up in a deeply diverged branch | Record Git and gh process count |
| 29 | Check latency for The Object Model: Git's Content-Addressable Store from Bytes Up in a deeply diverged branch | Record maximum retained document bytes |
| 30 | Check latency for The Object Model: Git's Content-Addressable Store from Bytes Up in a deeply diverged branch | Record cache disposition and complete key |
| 31 | Check latency for The Object Model: Git's Content-Addressable Store from Bytes Up in a deeply diverged branch | Record stale reply rejection |
| 32 | Check latency for The Object Model: Git's Content-Addressable Store from Bytes Up in a deeply diverged branch | Record visible state after failure |
| 33 | Check latency for The Object Model: Git's Content-Addressable Store from Bytes Up in an unavailable network | Record time to first useful rows |
| 34 | Check latency for The Object Model: Git's Content-Addressable Store from Bytes Up in an unavailable network | Record steady frame cost |
| 35 | Check latency for The Object Model: Git's Content-Addressable Store from Bytes Up in an unavailable network | Record bytes accepted from child output |
| 36 | Check latency for The Object Model: Git's Content-Addressable Store from Bytes Up in an unavailable network | Record Git and gh process count |
| 37 | Check latency for The Object Model: Git's Content-Addressable Store from Bytes Up in an unavailable network | Record maximum retained document bytes |
| 38 | Check latency for The Object Model: Git's Content-Addressable Store from Bytes Up in an unavailable network | Record cache disposition and complete key |
| 39 | Check latency for The Object Model: Git's Content-Addressable Store from Bytes Up in an unavailable network | Record stale reply rejection |
| 40 | Check latency for The Object Model: Git's Content-Addressable Store from Bytes Up in an unavailable network | Record visible state after failure |
| 41 | Check latency for The Object Model: Git's Content-Addressable Store from Bytes Up in rapid keyboard navigation | Record time to first useful rows |
| 42 | Check latency for The Object Model: Git's Content-Addressable Store from Bytes Up in rapid keyboard navigation | Record steady frame cost |
| 43 | Check latency for The Object Model: Git's Content-Addressable Store from Bytes Up in rapid keyboard navigation | Record bytes accepted from child output |
| 44 | Check latency for The Object Model: Git's Content-Addressable Store from Bytes Up in rapid keyboard navigation | Record Git and gh process count |
| 45 | Check latency for The Object Model: Git's Content-Addressable Store from Bytes Up in rapid keyboard navigation | Record maximum retained document bytes |
| 46 | Check latency for The Object Model: Git's Content-Addressable Store from Bytes Up in rapid keyboard navigation | Record cache disposition and complete key |
| 47 | Check latency for The Object Model: Git's Content-Addressable Store from Bytes Up in rapid keyboard navigation | Record stale reply rejection |
| 48 | Check latency for The Object Model: Git's Content-Addressable Store from Bytes Up in rapid keyboard navigation | Record visible state after failure |
| 49 | Check latency for The Object Model: Git's Content-Addressable Store from Bytes Up in a linked worktree | Record time to first useful rows |
| 50 | Check latency for The Object Model: Git's Content-Addressable Store from Bytes Up in a linked worktree | Record steady frame cost |
| 51 | Check latency for The Object Model: Git's Content-Addressable Store from Bytes Up in a linked worktree | Record bytes accepted from child output |
| 52 | Check latency for The Object Model: Git's Content-Addressable Store from Bytes Up in a linked worktree | Record Git and gh process count |
| 53 | Check latency for The Object Model: Git's Content-Addressable Store from Bytes Up in a linked worktree | Record maximum retained document bytes |
| 54 | Check latency for The Object Model: Git's Content-Addressable Store from Bytes Up in a linked worktree | Record cache disposition and complete key |
| 55 | Check latency for The Object Model: Git's Content-Addressable Store from Bytes Up in a linked worktree | Record stale reply rejection |
| 56 | Check latency for The Object Model: Git's Content-Addressable Store from Bytes Up in a linked worktree | Record visible state after failure |
| 57 | Check latency for The Object Model: Git's Content-Addressable Store from Bytes Up in cold and warm cache states | Record time to first useful rows |
| 58 | Check latency for The Object Model: Git's Content-Addressable Store from Bytes Up in cold and warm cache states | Record steady frame cost |
| 59 | Check latency for The Object Model: Git's Content-Addressable Store from Bytes Up in cold and warm cache states | Record bytes accepted from child output |
| 60 | Check latency for The Object Model: Git's Content-Addressable Store from Bytes Up in cold and warm cache states | Record Git and gh process count |
| 61 | Check latency for The Object Model: Git's Content-Addressable Store from Bytes Up in cold and warm cache states | Record maximum retained document bytes |
| 62 | Check latency for The Object Model: Git's Content-Addressable Store from Bytes Up in cold and warm cache states | Record cache disposition and complete key |
| 63 | Check latency for The Object Model: Git's Content-Addressable Store from Bytes Up in cold and warm cache states | Record stale reply rejection |
| 64 | Check latency for The Object Model: Git's Content-Addressable Store from Bytes Up in cold and warm cache states | Record visible state after failure |
| 65 | Check peak memory for The Object Model: Git's Content-Addressable Store from Bytes Up in a small local repository | Record time to first useful rows |
| 66 | Check peak memory for The Object Model: Git's Content-Addressable Store from Bytes Up in a small local repository | Record steady frame cost |
| 67 | Check peak memory for The Object Model: Git's Content-Addressable Store from Bytes Up in a small local repository | Record bytes accepted from child output |
| 68 | Check peak memory for The Object Model: Git's Content-Addressable Store from Bytes Up in a small local repository | Record Git and gh process count |
| 69 | Check peak memory for The Object Model: Git's Content-Addressable Store from Bytes Up in a small local repository | Record maximum retained document bytes |
| 70 | Check peak memory for The Object Model: Git's Content-Addressable Store from Bytes Up in a small local repository | Record cache disposition and complete key |
| 71 | Check peak memory for The Object Model: Git's Content-Addressable Store from Bytes Up in a small local repository | Record stale reply rejection |
| 72 | Check peak memory for The Object Model: Git's Content-Addressable Store from Bytes Up in a small local repository | Record visible state after failure |
| 73 | Check peak memory for The Object Model: Git's Content-Addressable Store from Bytes Up in a monorepo with many changed paths | Record time to first useful rows |
| 74 | Check peak memory for The Object Model: Git's Content-Addressable Store from Bytes Up in a monorepo with many changed paths | Record steady frame cost |
| 75 | Check peak memory for The Object Model: Git's Content-Addressable Store from Bytes Up in a monorepo with many changed paths | Record bytes accepted from child output |
| 76 | Check peak memory for The Object Model: Git's Content-Addressable Store from Bytes Up in a monorepo with many changed paths | Record Git and gh process count |
| 77 | Check peak memory for The Object Model: Git's Content-Addressable Store from Bytes Up in a monorepo with many changed paths | Record maximum retained document bytes |
| 78 | Check peak memory for The Object Model: Git's Content-Addressable Store from Bytes Up in a monorepo with many changed paths | Record cache disposition and complete key |
| 79 | Check peak memory for The Object Model: Git's Content-Addressable Store from Bytes Up in a monorepo with many changed paths | Record stale reply rejection |
| 80 | Check peak memory for The Object Model: Git's Content-Addressable Store from Bytes Up in a monorepo with many changed paths | Record visible state after failure |
| 81 | Check peak memory for The Object Model: Git's Content-Addressable Store from Bytes Up in a pull request containing generated files | Record time to first useful rows |
| 82 | Check peak memory for The Object Model: Git's Content-Addressable Store from Bytes Up in a pull request containing generated files | Record steady frame cost |
| 83 | Check peak memory for The Object Model: Git's Content-Addressable Store from Bytes Up in a pull request containing generated files | Record bytes accepted from child output |
| 84 | Check peak memory for The Object Model: Git's Content-Addressable Store from Bytes Up in a pull request containing generated files | Record Git and gh process count |
| 85 | Check peak memory for The Object Model: Git's Content-Addressable Store from Bytes Up in a pull request containing generated files | Record maximum retained document bytes |
| 86 | Check peak memory for The Object Model: Git's Content-Addressable Store from Bytes Up in a pull request containing generated files | Record cache disposition and complete key |
| 87 | Check peak memory for The Object Model: Git's Content-Addressable Store from Bytes Up in a pull request containing generated files | Record stale reply rejection |
| 88 | Check peak memory for The Object Model: Git's Content-Addressable Store from Bytes Up in a pull request containing generated files | Record visible state after failure |
| 89 | Check peak memory for The Object Model: Git's Content-Addressable Store from Bytes Up in a deeply diverged branch | Record time to first useful rows |
| 90 | Check peak memory for The Object Model: Git's Content-Addressable Store from Bytes Up in a deeply diverged branch | Record steady frame cost |
| 91 | Check peak memory for The Object Model: Git's Content-Addressable Store from Bytes Up in a deeply diverged branch | Record bytes accepted from child output |
| 92 | Check peak memory for The Object Model: Git's Content-Addressable Store from Bytes Up in a deeply diverged branch | Record Git and gh process count |
| 93 | Check peak memory for The Object Model: Git's Content-Addressable Store from Bytes Up in a deeply diverged branch | Record maximum retained document bytes |
| 94 | Check peak memory for The Object Model: Git's Content-Addressable Store from Bytes Up in a deeply diverged branch | Record cache disposition and complete key |
| 95 | Check peak memory for The Object Model: Git's Content-Addressable Store from Bytes Up in a deeply diverged branch | Record stale reply rejection |
| 96 | Check peak memory for The Object Model: Git's Content-Addressable Store from Bytes Up in a deeply diverged branch | Record visible state after failure |
| 97 | Check peak memory for The Object Model: Git's Content-Addressable Store from Bytes Up in an unavailable network | Record time to first useful rows |
| 98 | Check peak memory for The Object Model: Git's Content-Addressable Store from Bytes Up in an unavailable network | Record steady frame cost |
| 99 | Check peak memory for The Object Model: Git's Content-Addressable Store from Bytes Up in an unavailable network | Record bytes accepted from child output |
| 100 | Check peak memory for The Object Model: Git's Content-Addressable Store from Bytes Up in an unavailable network | Record Git and gh process count |
| 101 | Check peak memory for The Object Model: Git's Content-Addressable Store from Bytes Up in an unavailable network | Record maximum retained document bytes |
| 102 | Check peak memory for The Object Model: Git's Content-Addressable Store from Bytes Up in an unavailable network | Record cache disposition and complete key |
| 103 | Check peak memory for The Object Model: Git's Content-Addressable Store from Bytes Up in an unavailable network | Record stale reply rejection |
| 104 | Check peak memory for The Object Model: Git's Content-Addressable Store from Bytes Up in an unavailable network | Record visible state after failure |
| 105 | Check peak memory for The Object Model: Git's Content-Addressable Store from Bytes Up in rapid keyboard navigation | Record time to first useful rows |
| 106 | Check peak memory for The Object Model: Git's Content-Addressable Store from Bytes Up in rapid keyboard navigation | Record steady frame cost |
| 107 | Check peak memory for The Object Model: Git's Content-Addressable Store from Bytes Up in rapid keyboard navigation | Record bytes accepted from child output |
| 108 | Check peak memory for The Object Model: Git's Content-Addressable Store from Bytes Up in rapid keyboard navigation | Record Git and gh process count |
| 109 | Check peak memory for The Object Model: Git's Content-Addressable Store from Bytes Up in rapid keyboard navigation | Record maximum retained document bytes |
| 110 | Check peak memory for The Object Model: Git's Content-Addressable Store from Bytes Up in rapid keyboard navigation | Record cache disposition and complete key |
| 111 | Check peak memory for The Object Model: Git's Content-Addressable Store from Bytes Up in rapid keyboard navigation | Record stale reply rejection |
| 112 | Check peak memory for The Object Model: Git's Content-Addressable Store from Bytes Up in rapid keyboard navigation | Record visible state after failure |
| 113 | Check peak memory for The Object Model: Git's Content-Addressable Store from Bytes Up in a linked worktree | Record time to first useful rows |
| 114 | Check peak memory for The Object Model: Git's Content-Addressable Store from Bytes Up in a linked worktree | Record steady frame cost |
| 115 | Check peak memory for The Object Model: Git's Content-Addressable Store from Bytes Up in a linked worktree | Record bytes accepted from child output |
| 116 | Check peak memory for The Object Model: Git's Content-Addressable Store from Bytes Up in a linked worktree | Record Git and gh process count |
| 117 | Check peak memory for The Object Model: Git's Content-Addressable Store from Bytes Up in a linked worktree | Record maximum retained document bytes |
| 118 | Check peak memory for The Object Model: Git's Content-Addressable Store from Bytes Up in a linked worktree | Record cache disposition and complete key |
| 119 | Check peak memory for The Object Model: Git's Content-Addressable Store from Bytes Up in a linked worktree | Record stale reply rejection |
| 120 | Check peak memory for The Object Model: Git's Content-Addressable Store from Bytes Up in a linked worktree | Record visible state after failure |
| 121 | Check peak memory for The Object Model: Git's Content-Addressable Store from Bytes Up in cold and warm cache states | Record time to first useful rows |
| 122 | Check peak memory for The Object Model: Git's Content-Addressable Store from Bytes Up in cold and warm cache states | Record steady frame cost |
| 123 | Check peak memory for The Object Model: Git's Content-Addressable Store from Bytes Up in cold and warm cache states | Record bytes accepted from child output |
| 124 | Check peak memory for The Object Model: Git's Content-Addressable Store from Bytes Up in cold and warm cache states | Record Git and gh process count |
| 125 | Check peak memory for The Object Model: Git's Content-Addressable Store from Bytes Up in cold and warm cache states | Record maximum retained document bytes |
| 126 | Check peak memory for The Object Model: Git's Content-Addressable Store from Bytes Up in cold and warm cache states | Record cache disposition and complete key |
| 127 | Check peak memory for The Object Model: Git's Content-Addressable Store from Bytes Up in cold and warm cache states | Record stale reply rejection |
| 128 | Check peak memory for The Object Model: Git's Content-Addressable Store from Bytes Up in cold and warm cache states | Record visible state after failure |
| 129 | Check network transfer for The Object Model: Git's Content-Addressable Store from Bytes Up in a small local repository | Record time to first useful rows |
| 130 | Check network transfer for The Object Model: Git's Content-Addressable Store from Bytes Up in a small local repository | Record steady frame cost |
| 131 | Check network transfer for The Object Model: Git's Content-Addressable Store from Bytes Up in a small local repository | Record bytes accepted from child output |
| 132 | Check network transfer for The Object Model: Git's Content-Addressable Store from Bytes Up in a small local repository | Record Git and gh process count |
| 133 | Check network transfer for The Object Model: Git's Content-Addressable Store from Bytes Up in a small local repository | Record maximum retained document bytes |
| 134 | Check network transfer for The Object Model: Git's Content-Addressable Store from Bytes Up in a small local repository | Record cache disposition and complete key |
| 135 | Check network transfer for The Object Model: Git's Content-Addressable Store from Bytes Up in a small local repository | Record stale reply rejection |
| 136 | Check network transfer for The Object Model: Git's Content-Addressable Store from Bytes Up in a small local repository | Record visible state after failure |
| 137 | Check network transfer for The Object Model: Git's Content-Addressable Store from Bytes Up in a monorepo with many changed paths | Record time to first useful rows |
| 138 | Check network transfer for The Object Model: Git's Content-Addressable Store from Bytes Up in a monorepo with many changed paths | Record steady frame cost |
| 139 | Check network transfer for The Object Model: Git's Content-Addressable Store from Bytes Up in a monorepo with many changed paths | Record bytes accepted from child output |
| 140 | Check network transfer for The Object Model: Git's Content-Addressable Store from Bytes Up in a monorepo with many changed paths | Record Git and gh process count |
| 141 | Check network transfer for The Object Model: Git's Content-Addressable Store from Bytes Up in a monorepo with many changed paths | Record maximum retained document bytes |
| 142 | Check network transfer for The Object Model: Git's Content-Addressable Store from Bytes Up in a monorepo with many changed paths | Record cache disposition and complete key |
| 143 | Check network transfer for The Object Model: Git's Content-Addressable Store from Bytes Up in a monorepo with many changed paths | Record stale reply rejection |
| 144 | Check network transfer for The Object Model: Git's Content-Addressable Store from Bytes Up in a monorepo with many changed paths | Record visible state after failure |
| 145 | Check network transfer for The Object Model: Git's Content-Addressable Store from Bytes Up in a pull request containing generated files | Record time to first useful rows |
| 146 | Check network transfer for The Object Model: Git's Content-Addressable Store from Bytes Up in a pull request containing generated files | Record steady frame cost |
| 147 | Check network transfer for The Object Model: Git's Content-Addressable Store from Bytes Up in a pull request containing generated files | Record bytes accepted from child output |
| 148 | Check network transfer for The Object Model: Git's Content-Addressable Store from Bytes Up in a pull request containing generated files | Record Git and gh process count |
| 149 | Check network transfer for The Object Model: Git's Content-Addressable Store from Bytes Up in a pull request containing generated files | Record maximum retained document bytes |
| 150 | Check network transfer for The Object Model: Git's Content-Addressable Store from Bytes Up in a pull request containing generated files | Record cache disposition and complete key |
| 151 | Check network transfer for The Object Model: Git's Content-Addressable Store from Bytes Up in a pull request containing generated files | Record stale reply rejection |
| 152 | Check network transfer for The Object Model: Git's Content-Addressable Store from Bytes Up in a pull request containing generated files | Record visible state after failure |
| 153 | Check network transfer for The Object Model: Git's Content-Addressable Store from Bytes Up in a deeply diverged branch | Record time to first useful rows |
| 154 | Check network transfer for The Object Model: Git's Content-Addressable Store from Bytes Up in a deeply diverged branch | Record steady frame cost |
| 155 | Check network transfer for The Object Model: Git's Content-Addressable Store from Bytes Up in a deeply diverged branch | Record bytes accepted from child output |
| 156 | Check network transfer for The Object Model: Git's Content-Addressable Store from Bytes Up in a deeply diverged branch | Record Git and gh process count |
| 157 | Check network transfer for The Object Model: Git's Content-Addressable Store from Bytes Up in a deeply diverged branch | Record maximum retained document bytes |
| 158 | Check network transfer for The Object Model: Git's Content-Addressable Store from Bytes Up in a deeply diverged branch | Record cache disposition and complete key |
| 159 | Check network transfer for The Object Model: Git's Content-Addressable Store from Bytes Up in a deeply diverged branch | Record stale reply rejection |
| 160 | Check network transfer for The Object Model: Git's Content-Addressable Store from Bytes Up in a deeply diverged branch | Record visible state after failure |
| 161 | Check network transfer for The Object Model: Git's Content-Addressable Store from Bytes Up in an unavailable network | Record time to first useful rows |
| 162 | Check network transfer for The Object Model: Git's Content-Addressable Store from Bytes Up in an unavailable network | Record steady frame cost |
| 163 | Check network transfer for The Object Model: Git's Content-Addressable Store from Bytes Up in an unavailable network | Record bytes accepted from child output |
| 164 | Check network transfer for The Object Model: Git's Content-Addressable Store from Bytes Up in an unavailable network | Record Git and gh process count |
| 165 | Check network transfer for The Object Model: Git's Content-Addressable Store from Bytes Up in an unavailable network | Record maximum retained document bytes |
| 166 | Check network transfer for The Object Model: Git's Content-Addressable Store from Bytes Up in an unavailable network | Record cache disposition and complete key |
| 167 | Check network transfer for The Object Model: Git's Content-Addressable Store from Bytes Up in an unavailable network | Record stale reply rejection |
| 168 | Check network transfer for The Object Model: Git's Content-Addressable Store from Bytes Up in an unavailable network | Record visible state after failure |
| 169 | Check network transfer for The Object Model: Git's Content-Addressable Store from Bytes Up in rapid keyboard navigation | Record time to first useful rows |
| 170 | Check network transfer for The Object Model: Git's Content-Addressable Store from Bytes Up in rapid keyboard navigation | Record steady frame cost |
| 171 | Check network transfer for The Object Model: Git's Content-Addressable Store from Bytes Up in rapid keyboard navigation | Record bytes accepted from child output |
| 172 | Check network transfer for The Object Model: Git's Content-Addressable Store from Bytes Up in rapid keyboard navigation | Record Git and gh process count |
| 173 | Check network transfer for The Object Model: Git's Content-Addressable Store from Bytes Up in rapid keyboard navigation | Record maximum retained document bytes |
| 174 | Check network transfer for The Object Model: Git's Content-Addressable Store from Bytes Up in rapid keyboard navigation | Record cache disposition and complete key |
| 175 | Check network transfer for The Object Model: Git's Content-Addressable Store from Bytes Up in rapid keyboard navigation | Record stale reply rejection |
| 176 | Check network transfer for The Object Model: Git's Content-Addressable Store from Bytes Up in rapid keyboard navigation | Record visible state after failure |
| 177 | Check network transfer for The Object Model: Git's Content-Addressable Store from Bytes Up in a linked worktree | Record time to first useful rows |
| 178 | Check network transfer for The Object Model: Git's Content-Addressable Store from Bytes Up in a linked worktree | Record steady frame cost |
| 179 | Check network transfer for The Object Model: Git's Content-Addressable Store from Bytes Up in a linked worktree | Record bytes accepted from child output |
| 180 | Check network transfer for The Object Model: Git's Content-Addressable Store from Bytes Up in a linked worktree | Record Git and gh process count |
| 181 | Check network transfer for The Object Model: Git's Content-Addressable Store from Bytes Up in a linked worktree | Record maximum retained document bytes |
| 182 | Check network transfer for The Object Model: Git's Content-Addressable Store from Bytes Up in a linked worktree | Record cache disposition and complete key |
| 183 | Check network transfer for The Object Model: Git's Content-Addressable Store from Bytes Up in a linked worktree | Record stale reply rejection |
| 184 | Check network transfer for The Object Model: Git's Content-Addressable Store from Bytes Up in a linked worktree | Record visible state after failure |
| 185 | Check network transfer for The Object Model: Git's Content-Addressable Store from Bytes Up in cold and warm cache states | Record time to first useful rows |
| 186 | Check network transfer for The Object Model: Git's Content-Addressable Store from Bytes Up in cold and warm cache states | Record steady frame cost |
| 187 | Check network transfer for The Object Model: Git's Content-Addressable Store from Bytes Up in cold and warm cache states | Record bytes accepted from child output |
| 188 | Check network transfer for The Object Model: Git's Content-Addressable Store from Bytes Up in cold and warm cache states | Record Git and gh process count |
| 189 | Check network transfer for The Object Model: Git's Content-Addressable Store from Bytes Up in cold and warm cache states | Record maximum retained document bytes |
| 190 | Check network transfer for The Object Model: Git's Content-Addressable Store from Bytes Up in cold and warm cache states | Record cache disposition and complete key |
| 191 | Check network transfer for The Object Model: Git's Content-Addressable Store from Bytes Up in cold and warm cache states | Record stale reply rejection |
| 192 | Check network transfer for The Object Model: Git's Content-Addressable Store from Bytes Up in cold and warm cache states | Record visible state after failure |
| 193 | Check subprocess count for The Object Model: Git's Content-Addressable Store from Bytes Up in a small local repository | Record time to first useful rows |
| 194 | Check subprocess count for The Object Model: Git's Content-Addressable Store from Bytes Up in a small local repository | Record steady frame cost |
| 195 | Check subprocess count for The Object Model: Git's Content-Addressable Store from Bytes Up in a small local repository | Record bytes accepted from child output |
| 196 | Check subprocess count for The Object Model: Git's Content-Addressable Store from Bytes Up in a small local repository | Record Git and gh process count |
| 197 | Check subprocess count for The Object Model: Git's Content-Addressable Store from Bytes Up in a small local repository | Record maximum retained document bytes |
| 198 | Check subprocess count for The Object Model: Git's Content-Addressable Store from Bytes Up in a small local repository | Record cache disposition and complete key |
| 199 | Check subprocess count for The Object Model: Git's Content-Addressable Store from Bytes Up in a small local repository | Record stale reply rejection |
| 200 | Check subprocess count for The Object Model: Git's Content-Addressable Store from Bytes Up in a small local repository | Record visible state after failure |
| 201 | Check subprocess count for The Object Model: Git's Content-Addressable Store from Bytes Up in a monorepo with many changed paths | Record time to first useful rows |
| 202 | Check subprocess count for The Object Model: Git's Content-Addressable Store from Bytes Up in a monorepo with many changed paths | Record steady frame cost |
| 203 | Check subprocess count for The Object Model: Git's Content-Addressable Store from Bytes Up in a monorepo with many changed paths | Record bytes accepted from child output |
| 204 | Check subprocess count for The Object Model: Git's Content-Addressable Store from Bytes Up in a monorepo with many changed paths | Record Git and gh process count |
| 205 | Check subprocess count for The Object Model: Git's Content-Addressable Store from Bytes Up in a monorepo with many changed paths | Record maximum retained document bytes |
| 206 | Check subprocess count for The Object Model: Git's Content-Addressable Store from Bytes Up in a monorepo with many changed paths | Record cache disposition and complete key |
| 207 | Check subprocess count for The Object Model: Git's Content-Addressable Store from Bytes Up in a monorepo with many changed paths | Record stale reply rejection |
| 208 | Check subprocess count for The Object Model: Git's Content-Addressable Store from Bytes Up in a monorepo with many changed paths | Record visible state after failure |
| 209 | Check subprocess count for The Object Model: Git's Content-Addressable Store from Bytes Up in a pull request containing generated files | Record time to first useful rows |
| 210 | Check subprocess count for The Object Model: Git's Content-Addressable Store from Bytes Up in a pull request containing generated files | Record steady frame cost |
| 211 | Check subprocess count for The Object Model: Git's Content-Addressable Store from Bytes Up in a pull request containing generated files | Record bytes accepted from child output |
| 212 | Check subprocess count for The Object Model: Git's Content-Addressable Store from Bytes Up in a pull request containing generated files | Record Git and gh process count |
| 213 | Check subprocess count for The Object Model: Git's Content-Addressable Store from Bytes Up in a pull request containing generated files | Record maximum retained document bytes |
| 214 | Check subprocess count for The Object Model: Git's Content-Addressable Store from Bytes Up in a pull request containing generated files | Record cache disposition and complete key |
| 215 | Check subprocess count for The Object Model: Git's Content-Addressable Store from Bytes Up in a pull request containing generated files | Record stale reply rejection |
| 216 | Check subprocess count for The Object Model: Git's Content-Addressable Store from Bytes Up in a pull request containing generated files | Record visible state after failure |
| 217 | Check subprocess count for The Object Model: Git's Content-Addressable Store from Bytes Up in a deeply diverged branch | Record time to first useful rows |
| 218 | Check subprocess count for The Object Model: Git's Content-Addressable Store from Bytes Up in a deeply diverged branch | Record steady frame cost |
| 219 | Check subprocess count for The Object Model: Git's Content-Addressable Store from Bytes Up in a deeply diverged branch | Record bytes accepted from child output |
| 220 | Check subprocess count for The Object Model: Git's Content-Addressable Store from Bytes Up in a deeply diverged branch | Record Git and gh process count |
| 221 | Check subprocess count for The Object Model: Git's Content-Addressable Store from Bytes Up in a deeply diverged branch | Record maximum retained document bytes |
| 222 | Check subprocess count for The Object Model: Git's Content-Addressable Store from Bytes Up in a deeply diverged branch | Record cache disposition and complete key |
| 223 | Check subprocess count for The Object Model: Git's Content-Addressable Store from Bytes Up in a deeply diverged branch | Record stale reply rejection |
| 224 | Check subprocess count for The Object Model: Git's Content-Addressable Store from Bytes Up in a deeply diverged branch | Record visible state after failure |
| 225 | Check subprocess count for The Object Model: Git's Content-Addressable Store from Bytes Up in an unavailable network | Record time to first useful rows |
| 226 | Check subprocess count for The Object Model: Git's Content-Addressable Store from Bytes Up in an unavailable network | Record steady frame cost |
| 227 | Check subprocess count for The Object Model: Git's Content-Addressable Store from Bytes Up in an unavailable network | Record bytes accepted from child output |
| 228 | Check subprocess count for The Object Model: Git's Content-Addressable Store from Bytes Up in an unavailable network | Record Git and gh process count |
| 229 | Check subprocess count for The Object Model: Git's Content-Addressable Store from Bytes Up in an unavailable network | Record maximum retained document bytes |
| 230 | Check subprocess count for The Object Model: Git's Content-Addressable Store from Bytes Up in an unavailable network | Record cache disposition and complete key |
| 231 | Check subprocess count for The Object Model: Git's Content-Addressable Store from Bytes Up in an unavailable network | Record stale reply rejection |
| 232 | Check subprocess count for The Object Model: Git's Content-Addressable Store from Bytes Up in an unavailable network | Record visible state after failure |
| 233 | Check subprocess count for The Object Model: Git's Content-Addressable Store from Bytes Up in rapid keyboard navigation | Record time to first useful rows |
| 234 | Check subprocess count for The Object Model: Git's Content-Addressable Store from Bytes Up in rapid keyboard navigation | Record steady frame cost |
| 235 | Check subprocess count for The Object Model: Git's Content-Addressable Store from Bytes Up in rapid keyboard navigation | Record bytes accepted from child output |
| 236 | Check subprocess count for The Object Model: Git's Content-Addressable Store from Bytes Up in rapid keyboard navigation | Record Git and gh process count |
| 237 | Check subprocess count for The Object Model: Git's Content-Addressable Store from Bytes Up in rapid keyboard navigation | Record maximum retained document bytes |
| 238 | Check subprocess count for The Object Model: Git's Content-Addressable Store from Bytes Up in rapid keyboard navigation | Record cache disposition and complete key |
| 239 | Check subprocess count for The Object Model: Git's Content-Addressable Store from Bytes Up in rapid keyboard navigation | Record stale reply rejection |
| 240 | Check subprocess count for The Object Model: Git's Content-Addressable Store from Bytes Up in rapid keyboard navigation | Record visible state after failure |
| 241 | Check subprocess count for The Object Model: Git's Content-Addressable Store from Bytes Up in a linked worktree | Record time to first useful rows |
| 242 | Check subprocess count for The Object Model: Git's Content-Addressable Store from Bytes Up in a linked worktree | Record steady frame cost |
| 243 | Check subprocess count for The Object Model: Git's Content-Addressable Store from Bytes Up in a linked worktree | Record bytes accepted from child output |
| 244 | Check subprocess count for The Object Model: Git's Content-Addressable Store from Bytes Up in a linked worktree | Record Git and gh process count |
| 245 | Check subprocess count for The Object Model: Git's Content-Addressable Store from Bytes Up in a linked worktree | Record maximum retained document bytes |
| 246 | Check subprocess count for The Object Model: Git's Content-Addressable Store from Bytes Up in a linked worktree | Record cache disposition and complete key |
| 247 | Check subprocess count for The Object Model: Git's Content-Addressable Store from Bytes Up in a linked worktree | Record stale reply rejection |
| 248 | Check subprocess count for The Object Model: Git's Content-Addressable Store from Bytes Up in a linked worktree | Record visible state after failure |
| 249 | Check subprocess count for The Object Model: Git's Content-Addressable Store from Bytes Up in cold and warm cache states | Record time to first useful rows |
| 250 | Check subprocess count for The Object Model: Git's Content-Addressable Store from Bytes Up in cold and warm cache states | Record steady frame cost |
| 251 | Check subprocess count for The Object Model: Git's Content-Addressable Store from Bytes Up in cold and warm cache states | Record bytes accepted from child output |
| 252 | Check subprocess count for The Object Model: Git's Content-Addressable Store from Bytes Up in cold and warm cache states | Record Git and gh process count |
| 253 | Check subprocess count for The Object Model: Git's Content-Addressable Store from Bytes Up in cold and warm cache states | Record maximum retained document bytes |
| 254 | Check subprocess count for The Object Model: Git's Content-Addressable Store from Bytes Up in cold and warm cache states | Record cache disposition and complete key |
| 255 | Check subprocess count for The Object Model: Git's Content-Addressable Store from Bytes Up in cold and warm cache states | Record stale reply rejection |
| 256 | Check subprocess count for The Object Model: Git's Content-Addressable Store from Bytes Up in cold and warm cache states | Record visible state after failure |
| 257 | Check cache identity for The Object Model: Git's Content-Addressable Store from Bytes Up in a small local repository | Record time to first useful rows |
| 258 | Check cache identity for The Object Model: Git's Content-Addressable Store from Bytes Up in a small local repository | Record steady frame cost |
| 259 | Check cache identity for The Object Model: Git's Content-Addressable Store from Bytes Up in a small local repository | Record bytes accepted from child output |
| 260 | Check cache identity for The Object Model: Git's Content-Addressable Store from Bytes Up in a small local repository | Record Git and gh process count |
| 261 | Check cache identity for The Object Model: Git's Content-Addressable Store from Bytes Up in a small local repository | Record maximum retained document bytes |
| 262 | Check cache identity for The Object Model: Git's Content-Addressable Store from Bytes Up in a small local repository | Record cache disposition and complete key |
| 263 | Check cache identity for The Object Model: Git's Content-Addressable Store from Bytes Up in a small local repository | Record stale reply rejection |
| 264 | Check cache identity for The Object Model: Git's Content-Addressable Store from Bytes Up in a small local repository | Record visible state after failure |
| 265 | Check cache identity for The Object Model: Git's Content-Addressable Store from Bytes Up in a monorepo with many changed paths | Record time to first useful rows |
| 266 | Check cache identity for The Object Model: Git's Content-Addressable Store from Bytes Up in a monorepo with many changed paths | Record steady frame cost |
| 267 | Check cache identity for The Object Model: Git's Content-Addressable Store from Bytes Up in a monorepo with many changed paths | Record bytes accepted from child output |
| 268 | Check cache identity for The Object Model: Git's Content-Addressable Store from Bytes Up in a monorepo with many changed paths | Record Git and gh process count |
| 269 | Check cache identity for The Object Model: Git's Content-Addressable Store from Bytes Up in a monorepo with many changed paths | Record maximum retained document bytes |
| 270 | Check cache identity for The Object Model: Git's Content-Addressable Store from Bytes Up in a monorepo with many changed paths | Record cache disposition and complete key |
| 271 | Check cache identity for The Object Model: Git's Content-Addressable Store from Bytes Up in a monorepo with many changed paths | Record stale reply rejection |
| 272 | Check cache identity for The Object Model: Git's Content-Addressable Store from Bytes Up in a monorepo with many changed paths | Record visible state after failure |
| 273 | Check cache identity for The Object Model: Git's Content-Addressable Store from Bytes Up in a pull request containing generated files | Record time to first useful rows |
| 274 | Check cache identity for The Object Model: Git's Content-Addressable Store from Bytes Up in a pull request containing generated files | Record steady frame cost |
| 275 | Check cache identity for The Object Model: Git's Content-Addressable Store from Bytes Up in a pull request containing generated files | Record bytes accepted from child output |
| 276 | Check cache identity for The Object Model: Git's Content-Addressable Store from Bytes Up in a pull request containing generated files | Record Git and gh process count |
| 277 | Check cache identity for The Object Model: Git's Content-Addressable Store from Bytes Up in a pull request containing generated files | Record maximum retained document bytes |
| 278 | Check cache identity for The Object Model: Git's Content-Addressable Store from Bytes Up in a pull request containing generated files | Record cache disposition and complete key |
| 279 | Check cache identity for The Object Model: Git's Content-Addressable Store from Bytes Up in a pull request containing generated files | Record stale reply rejection |
| 280 | Check cache identity for The Object Model: Git's Content-Addressable Store from Bytes Up in a pull request containing generated files | Record visible state after failure |
| 281 | Check cache identity for The Object Model: Git's Content-Addressable Store from Bytes Up in a deeply diverged branch | Record time to first useful rows |
