# Packfiles and Deltas

This page walks the pack machinery end to end: the on-disk pack format byte by byte, ref and
offset deltas, delta chains, the `.idx` fan-out index, thin packs on the wire, repacking and
garbage collection, the multi-pack-index, commit-graph files, and reachability bitmaps. It then
maps every one of those mechanisms onto the code Quinjet ships: why a depth-1 fetch produces a
tiny pack, why `blob:none` packs defer all blob transfer, what a promisor pack is and why it made
a local `--numstat` the dominant cost of opening a huge pull request until PR #49 removed it, and
how the 8 MiB capped pipe reads defend the process against pack-inflated subprocess output.
Quinjet never parses a pack itself; every mechanism here matters because it is the cost model
behind every `git` subprocess the app spawns.

## Contents

- [Why packfiles exist](#why-packfiles-exist)
- [The pack format byte by byte](#the-pack-format-byte-by-byte)
- [Delta compression inside a pack](#delta-compression-inside-a-pack)
- [The pack index: fan-out and binary search](#the-pack-index-fan-out-and-binary-search)
- [Thin packs on the wire](#thin-packs-on-the-wire)
- [Repack, gc, and pack maintenance](#repack-gc-and-pack-maintenance)
- [The multi-pack-index](#the-multi-pack-index)
- [Commit-graph files](#commit-graph-files)
- [Reachability bitmaps](#reachability-bitmaps)
- [Cheap bytes, expensive inflation](#cheap-bytes-expensive-inflation)
- [Quinjet's stance: subprocesses over pack parsing](#quinjets-stance-subprocesses-over-pack-parsing)
- [Small packs by construction: depth-1 and blob:none](#small-packs-by-construction-depth-1-and-blobnone)
- [Promisor packs and the lazy-fetch trap behind numstat](#promisor-packs-and-the-lazy-fetch-trap-behind-numstat)
- [Capped reads: the 8 MiB defense against pack-inflated output](#capped-reads-the-8-mib-defense-against-pack-inflated-output)
- [A worked end-to-end example: opening bun#30412](#a-worked-end-to-end-example-opening-bun30412)
- [Design alternatives and why they lost](#design-alternatives-and-why-they-lost)
- [Failure modes and edge cases](#failure-modes-and-edge-cases)
- [Related pages](#related-pages)

## Why packfiles exist

Git's primary storage unit is the loose object: one zlib-deflated file per object under
`.git/objects/ab/cdef...`, holding the `"type size\0content"` byte stream whose hash is the
object's name. The loose format is covered in depth in [the object model page](./object-model.md);
what matters here is its cost profile.

Loose objects are ideal for writes and terrible for scale:

**1. One file per object multiplies filesystem overhead.** A repository with a million objects
as loose files pays a million inodes, a million open/read/close cycles for a full walk, and
directory lookups fanned out over only 256 buckets. Filesystems handle this badly, and network
filesystems handle it catastrophically.

**2. zlib alone cannot exploit similarity between objects.** Every revision of a 100 KiB source
file stores a fresh ~30 KiB deflate stream, even when two adjacent revisions differ by one line.
Compression that only ever sees one object at a time cannot see that the repository is mostly
near-duplicates of itself.

**3. There is no way to ship loose objects efficiently.** A fetch that sent objects one at a time
would pay per-object round trips and per-object compression setup for what is fundamentally one
stream of highly redundant data.

The packfile answers all three at once. A pack is a single file that concatenates many objects,
stores most of them as *deltas* against similar objects, and zlib-compresses each entry
individually so any entry can be inflated without touching its neighbors. Packs serve double
duty: they are both the at-rest storage format (`.git/objects/pack/pack-<hash>.pack`, produced by
[`git repack`](https://git-scm.com/docs/git-repack) and
[`git gc`](https://git-scm.com/docs/git-gc)) and the wire format (every fetch and push is a pack
stream generated on the fly by [`git pack-objects`](https://git-scm.com/docs/git-pack-objects)).
The authoritative format description is
[`gitformat-pack`](https://git-scm.com/docs/gitformat-pack).

That double duty is exactly why this page exists in Quinjet's optimization docs. When Quinjet
fetches a pull-request head into its disposable workspace, the server runs `pack-objects` and the
bytes on the wire are a pack. When Quinjet then asks `git diff` for a patch, Git resolves objects
out of that pack, walking delta chains and inflating zlib streams. The number of bytes fetched,
the time to first diff, and the worst-case memory of a runaway subprocess are all functions of
pack mechanics, and every flag in `src/git/github/mod.rs` that this page quotes exists to bend
one of those functions.

## The pack format byte by byte

A pack file has three parts: a 12-byte header, a body of object entries, and a trailing checksum.

### The header and trailer

```text
offset  size  field
------  ----  -----------------------------------------------
0       4     signature: the ASCII bytes "PACK"
4       4     version number, big-endian; 2 in practice
8       4     number of objects in the pack, big-endian
12      ...   object entries, back to back
end-20  20    SHA-1 of every preceding byte of the pack
```

The version field is 2 for every pack Git writes today (version 3 is defined but unused). The
object count means a pack can hold at most 2^32 - 1 objects, a limit no real repository
approaches. The trailer hashes the entire preceding content, so any truncation or corruption of
a pack is detectable without an index; `git index-pack` verifies it while building the `.idx`.
In SHA-256 repositories the trailer is a 32-byte SHA-256 instead.

There is no table of contents inside the pack itself. Entries simply follow one another, and each
entry's length is discoverable only by decoding it. Random access is the job of the separate
`.idx` file described below; the pack alone is a purely sequential format, which is exactly what
a network stream needs.

### The object entry header

Each entry starts with a variable-length header that encodes the object's type and its
*inflated* size, followed by the zlib-deflated data (for deltas, the deflated delta payload).

The first header byte is packed as:

```text
bit 7      continuation flag: 1 means another header byte follows
bits 6-4   object type, 3 bits
bits 3-0   the low 4 bits of the inflated size
```

Continuation bytes each carry 7 more size bits (bit 7 again the continuation flag), accumulated
little-endian: each later byte contributes more significant bits.

The 3-bit type field:

| Value | Name            | Meaning                                        |
| ----- | --------------- | ---------------------------------------------- |
| 1     | `OBJ_COMMIT`    | full commit object                             |
| 2     | `OBJ_TREE`      | full tree object                               |
| 3     | `OBJ_BLOB`      | full blob object                               |
| 4     | `OBJ_TAG`       | full annotated tag object                      |
| 6     | `OBJ_OFS_DELTA` | delta whose base is named by a backward offset |
| 7     | `OBJ_REF_DELTA` | delta whose base is named by its full OID      |

Values 0 and 5 are reserved and invalid. Types 1 through 4 are the real object types from the
object model; types 6 and 7 exist only inside packs and are the whole point of the format.

### Worked example: encoding an object header

A 10-byte blob fits in a single header byte:

```text
size 10 = 0b1010, fits in 4 bits
byte 0:  0 011 1010  =  0x3A
         ^ ^^^ ^^^^
         | |   low 4 size bits (1010 = 10)
         | type 3 = OBJ_BLOB
         no continuation
```

A 1,234-byte blob needs two bytes:

```text
size 1234 = 0b100_1101_0010
low 4 bits          = 0010          (goes in byte 0)
remaining 7 bits    = 1001101 = 77  (goes in byte 1)

byte 0:  1 011 0010  =  0xB2   (continuation set, type blob, size bits 0010)
byte 1:  0 1001101   =  0x4D   (last byte, 7 size bits)

decoded size = 0b0010 | (77 << 4) = 2 + 1232 = 1234
```

A 253-byte commit also needs two bytes:

```text
size 253 = 0b1111_1101
byte 0:  1 001 1101  =  0x9D   (continuation, type 1 = commit, low bits 1101)
byte 1:  0 0001111   =  0x0F   (7 more bits: 15)

decoded size = 0b1101 | (15 << 4) = 13 + 240 = 253
```

Two properties of this encoding matter downstream. First, the recorded size is the *inflated*
size, not the compressed size; a reader knows exactly how much memory an entry will occupy before
inflating it, which is how tools bound their buffers. Second, small objects pay one header byte,
and even multi-gigabyte objects pay only five or six; the format has essentially no per-object
framing overhead, which is why a pack of a hundred thousand tiny tree objects stays small.

### What follows the header

For types 1 through 4, the header is followed directly by the zlib stream of the raw object
content (without the `"type size\0"` prefix used by loose objects; the pack header already
carries type and size). For `OBJ_REF_DELTA`, a 20-byte binary OID of the base object precedes the
zlib stream. For `OBJ_OFS_DELTA`, a variable-length backward offset precedes it, encoded as
described in the next section. In both delta cases the zlib stream contains a delta program, not
object content.

The zlib streams are self-terminating, which is how a sequential reader (such as `git
index-pack` consuming a fetch) finds each next entry without an index: inflate until the stream
ends, and the next entry header begins at the following byte.

## Delta compression inside a pack

Deltas are where the real compression happens. A delta entry stores an object as an edit script
against a *base* object elsewhere in the pack (or, for thin packs, outside it). Git's delta
format is a compact binary language with exactly two instructions.

### Naming the base: ref deltas and offset deltas

**1. `OBJ_REF_DELTA` names its base by full OID.** The 20 raw bytes of the base's object name sit
between the entry header and the zlib stream. The reader must then locate that object by hash,
wherever it lives. This is the older form, and the only form usable when the base is not in the
same pack, which makes it the currency of thin packs on the wire.

**2. `OBJ_OFS_DELTA` names its base by backward distance.** The encoded value is the number of
bytes between the start of the delta entry and the start of the base entry, earlier in the same
pack. The reader seeks backward; no hash lookup is needed. On-disk packs prefer this form: it
saves 17 to 18 bytes per delta versus a full OID and turns base resolution into pointer
arithmetic. It also guarantees the base precedes the delta, so a sequential reader has always
seen the base first.

The offset uses a deliberately quirky variable-length encoding. Bytes carry 7 value bits each,
most significant group first, with bit 7 as the continuation flag, and each continuation adds an
implicit `+1` to the accumulated high bits:

```text
value = byte0 & 0x7f
while continuation:
    value = ((value + 1) << 7) | (next_byte & 0x7f)
```

The `+1` removes redundant encodings: without it, `0x80 0x00` and `0x00` would both mean zero.
With it, every offset has exactly one encoding and two bytes reach 16,511 instead of 16,383.

Worked example, offset 1,000:

```text
1000 = (v + 1) * 128 + r   with r < 128
     = (6 + 1) * 128 + 104

byte 0:  1 0000110  =  0x86   (continuation, high group 6)
byte 1:  0 1101000  =  0x68   (final, low group 104)

decode:  v = 6; v = ((6 + 1) << 7) | 104 = 896 + 104 = 1000
```

### The delta program

The inflated delta payload begins with two varints in the same 7-bits-per-byte little-endian
style as the entry-size header: the expected size of the base object, then the size of the
result. Both are integrity checks; applying a delta to a base of the wrong size fails
immediately.

Then comes a sequence of instructions, one of two kinds, distinguished by the top bit of the
opcode byte:

**Copy (opcode bit 7 set).** Copy a range from the base into the output. The low 7 bits of the
opcode say which operand bytes follow:

```text
opcode:  1 sss oooo
         bits 0-3: which of 4 little-endian offset bytes are present
         bits 4-6: which of 3 little-endian size bytes are present
```

Absent bytes are zero. A size of zero after assembly means 0x10000 (65,536), the historical
maximum copy length. So `copy offset 0, size 0x10000` is the single byte `0x90`, and short
copies from the start of the base are two or three bytes.

**Insert (opcode bit 7 clear).** The low 7 bits are a literal count from 1 to 127; that many
bytes of new data follow inline. Runs of new content longer than 127 bytes become several insert
instructions. The all-zero opcode is reserved and invalid.

### Worked example: a 7-byte edit in a 1,024-byte file

Suppose a 1,024-byte blob differs from its base only in bytes 600 through 606, which were
replaced by 7 new bytes. The delta:

```text
80 08            varint: base size 1024
80 08            varint: result size 1024
B0 58 02         copy   offset 0,   size 600   (0x258; two size bytes present)
07 xx*7          insert 7 literal bytes
B3 5F 02 A1 01   copy   offset 607 (0x25F), size 417 (0x1A1)
```

Twenty bytes of delta program describe a kilobyte of content, and the delta then zlib-compresses
further. This 50x-before-zlib ratio is typical of source code history, where consecutive
revisions share almost everything, and it is the reason a repository's full history is routinely
smaller than its checkout.

### Delta chains and depth

A delta's base may itself be a delta. Resolving an object then walks a *chain*: find the deepest
full object, inflate it, apply each delta in turn. `git pack-objects` bounds chain length with
`--depth`, default 50 (`pack.depth` in [`git config`](https://git-scm.com/docs/git-config)); it
selects bases by sliding a window (default 10, `pack.window`) over objects sorted so that likely
relatives are adjacent: same type, similar path basename, size descending. Sorting size
descending makes deltas tend to run from newer, larger objects toward older ones, so recent
history, which is read most, sits shallow in its chains.

Chain depth is a pure read-time tax. A depth-40 object costs one full inflate plus 40 delta
applications, each of which needs its base materialized. Git amortizes this with an in-memory
delta-base cache (`core.deltaBaseCacheLimit`, 96 MiB by default) so that walking many objects
that share deep bases does not re-resolve the same chains, but the asymmetry stands: deltas make
bytes cheap and reads expensive. That asymmetry is the theme of this whole page, and the section
[Cheap bytes, expensive inflation](#cheap-bytes-expensive-inflation) returns to it.

`git verify-pack -v` prints per-object chain positions, and its histogram is a quick health check
of any pack:

```console
$ git verify-pack -v .git/objects/pack/pack-abc123.idx | tail -n 8
non delta: 4821 objects
chain length = 1: 3990 objects
chain length = 2: 2765 objects
chain length = 3: 1893 objects
...
```

## The pack index: fan-out and binary search

The pack itself answers "give me every object in order". Serving `git diff` requires the
opposite: "give me the entry for OID X, now". That is the `.idx` file's job, one per pack,
written by `git index-pack` when the pack arrives or by `git repack` when it is created.

### The idx v2 layout

```text
offset  size        field
------  ----------  ---------------------------------------------------
0       4           magic: 0xFF 0x74 0x4F 0x63 ("\377tOc")
4       4           version, big-endian: 2
8       1024        fan-out table: 256 big-endian cumulative counts
1032    20 * N      sorted table of all N object names
...     4 * N       CRC32 of each object's packed bytes
...     4 * N       31-bit pack offsets (top bit = large-offset flag)
...     8 * E       large-offset table for entries past 2 GiB
...     20          copy of the pack's trailing checksum
...     20          checksum of the idx itself
```

Version 1, still readable, interleaved 4-byte offsets with the OIDs and could not address packs
over 4 GiB; v2 split the tables, added CRCs (so a pack can be verified object by object and
re-served byte-exactly), and added the escape hatch for 64-bit offsets: a 4-byte offset with its
top bit set is an index into the 8-byte table instead.

### The fan-out table

The fan-out is the heart of the format. Entry `k` holds the count of objects whose first byte is
less than or equal to `k`; entry 255 is therefore the total object count. Because the OID table
is sorted, the two adjacent fan-out entries bracket exactly the slice of the table where an OID
can live.

Worked example. Take a pack of 4,000 objects and a lookup for an OID beginning `2a91...`:

```text
fanout[0x29] = 655     objects with first byte 0x00..0x29
fanout[0x2a] = 671     objects with first byte 0x00..0x2a

candidate slice: positions 655..670, sixteen entries
binary search: ceil(log2(16)) = 4 comparisons of 20-byte names
position found -> offset table -> seek into the pack
```

One 1 KiB table read replaces the first eight comparisons of a plain binary search, and the
remaining slice is 1/256th of the pack. The same trick, at the same table shape, reappears in the
multi-pack-index and the commit-graph below; and it is the on-disk twin of the in-memory pattern
Quinjet uses for its compile-time icon catalogs (sorted static tables, no allocation, bounded
probes; ARCHITECTURE.md invariant 1b).

The fan-out also serves abbreviation: to expand a short hash like `2a91f3c`, Git only needs the
same bracketed slice of each index to prove uniqueness. With many packs, that means touching
every `.idx`; the [multi-pack-index](#the-multi-pack-index) collapses those probes into one.

### What a lookup costs

The full path for "materialize object X from packs" is:

1. For each pack index (or once, via the multi-pack-index): fan-out bracket, binary search.
1. On a hit: read the 4-byte (or 8-byte) offset, seek the pack, decode the entry header.
1. If the entry is a delta: resolve the base (backward seek for `OFS`, recursive lookup for
   `REF`), then apply the chain from its bottom.
1. Inflate and hand the bytes to the caller.

Steps 1 and 2 are microseconds. Steps 3 and 4 are where a huge diff spends its time, and they
scale with inflated content size and chain depth, not with the pack's size on disk. Keep this
split in mind for the Quinjet sections: fetching fewer bytes (steps the server takes) and
inflating fewer objects (steps the client takes) are two different optimizations, and the
optimization stack needed both.

## Thin packs on the wire

The pack sent by a fetch is not quite the pack stored on disk. Wire packs may be *thin*: they may
contain `OBJ_REF_DELTA` entries whose base object is not in the pack at all, because the sender
knows the receiver already has it.

The knowledge comes from negotiation. During a fetch, the client advertises `have` lines naming
commits it possesses; the server computes the set difference and, crucially, may delta new
objects against any object reachable from the common commits. A one-line change to a large file
then crosses the wire as a few dozen delta bytes referencing a blob that only the receiver holds.

A thin pack is illegal at rest: an on-disk pack must be self-contained. The receiving side runs
`git index-pack --fix-thin` (see [`git index-pack`](https://git-scm.com/docs/git-index-pack)) on
the stream, which detects external bases, appends a full copy of each missing base object to the
end of the pack, recomputes the trailing checksum, and only then writes the `.idx`. The received
pack therefore grows slightly on landing, by exactly the bases it referenced.

Two consequences are worth keeping:

**1. Incremental fetches are priced by novelty, not by size.** The bytes transferred scale with
what changed since the common ancestor, not with how big the touched files are. Every fetch
Quinjet's PR workspace performs benefits, and the deepening ladder described later re-runs
fetches at growing depths precisely because each re-run only pays for the newly exposed commits.

**2. Small fetched packs are normal, not suspicious.** A repository can absorb a large logical
change as a small pack. The corollary, that a small pack can inflate into a huge diff, is the
amplification problem the capped-read section below is about.

For completeness: `transfer.unpackLimit` (default 100) decides whether a small received pack is
kept as a pack or exploded into loose objects; big fetches always stay packs.

## Repack, gc, and pack maintenance

Left alone, a repository accretes one pack per fetch plus a scatter of loose objects from local
commits. Read performance decays: every object lookup probes every index, deltas cannot span
packs, and duplicate objects accumulate. Maintenance folds this state back into a few good packs.

**1. `git gc --auto` is the trigger.** It fires when thresholds pass: more than `gc.auto` loose
objects (default 6,700) or more than `gc.autoPackLimit` packs (default 50). Modern Git can spread
the same work across [`git maintenance`](https://git-scm.com/docs/git-maintenance) tasks instead.

**2. `git repack -a -d` rebuilds the world.** All reachable objects are rewritten into one new
pack, with a fresh delta search across everything, then superseded packs and loose copies are
deleted. This produces the best chains and the smallest disk footprint, at a CPU and memory cost
proportional to the whole repository.

**3. Geometric repacking bounds the middle ground.** `git repack --geometric=2` maintains packs
in a size progression where each pack is at least twice as large as the next smaller one, merging
only the small tail of packs on each run. Amortized, every object is rewritten O(log N) times
over its life instead of on every repack. Large hosts (GitHub included) keep repositories healthy
with this shape of maintenance, which is one reason server-side fetch generation stays fast for a
repository the size of `oven-sh/bun`.

**4. Cruft packs quarantine the unreachable.** Instead of exploding unreachable objects into
loose files awaiting expiry, `git repack --cruft` collects them into a single cruft pack with a
companion `.mtimes` file, keeping the object database tidy while preserving the grace period.

Quinjet deliberately performs none of this on its own repositories. The disposable PR workspace
(`TemporaryBareRepository`, `src/git/github/mod.rs:1689-1726`) lives for one prepared pull
request and is deleted on `Drop`, so it never lives long enough to need maintenance; its object
store is whatever packs its few fetches delivered. The user's opened repository is never mutated
at all (ARCHITECTURE.md invariant 9), so its pack health remains whatever the user's own Git
habits produce.

## The multi-pack-index

A busy repository between repacks holds many packs, and a plain object lookup must binary-search
every one of their indexes. The multi-pack-index (midx) is one index over many packs:
`.git/objects/pack/multi-pack-index`, written by
[`git multi-pack-index`](https://git-scm.com/docs/git-multi-pack-index) or as a side effect of
maintenance.

The file is chunk-based: a small header with the magic `MIDX`, a chunk lookup table, then chunks
identified by four-byte names:

```text
PNAM   the covered pack names, NUL-separated
OIDF   a 256-entry fan-out over all objects of all covered packs
OIDL   the sorted object names, merged across packs
OOFF   per object: 4-byte pack id + 4-byte offset within that pack
LOFF   8-byte large offsets, referenced from OOFF when needed
```

A lookup becomes: one fan-out bracket, one binary search over the merged name table, then a
(pack, offset) pair, regardless of how many packs exist. Hash abbreviation gets the same benefit,
which is visible in interactive tooling: expanding short hashes stops scaling with pack count.
If the same object exists in several covered packs, the midx names one canonical copy, which
also stabilizes which entry serves reads.

The midx matters to Quinjet mostly on the user's side of the fence. Quinjet issues many small
object-addressed reads per session (`git cat-file -e` probes, path-scoped `git diff`, history
pages), so a well-maintained opened repository with a midx answers the network-free PR preview
path (`has_commit` at `src/git/mod.rs:790-799`) with two cheap index probes. The disposable
workspace, holding a handful of packs from a handful of fetches, never accumulates enough packs
for the midx to matter.

## Commit-graph files

Packs answer "give me object X". History questions, such as "is A an ancestor of B" and "what is
the merge base", require walking commits: inflate a commit, parse its parent lines, repeat. Even
with packs this is real parsing work over potentially hundreds of thousands of commits.

The commit-graph file (`.git/objects/info/commit-graph`, format in
[`gitformat-commit-graph`](https://git-scm.com/docs/gitformat-commit-graph)) is a sidecar that
stores the walkable skeleton of history in fixed-width binary records. Same chunked shape as the
midx, magic `CGPH`:

```text
OIDF   256-entry fan-out over the indexed commits
OIDL   sorted commit names
CDAT   per commit: root tree OID, two parent positions,
       generation number and commit time packed into 8 bytes
EDGE   overflow list for octopus merges (more than two parents)
GDA2   corrected commit-date offsets (generation v2), when present
```

Parents are stored as 31-bit *positions into the same file*, not OIDs, so following a parent is
an array index instead of a hash lookup plus object inflation. A walk that touches a hundred
thousand commits does a hundred thousand array reads and zero zlib calls.

### Generation numbers

The `CDAT` records carry the file's most powerful idea. Generation v1 is the topological level:
1 for a root commit, otherwise one more than the maximum of the parents' generations. It yields a
pruning invariant: if `gen(A) <= gen(B)` and `A != B`, then `A` cannot reach `B`. Reachability
walks, `git merge-base`, ahead/behind counts, and `--contains` queries all use it to stop
exploring whole regions of the DAG without visiting them. Generation v2 (corrected commit dates)
keeps the same invariant while ordering walks closer to real time, which prunes better on
histories with clock skew.

### Why Quinjet's workspace cannot lean on it

Two facts collide. First, `git merge-base` is dramatically cheaper with a commit-graph. Second,
*shallow repositories neither write nor use commit-graph files*: a shallow boundary lies about
parents (the boundary commits' parents are cut), and a graph built over lies would corrupt
reachability answers, so Git disables the feature outright.

Quinjet's disposable PR workspace is always shallow: every fetch carries `--depth`. So the one
environment where Quinjet must answer a merge-base question from scratch is exactly the
environment where Git's own accelerator is unavailable, and each `git merge-base` there is a raw
commit-parsing walk over whatever shallow history has been fetched. That asymmetry is a large
part of why the optimization stack moved merge-base resolution off the local walk entirely and
onto the GitHub compare API (`merge_base_from_api`, `src/git/github/mod.rs:1288-1325`), keeping
the local deepening ladder only as a fallback. The full story, including multiple merge bases and
criss-cross histories, lives in [merge bases and history](./merge-bases-and-history.md).

## Reachability bitmaps

The last accelerator lives mostly server-side. When a fetch arrives, the server must compute the
set of objects reachable from the client's wants but not from its haves. Walking commits and
trees for a repository the size of a major open-source project on every fetch would be
prohibitive, so servers precompute *reachability bitmaps* (a `.bitmap` file beside a pack's
`.idx`, or beside the midx).

For a selection of commits, the bitmap file stores one bit per object in the pack, set when the
object is reachable from that commit, compressed with EWAH run-length encoding. It also stores
four type bitmaps marking which pack positions are commits, trees, blobs, and tags. Answering a
fetch becomes boolean algebra: OR the bitmaps of the wants, subtract the bitmaps of the haves,
and the surviving bits are the object list to pack, no graph walk at all. Commits without a
precomputed bitmap walk only the short distance to the nearest bitmapped ancestor.

The type bitmaps are what make partial clone cheap to serve. A filter like `blob:none` is one
more AND against the complement of the blob bitmap: the server drops every blob from the send
list without ever opening an object. When Quinjet's workspace requests its blob-less,
depth-limited fetches, the reason GitHub can answer them quickly is this data structure; the
filter that saves Quinjet transfer bytes costs the server almost nothing to apply. Bitmaps also
accelerate the server-side `rev-list --count` arithmetic behind the compare API's ahead/behind
and merge-base answers, which Quinjet substitutes for local history work wherever it can.

Client-side, bitmaps exist (`git rev-list --use-bitmap-index`) but rarely matter for a TUI's
workload, and never for Quinjet's shallow workspaces: like the commit-graph, bitmaps assume a
truthful, complete object graph, which a shallow promisor repository does not have.

## Cheap bytes, expensive inflation

Everything above compounds into one economic fact that shaped the whole optimization stack:
*transfer and storage costs scale with compressed deltas, while read costs scale with inflated
content*. The two are separated by multiple orders of magnitude, and any design that conflates
them budgets the wrong resource.

Consider the three currencies a pull-request view spends:

**1. Wire bytes.** Priced by pack mechanics: thin deltas against common history, zlib, filters.
A million-line PR whose lines are mostly new pays real bytes for the new blobs; a PR touching
existing files lightly pays almost nothing. Negotiation, depth, and filters let a client choose
how much history and how much content to buy.

**2. Inflation work.** Priced by what the reader actually materializes: full object bytes times
delta-chain depth, paid at `git diff` / `git show` time, on the client, per read. A pack can sit
on disk for years costing nothing; the moment a command needs blob contents, chains resolve and
zlib runs.

**3. Round trips.** In a *partial* clone, inflation of a missing blob is not CPU but a network
fetch (the promisor machinery below). The cost of "read this object" silently changes category,
from microseconds to a request, and code written for category 2 becomes catastrophic in
category 3.

The Quinjet sections that follow are three applications of this one fact. The fetch strategy
minimizes currency 1 by construction (depth limits, `blob:none`, API-resolved merge base). The
PR #49 counts change eliminated a currency-3 disaster: a `--numstat` that forced a network fetch
per changed blob. And the capped pipe reads bound currency 2's blast radius, because no matter
how small the pack was, the inflated diff text a subprocess can emit is unbounded, and something
must stop a 10 GiB patch from becoming a 10 GiB `Vec<u8>`.

## Quinjet's stance: subprocesses over pack parsing

Quinjet never links `libgit2` or `gitoxide` and contains no pack, idx, or delta decoder. Every
repository operation is a spawned `git` subprocess and every GitHub operation is a spawned `gh`
subprocess, with byte-oriented parsing of their stdout (NUL, unit-separator, and TSV framing,
never localized text). The catalog of exact invocations lives in
[plumbing and porcelain](./plumbing-and-porcelain.md); the layering that keeps all of this off
the render path is in ARCHITECTURE.md ("The terminal render path never spawns Git or performs
filesystem traversal").

That stance turns this page's material into a budget rather than an implementation guide. Quinjet
cannot make pack access faster; Git already does that as well as anything could. What Quinjet
*can* control is:

**1. Which packs come into existence.** Every fetch argument (`--depth`, `--filter`, `--no-tags`,
the refspec) is a instruction to the server's `pack-objects` about what to put in the wire pack.
Quinjet phrases its fetches so the packs are as small as the diff it needs.

**2. Which objects ever get inflated.** Name-status and numstat listings, tree-level diff
enumeration, `cat-file -e` existence probes, and API-sourced metadata all answer questions
without materializing blob content. Quinjet prefers each of these over any command that inflates
file contents, and defers patch generation until a file is actually wanted on screen.

**3. How much inflated output the process will accept.** Since subprocess stdout is the only
interface, a hard cap on every pipe read is the only memory guarantee available. Quinjet reads
everything through one bounded runner that kills the child at the cap.

The next three sections take these in order.

## Small packs by construction: depth-1 and blob:none

When a pull request's base and head commits already exist in the opened repository, Quinjet
fetches nothing at all: `prepare_pull_request_diff` (`src/git/github/mod.rs:767-822`) probes both
OIDs with `git cat-file -e <oid>^{commit}` and, on success, diffs inside the opened repository.
`src/git/mod.rs:790-799` shows the probe:

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

`-e` produces no stdout at all; it is two index probes (or one, through the midx) and an exit
code. This is the network-free path that makes previews of locally built or merged PRs instant,
and it is verified by the test `locally_available_pr_objects_avoid_disposable_fetches`
(`src/git/github/mod.rs:2946-2986`), which points the base repository URL at an unreachable host
and still requires prepare-plus-diff to finish in under 2 seconds.

Everything below concerns the other path: the PR's commits are not local, and a disposable bare
repository under the cache root must fetch them. Every byte that path transfers is a pack, and
every argument is chosen to shrink it.

### The fetch command and what each flag removes from the pack

`fetch_ref` (`src/git/github/mod.rs:1876-1909`) is the single choke point through which every
workspace fetch passes:

```rust
fn fetch_ref(temporary: &Path, remote: &str, refspec: &str, depth: usize) -> Result<()> {
    let args = [
        OsString::from("fetch"),
        OsString::from("--quiet"),
        OsString::from("--force"),
        OsString::from("--no-tags"),
        OsString::from("--filter=blob:none"),
        OsString::from(format!("--depth={depth}")),
        OsString::from(remote),
        OsString::from(refspec),
    ];
    let output = run_temp_git(temporary, &args, 128 * 1024, MAX_GH_ERROR_BYTES)?;
```

Read the flags as a series of subtractions from the wire pack that a plain
[`git fetch`](https://git-scm.com/docs/git-fetch) would request:

**1. `--depth=N` subtracts history.** The server walks at most `N` commits back from the
requested tip and records the cut points as shallow boundaries. The pack contains at most `N`
commit objects per side instead of the repository's full history, and, transitively, only the
trees and blobs those commits introduce. For a repository with hundreds of thousands of commits,
this is the difference between megabytes and gigabytes before any other flag applies.

**2. `--filter=blob:none` subtracts every blob.** The partial-clone filter (described in full in
[shallow and partial clone](./shallow-and-partial-clone.md) and in Git's own
[partial clone documentation](https://git-scm.com/docs/partial-clone)) tells `pack-objects` to
omit all blob objects from the pack. Server-side, with reachability bitmaps, this is one AND
against the type bitmaps. What arrives is commits and trees only: the *shape* of the change,
with none of its contents. Contents are promised for later (the promisor machinery in the next
section).

**3. `--no-tags` subtracts tag-following.** By default a fetch also grabs tags pointing into the
fetched history; each annotated tag drags its target and, on an unlucky repository, unrelated
history into the shallow clone. The workspace has no use for tags.

**4. `--force` subtracts failure modes rather than bytes.** All workspace refspecs are
`+`-prefixed anyway; updates to `refs/quinjet/*` must never fail on a non-fast-forward when a PR
branch was rebased between fetches.

The stdout cap of 128 KiB on this call is worth a glance: a fetch's stdout is diagnostics, not
data, so it gets a small bound while stderr keeps `MAX_GH_ERROR_BYTES` (256 KiB) for error
reporting.

If the filtered fetch fails, the same function retries the identical command without
`--filter=blob:none` (`src/git/github/mod.rs:1892-1901`): not every server enables
`uploadpack.allowFilter`, and a shallow-but-blobby fetch is still bounded by depth. Shallowness
is never given up.

### Ref choreography: what gets fetched, exactly

`fetch_pull_request` (`src/git/github/mod.rs:1781-1864`) arranges at most a handful of these
fetches. The refspecs pin everything under a private namespace:

```text
+refs/heads/{base_ref}:refs/quinjet/base      base branch, from the base repository
+refs/pull/{number}/head:refs/quinjet/head    GitHub's synthetic PR head ref
+{merge_base_hint}:refs/quinjet/merge-base    one exact commit, when the API supplied it
```

`refs/pull/{number}/head` is a GitHub-maintained ref that exists on the base repository even for
fork PRs, and even after the fork branch was renamed; it is the most stable name a PR's head has.
Only when that ref cannot be fetched does the code add the fork as a second remote and fetch
`+refs/heads/{head_ref}:refs/quinjet/head` from there. Everything lands under `refs/quinjet/*` so
no real ref in any repository is ever shadowed.

The head fetch runs at depth 64: enough history that a merge base with a recently updated base
branch is often already reachable, small enough that the pack stays tens of commits of trees.

### The depth-1 merge-base fetch: the smallest useful pack

The centerpiece is the short-circuit that usually makes the base branch's history irrelevant.
Before fetching, `prepare_pull_request_diff` asks the GitHub compare API for the merge base of
the two immutable PR OIDs (`merge_base_from_api`, cached forever under
`pr-merge-base-v1\n{repo_url}\n{base}\n{head}`; one `gh api
repos/{owner}/{repo}/compare/{base}...{head} --jq .merge_base_commit.sha` call). With that hint
in hand, `src/git/github/mod.rs:1834-1844` does this:

```rust
progress(PullRequestProgress::FindingMergeBase);
if let Some(hint) = merge_base_hint {
    let hint_refspec = format!("+{hint}:refs/quinjet/merge-base");
    if fetch_ref(temporary, "origin", &hint_refspec, 1).is_ok() {
        let head =
            preferred_fetched_commit(temporary, &pull_request.head_oid, "refs/quinjet/head")?;
        if head == pull_request.head_oid {
            return Ok((hint.to_owned(), head));
        }
    }
}
```

A fetch of one commit OID at `--depth=1` with `blob:none` asks the server for the smallest pack
that is still a valid diff anchor:

```text
contents of the depth-1 blob:none merge-base pack
--------------------------------------------------
1 commit object          the merge base itself
N tree objects           the trees of that one commit's snapshot,
                         minus any the negotiation proved common
0 blobs                  filtered
0 tags                   --no-tags
0 further commits        depth 1 cuts the parents off
```

Trees are small (a 20-byte OID plus mode and name per entry) and delta beautifully against each
other, so even a snapshot with tens of thousands of paths costs single-digit megabytes at most,
and usually far less once thin-pack deltas against the already-fetched head trees apply. Compare
this against the alternative the ladder represents: fetching *the entire base branch history*
deep enough to reach a merge base that may sit thousands of commits back.

The guard on `head == pull_request.head_oid` closes a correctness hole the adversarial review
found: the API hint was computed from the metadata snapshot's OIDs, so if a force-push landed
between the metadata read and the fetch, the hint could pair a stale merge base with a fresh
head, and the wrong file list would then be cached immutably under that OID pair. The hint is
used only when the fetched head still is the snapshot head; otherwise the code falls through to
the ladder and re-derives everything from what was actually fetched.

### The fallback ladder: paying for history in installments

When there is no hint (API failure, offline, non-GitHub-shaped answer), the workspace has to find
a merge base the local way, with `git merge-base` over shallow history that gets deepened until
an answer appears (`src/git/github/mod.rs:1846-1863`):

```rust
progress(PullRequestProgress::FetchingBase);
fetch_ref(temporary, "origin", &base_refspec, 64)?;
for depth in [64_usize, 256, 1_024, 4_096, 16_384] {
    if depth != 64 {
        fetch_ref(temporary, "origin", &base_refspec, depth)?;
        fetch_ref(temporary, &head_remote, &head_refspec, depth)?;
    }
    let base =
        preferred_fetched_commit(temporary, &pull_request.base_oid, "refs/quinjet/base")?;
    let head =
        preferred_fetched_commit(temporary, &pull_request.head_oid, "refs/quinjet/head")?;
    if let Some(merge_base) = try_merge_base(temporary, &base, &head)? {
        return Ok((merge_base, head));
    }
}
bail!(
    "Unable to find the PR merge base within 16,384 commits; refusing an unbounded history fetch"
)
```

Pack mechanics make the ladder cheaper than it looks. Each deepening re-fetch negotiates against
what the workspace already holds: the depth-256 fetch sends only the 192 newly exposed commits
(and their trees, thin-delta'd against known ones), not a fresh 256. The geometric progression
means total transfer across all rungs is at most about twice the final rung, and the final rung
is capped: past 16,384 commits of divergence the code refuses an unbounded history download and
fails with the message above, rather than degenerating into a full clone. (The pre-stack ladder
stopped at 4,096 and made that failure common on long-lived rewrite branches; PR #47 both added
the API hint that usually skips the ladder entirely and extended the ceiling.)

`try_merge_base` treats a non-zero `git merge-base` exit as "deepen further", not as an error:
inside a shallow boundary the command legitimately cannot see a common ancestor yet. And
`preferred_fetched_commit` re-resolves the advertised OIDs (`git rev-parse --verify
{oid}^{commit}`) in preference to whatever the refs point at now, pinning the diff to the exact
commits the metadata described even if branches moved mid-flight.

### The concrete datum: a 389 MB bun

The benchmark environment for the whole optimization stack makes the transfer arithmetic
tangible. The session's test clone of `oven-sh/bun` (the repository behind the
[bun](https://github.com/oven-sh/bun) project), used to drive `quinjet pr view/files/diff/
conversation 30412` against the "Rewrite Bun in Rust" pull request (2,188 changed files,
+1,009,257 added lines), was created as a shallow `blob:none` clone and measured on disk at
389 MB. Its exact configuration, as recorded during the session:

```text
/tmp/bun-test git config
------------------------------------------------------------------
core.repositoryformatversion = 1
remote.origin.url            = https://github.com/oven-sh/bun
remote.origin.fetch          = +refs/heads/main:refs/remotes/origin/main
remote.origin.promisor       = true
remote.origin.partialclonefilter = blob:none
git rev-parse --is-shallow-repository = true
```

Every line of that config is a pack-machinery statement. `repositoryformatversion = 1` opts into
extensions, which partial clone requires. The single-branch fetch refspec keeps other branches'
history out of every negotiation. `partialclonefilter = blob:none` re-applies the filter to every
subsequent fetch, and `promisor = true` marks `origin` as the remote that stands behind the
missing blobs: the subject of the next section. The 389 MB on disk is the packs of commits and
trees for one branch of one of the largest active open-source repositories, plus whichever blobs
lazy fetches had already pulled in; the blobs it does *not* contain are the point.

The measured effect of the whole strategy on that clone, quoted from the session's verification
rounds with their context: the first round recorded "Metadata in 1.7s" (`pr view` against
bun#30412, cold) and "The rewrite PR enumerates all 2,188 files with real counts in 18.5s cold."
with a warm re-run of the index at 0.04s and single-file patches at 0.1s; after the review-fix
round, "Final numbers on the bun PR: cold index 6.3s, warm 0.04s". The drop from 18.5s to 6.3s
cold came with the review-fix round, which among other things rebased and included the
counts-cache key fix. After a local `cargo install` of the final build, the session recorded
"`q pr files 30412` lists all 2,188 files of the 1M-line rewrite PR in 1.4s" (warm metadata,
real cache).

## Promisor packs and the lazy-fetch trap behind numstat

`blob:none` buys its small packs by breaking a foundational invariant: a Git object database is
supposed to be closed under reachability. A tree fetched under the filter references blob OIDs
that simply do not exist locally. The promisor machinery is how Git makes that state legal, and
its behavior under load is the single most important thing to understand about Quinjet's PR
workspace, because it is what PR #49 was written against.

### What a promisor pack is

When a pack arrives from a remote configured with `remote.<name>.promisor = true`, Git writes a
`.promisor` marker file beside the pack's `.pack` and `.idx` in `objects/pack/`. Such a pack is a
*promisor pack*, and its contents carry a guarantee by convention: any object referenced by an
object in a promisor pack, but absent locally, is *promised*, meaning the promisor remote has
committed to serving it on demand later.

Consistency checks are rewritten around that promise. `git fsck`, connectivity checks after
fetches, and reachability walks all treat "missing but promised" as healthy rather than corrupt
(`git rev-list` grows flags like `--exclude-promisor-objects` for tooling that needs to reason
about the boundary). The object database has become an explicit cache of a remote store, rather
than a replica of it.

### The lazy fetch

The other half of the machinery triggers on read. When any code path asks the object database for
an OID and the lookup misses everywhere it can legally miss, and the repository has a promisor
remote, Git does not report corruption. It launches a *lazy fetch*: a special fetch from the
promisor remote asking for exactly the missing OIDs, with negotiation disabled (there is nothing
to negotiate; the OIDs are known), no ref updates, and the filter suppressed so the actual
contents come back. The resulting little pack is itself a promisor pack, and the original read
then proceeds as if the object had always been there.

The full lookup order, which matters shortly, is:

1. Loose objects in `$GIT_DIR/objects`.
1. Every pack (through the midx when present).
1. Each alternate object store listed in `objects/info/alternates`, loose and packed.
1. Only then, for a promisor repository: the lazy fetch over the network.

The elegance is that *no caller had to change*: `git diff`, `git show`, `git blame` all work
unmodified against a partial clone. The danger is the same sentence read again. No caller
changed, so no caller *knows* the cost model changed underneath it. A function that
"just reads two blobs" is now a function that performs zero, one, or two network round trips,
and nothing in its signature says so. Some porcelain commands batch-prefetch the objects they
know they will need (checkout does); many code paths fault objects in as they touch them, one
miss at a time, each miss a fresh fetch process with TLS setup and request latency.

This is category 3 from [Cheap bytes, expensive inflation](#cheap-bytes-expensive-inflation):
inflation silently reclassified from CPU to network.

### The trap sprung: `--numstat` in a blob-less workspace

Quinjet's changed-file index wants two things per file: its status (added, modified, renamed...)
and its `+n -n` line counts, so every header can render honestly before any patch loads
(ARCHITECTURE.md invariant 8a). Status is cheap in a `blob:none` workspace:
`git diff --name-status -z --find-renames <merge_base> <head> --` compares *trees*, which the
filtered fetches did deliver, and emits paths and status letters without opening file contents.

Line counts are the trap. `git diff --numstat` must count added and deleted lines, and there is
no way to count lines without the bytes on both sides: it inflates the old blob and the new blob
of every changed file. In a normal repository that is category-2 work, milliseconds of zlib. In
the promisor workspace, essentially *every one of those blobs is missing by construction*, so the
numstat pass turns into a storm of lazy fetches, serialized inside one uninterruptible `git`
invocation, while the UI can only sit on its "Enumerating changed files" progress step. The
session's failure-mode analysis ranked this the dominant cold-load cost for the bun PR, and it
identified the mechanism precisely: "with a blob:none workspace, the numstat pass forces lazy
network download of essentially every changed blob in one uninterruptible git invocation".

Note the shape of the mistake, because it generalizes: the code did not choose to download every
blob. It chose `--numstat`, an operation that was cheap in every repository the code had been
tested against, and the partial-clone substrate silently attached a network request to each of
the 2,188 files' blob pairs. Promisor repositories invert the usual safety assumption; reads are
no longer free, and every Git invocation must be audited for which objects it will touch.

### The fix, part one (#49): ask GitHub instead of the object store

PR #49 removed the numstat pass from the workspace path entirely. GitHub already knows every
file's counts (its own UI renders them), and exposes them on the pull-request files endpoint.
`pull_request_file_counts_from_api` (`src/git/github/mod.rs:1238-1283`) reads them with:

```text
gh api -i "repos/{owner}/{repo}/pulls/{number}/files?per_page=100&page=N" \
  --jq '.[] | [.filename, (.additions|tostring), (.deletions|tostring), .status] | @tsv'
```

The doc comment above the function (`src/git/github/mod.rs:1235-1237`) states the tradeoff this
page has been building toward: "In the blob-less disposable workspace a local `--numstat` would
download every changed blob just to count lines; GitHub already knows the totals."

Mechanically: pages of 100 records are read up to `MAX_FILE_COUNT_PAGES = 64` pages (6,400
files), continuation decided by the HTTP `Link` header that `gh api -i` exposes; the accumulated
TSV is cached immutably under `pr-file-counts-v3\n{repo_url}\n{number}\n{base}\n{head}` with an
8 MiB limit, written only when complete; and `parse_api_file_counts`
(`src/git/github/mod.rs:1918-1943`) builds the per-path map. Two of its parsing rules encode
review findings:

**1. The cache key names both commits.** The first shipped key omitted the base identity; the
adversarial review showed that a retargeted or reset PR would then serve counts computed against
a different merge base, immutably and forever. The `-v3` key includes base and head, restoring
the property that makes immutable caching sound: the key names the content
(see [caching](../github/caching.md) and ARCHITECTURE.md invariant 12).

**2. Zero-zero records are dropped, except renames.** GitHub reports `additions: 0, deletions:
0` both for genuinely lineless changes and for some huge or binary files it declined to count.
Rather than render a false `+0 -0`, the parser keeps a zero-count record only when the status is
`renamed` (a pure rename honestly has zero changed lines) and otherwise leaves the file's counts
unknown, which the UI renders as a `+·· -··` skeleton until the real patch arrives and backfills
the header (the #55 backfill described in
[progressive loading](../rendering/progressive-loading.md)).

One acknowledged regression rode along, flagged minor in review: the API records carry no binary
indicator, so counts built from them hardcode `binary: false` and the `· binary` suffix that
local numstat parsing produces (`src/git/diff.rs`, `-` in either numstat column) is absent on the
workspace path.

The local `--numstat` did not disappear from the codebase. On the network-free path, where the
opened repository holds both commits and therefore their blobs, `numstat_counts`
(`src/git/github/mod.rs:2094-2120`) still runs `git diff --numstat -z --find-renames`, because
there it is category-2 work and strictly more accurate. The selection is one expression in
`changed_files_in_repository`: `api_counts.unwrap_or_else(|| numstat_counts(...))`. Counts come
from the API exactly when the workspace is the blob-less one.

### The fix, part two (#55): alternates put local objects in front of the network

The lazy fetch is the *last* stop in the object lookup order, and step 3 of that order is the
lever PR #55 pulled. `objects/info/alternates` lets one repository list other object stores to
search before declaring an object missing. `borrow_local_objects`
(`src/git/github/mod.rs:1732-1745`) points the disposable workspace at the opened repository's
store:

```rust
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

The doc comment states the intent: "A merged or locally built pull request usually already has
most of its blobs on disk under other refs, so lazy blob reads resolve from the local store
instead of the network. The opened repository is only read."

Now trace a blob read in the prepared workspace after this one-line file write: loose (miss),
workspace packs (miss, blobs were filtered), *alternate* (hit whenever the user's clone contains
the blob under any ref whatsoever), and only on a full miss the lazy fetch. For a PR that was
merged, or whose files mostly exist in the local history, the storm of category-3 fetches
collapses back into category-2 zlib against the user's own packs. The workspace stays disposable
and the opened repository stays untouched (invariant 9 still holds: an alternates file in the
*workspace* mutates nothing in the source repository, which is "only read").

### The user-visible episode behind it

The mechanism earns its place in the docs because of how it was found. Running the TUI against a
*full* local bun clone, the session's user still saw per-file "Loading diff…" crawls and asked
why anything was slow when everything was local. The diagnosis, confirmed with hard data:

**1. The PR was squash-merged.** bun squash-merged the rewrite PR, so the PR's head commit
`ed1a70f8` exists only on GitHub's `refs/pull/30412/head`, never in `main`. The clone was full;
that one commit was missing.

**2. The OID gate is strict.** Quinjet diffs inside the opened repository only when *both* PR
commits are locally present, because serving a diff from mixed sources would require fetching
into the user's clone, which invariant 9 forbids. One missing commit sent the whole load down
the disposable-workspace path.

**3. Pre-#55, the workspace could not see the local blobs.** Every batch of expanded files was a
lazy blob download from GitHub even though byte-identical blobs sat in packs a few directories
away. The alternates borrow is exactly the bridge; after it, those reads hit the local store.

The session also recorded the manual escape hatch for making the fast path itself apply: a
one-time `git fetch origin +refs/pull/30412/head:refs/remotes/origin/pr-30412` in the user's
clone brings the head commit local, after which `has_commit` passes on both OIDs, the merge base
is computed locally, and every patch is a local `git diff` with no workspace at all.

### Why the promisor trap specifically justifies API metadata

Step back to the general principle, because it is the one this page most wants to teach. A
partial clone re-prices object reads, and the correct response is not "read faster" but "read
less": prefer answers that already exist as metadata over answers recomputed from content.
GitHub's infrastructure has already inflated these blobs once, on its side of the wire, to
produce the counts; fetching thousands of blobs to recompute locally what one paged metadata
endpoint returns in a few requests is strictly wasted motion. The same reasoning powers the
compare-API merge base (one request versus a deepening history download) and, further afield,
the checks and conversation endpoints documented in [API strategy](../github/api-strategy.md).
The technique catalog names this pattern "API-derived metadata over local materialization"
([techniques](../techniques.md)).

## Capped reads: the 8 MiB defense against pack-inflated output

The final Quinjet half concerns the boundary where pack contents become process memory. Every
number the fetch strategy minimized was a *wire* number. But Quinjet consumes Git through
subprocess stdout, and stdout is sized by *inflated* content: a `git diff` over a pathological
file emits however many bytes the unified diff of the inflated blobs happens to be. The
20-byte-delta example earlier cuts both ways; if 20 wire bytes can encode a kilobyte of change,
then a modest pack can encode gigabytes of patch text, and nothing about the fetch's size
predicted it.

### The amplification problem, concretely

Consider the innocent-looking inputs that produce enormous diff output:

**1. A generated or minified file.** A bundler artifact rewritten wholesale diffs as its entire
old content removed plus its entire new content added: output roughly twice the file size, from
a wire delta that may have been kilobytes.

**2. `--unified=1000000`.** Quinjet's own expanded view (`src/git/mod.rs`, `revision_diff_file`)
deliberately requests the whole file as context, so even a one-line change emits the full file.

**3. A huge batch.** One batched invocation over 32 paths (the prefetch shape described below)
emits the sum of 32 patches.

A naive reader that collected stdout into an unbounded buffer would tie its peak memory to
whatever Git felt like emitting. Truncating *after* collection bounds nothing; the allocation
already happened. The only sound design is to stop the producer.

### `run_bounded_command`: kill at the cap

Every Git and gh subprocess in the codebase funnels through one bounded runner,
`run_bounded_command` (`src/git/github/mod.rs:2222-2274`). The core loop:

```rust
let mut collected = Vec::with_capacity(stdout_limit.min(64 * 1024));
let mut buffer = [0_u8; 64 * 1024];
let mut truncated = false;
loop {
    let read = match stdout.read(&mut buffer) {
        Ok(0) => break,
        Ok(read) => read,
        Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
        Err(error) => {
            drop(child.kill());
            drop(child.wait());
            drop(stderr_reader.join());
            return Err(error.into());
        }
    };
    let remaining = stdout_limit.saturating_sub(collected.len());
    if read > remaining {
        collected.extend_from_slice(buffer.get(..remaining).unwrap_or(&buffer));
        truncated = true;
        drop(child.kill());
        break;
    }
    collected.extend_from_slice(buffer.get(..read).unwrap_or(&buffer));
}
```

The properties worth naming:

**1. The child dies at the cap.** The moment a 64 KiB chunk would push the total past
`stdout_limit`, only the remaining allowance is kept, the flag is set, and `child.kill()` fires.
A runaway `git show` costs at most the limit plus one buffer of transfer, ever. The test
`bounded_runner_kills_oversized_git_output` (`src/git/github/mod.rs:3090-3105`) pins this: a
256 KiB blob read under a 1,024-byte cap yields `stdout_truncated` and exactly 1,024 retained
bytes.

**2. stderr cannot deadlock the pipeline.** A separate thread (`read_and_drain`) reads stderr to
EOF, retaining at most its own limit and discarding the excess, so a child that writes a flood
of warnings never blocks on a full stderr pipe while the parent waits on stdout.

**3. Truncation is data, not an error.** The returned `BoundedOutput` carries
`stdout_truncated`, and each caller decides what a cut stream means for its own format. This is
what turns hard caps into graceful degradation instead of failures.

The initial `Vec` capacity is `stdout_limit.min(64 * 1024)`: the buffer grows toward the cap
only if output actually arrives, so a thousand tiny invocations do not each pre-allocate 8 MiB.

### The cap inventory

Each read class carries a limit matched to its worst legitimate payload. The values, all from
`src/git/mod.rs:25-29` and `src/git/github/mod.rs:29-64`:

| Cap | Value | Bounds |
| --- | --- | --- |
| `MAX_DIFF_BYTES` | 8 MiB | any single patch read, including a whole prefetch batch |
| `MAX_DIFF_INDEX_BYTES` | 8 MiB | local `--name-status` / `--numstat` listings |
| `MAX_DIFF_INDEX_FILES` | 16,384 | files parsed into a local diff index |
| `MAX_PR_PATH_BYTES` | 8 MiB | PR name-status and numstat listings, counts cache entries |
| `MAX_PR_PATHS` | 16,384 | files parsed into the PR changed-file index |
| `MAX_GH_METADATA_BYTES` | 2 MiB | default gh stdout and cache entry limit |
| `MAX_GH_ERROR_BYTES` | 256 KiB | stderr kept from a gh (and workspace git) child |
| `MAX_GIT_ERROR_BYTES` | 128 KiB | stderr kept from a local git child |
| fetch stdout (inline) | 128 KiB | diagnostics of a workspace fetch |
| `MAX_CACHED_PATCH_BYTES` | 1 MiB | per-file patch cache ceiling |
| `MAX_CACHE_BYTES` / `MAX_CACHE_ENTRIES` | 128 MiB / 2,048 | whole on-disk cache, pruned oldest first |

ARCHITECTURE.md states the design rule behind the table twice over: invariant 5 fixes the
numbers ("Git/PR patch output is capped at 8 MiB per read. Exact PR/check metadata is capped at
2 MiB, changed-file indexes at 8 MiB / 16,384 entries...") and invariant 6 fixes the mechanism
("Crossing a cap kills the child rather than first allocating all output and truncating
afterward").

### Truncation repair: cutting to a whole record

A killed child leaves its stream cut at an arbitrary byte, possibly mid-record. Each format
Quinjet parses has a one-line repair that restores the invariant its parser needs:

**1. Line-oriented patches pop back to the last newline.** `diff_selected_paths`
(`src/git/github/mod.rs:2141-2173`) generates every PR patch and repairs its own truncation:

```rust
let output = run_repository_git(repository, &args, MAX_DIFF_BYTES, MAX_GH_ERROR_BYTES)?;
if !output.status.success() && !output.stdout_truncated {
    bail!(
        "{}",
        bounded_command_error("unable to generate the local pull-request diff", &output)
    );
}
let mut patch = output.stdout;
if output.stdout_truncated {
    while patch.last().is_some_and(|byte| *byte != b'\n') {
        let _ = patch.pop();
    }
}
Ok((patch, output.stdout_truncated))
```

Note the status check: a killed child reports a non-success exit, so "failed *because we killed
it*" is distinguished from "failed on its own" by consulting `stdout_truncated`. The former is
success-with-truncation; the latter is a real error with stderr context. The popped-to-newline
patch parses cleanly, with the loss confined to the tail and the resulting document flagged
truncated so the UI can say so.

**2. NUL-record listings cut at the last NUL.** The `-z` streams (`--name-status -z`,
`--numstat -z`) are cut back to the last `\0` (`src/git/github/mod.rs:2019-2030` for the PR
index, `src/git/mod.rs:525-531` for the local one), so a truncated listing still parses as whole
records. A rename record is three consecutive NUL fields (status, old path, new path); the
repair also detects a record run that ends mid-file and marks the index truncated rather than
inventing a half-parsed entry.

**3. Honesty survives the cut.** A truncated PR index sets `total_files =
pull_request.changed_files.max(files.len())` (`src/git/github/mod.rs:806-810`), so the header
count falls back to GitHub's own number instead of quietly reporting only what fit under the
cap. Truncated patches are never written into the patch cache, and a truncated counts
accumulation is never cached, so a cap can cost completeness for one read but can never poison
a future one.

### Budgeting under the cap: the prefetch estimate

The 8 MiB patch cap interacts with the one reader that deliberately asks for a lot at once:
background prefetch, which batches many files into a single `git diff` (one process spawn per
*batch* instead of per file, the economics described in [prefetch](../github/prefetch.md) and
invariant 10a). A batch whose combined patch crosses 8 MiB gets killed and loses its tail, so
the scheduler's job is to size batches that will not cross it, using only information that is
free: the per-file counts the index already holds.

`estimated_patch_bytes` (`src/app.rs:7052-7060`) is the whole cost model:

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

In words: 80 bytes per changed line (`PULL_REQUEST_PATCH_LINE_ESTIMATE`), plus a 4,096-byte
floor per file for the `diff --git` header, hunk headers, and context lines; a file with no
counts at all is assumed to be `PULL_REQUEST_PATCH_FALLBACK_ESTIMATE` = 512 KiB, pessimistic
enough that a batch cannot accidentally stack many unknowns. This estimate is the quiet payoff
of the counts work in #49: exact per-file counts were fetched up front *precisely so that this
function has real inputs* on the workspace path.

Batch assembly (`request_pull_request_prefetch`, `src/app.rs:5930-5977`) then packs files into a
batch until either `PULL_REQUEST_PREFETCH_BATCH` = 32 files or a running total that would pass
`PULL_REQUEST_PREFETCH_BYTE_BUDGET` = 6 MiB, whichever comes first. A worked packing, with
counts as a file index might hold them:

```text
file                     +/- counts   estimate = (a+d)*80 + 4096
----------------------   ----------   --------------------------
src/js/parser.zig        1,204 / 977  178,576
src/js/lexer.zig           410 / 388   67,936
docs/big-notes.md        9,800 / 12   789,056
assets/schema.json       (unknown)    524,288   (512 KiB fallback)
src/small-1.rs                8 / 3    4,976
...                      ...          ...

running total after each file stays <= 6,291,456 (6 MiB);
first file whose estimate would cross it ends the batch,
UNLESS the batch is still empty: a single file estimated
over the whole budget travels alone.
```

The 2 MiB of headroom between the 6 MiB estimate budget and the 8 MiB kill cap absorbs estimate
error: long lines (the 80-byte guess is an average, not a bound), context lines around dense
hunks, and rename headers. The budget keeps a batch's real patch comfortably under the
truncation cap in the ordinary case, and when reality still exceeds it, the truncation-repair
rules above make the failure partial and retryable rather than silent.

### When the estimate cannot see the elephant

The estimate has a documented blind spot, found by the adversarial review: line counts measure
*lines*, and a pathological file can hide arbitrary bytes in one line. The review's concrete
scenario: an added minified bundle written as a single 10 MB line has `additions = 1`, so its
estimate is 80 + 4,096 bytes, the byte budget waves it through, and the real batch patch blows
the 8 MiB cap inside that file's section.

The original code turned this into a livelock. `diff_files`
(`src/git/github/mod.rs:440-517`) splits the combined patch at `diff --git` boundaries
(`split_patch_by_file`, `src/git/diff.rs:618-663`) and, by construction, only the *last* section
of a truncated batch can be incomplete; a truncated non-final situation cannot be told apart
from a missing file. Pre-fix, a batch whose *first and only emitted* section was cut returned
zero documents, nothing was cached, and the scheduler immediately re-dispatched the identical
batch: "the app re-runs the identical 8 MB git diff in a tight worker loop forever." The fix
keeps exactly one truncated fallback document when a truncated batch would otherwise produce
nothing (`src/git/github/mod.rs:511-515`), so the enormous file renders its truncated head, is
recorded as handled, and the walk moves on. A truncated middle-of-batch file is instead withheld
and retried alone in a later batch, where the single-file read gives it the full 8 MiB to
itself.

### Prefetch ordering: #50's size tiers, then #55's viewport anchor

How the walk chooses *which* files to batch changed once during the stack, and the evolution is
worth recording because both steps were principled.

**The superseded step: smallest files first.** PR #50 ("perf: prefetch smallest files first on
huge pull requests") introduced `HUGE_PULL_REQUEST_LINES` = 100,000 and
`HUGE_PULL_REQUEST_FILES` = 1,000; when a PR's total changed lines or file count crossed those
thresholds, prefetch candidates were sorted by `estimated_patch_bytes` ascending, so the byte
budget filled with the greatest possible *number* of completed files per batch. On a
million-line PR this maximized how much of the file tree lit up early, at the cost that the
files the reader was actually looking at, if large, loaded last. At the time the prefetch walk
also stopped at 400 files total, so coverage was the scarce resource being optimized.

**The current behavior: start where the reader is looking.** PR #55 removed both `HUGE_`
constants and the sort, raised `MAX_PREFETCHED_PULL_REQUEST_FILES` from 400 to 4,096 (the whole
2,188-file bun index became prefetchable), and anchored the walk to the viewport.
`prefetch_anchor_index` (`src/app.rs:5912-5925`) finds the first file visible in the Files
tree, and the batch loop iterates `from_anchor.iter().chain(before.iter())`, wrapping around the
index. The doc comment over the anchor states the priority inversion plainly: "Where background
fill should start: the first file visible in the Files tree, so patches land where the reader is
looking and then wrap around the rest of the index in order." Once the cap stopped being the
scarce resource, latency-to-visible-content beat completed-file count, and the size tiers were
retired as superseded rather than layered under the anchor. The full progressive-loading design
is on [its own page](../rendering/progressive-loading.md).

Both orderings respected the same byte discipline; only the priority function changed. The
final wording of ARCHITECTURE.md invariant 5 records the current contract: "Background prefetch
walks the whole index up to 4,096 files, starting at the file the Files tree is showing and
wrapping around the rest in order, sizes each batch by per-file count estimates to stay under
the 8 MiB patch read, and backfills a header's counts from its arrived patch when GitHub could
not report them."

### Memory budgets past the pipe

Two more bounds complete the defense in depth, because surviving the pipe is not the end of a
patch's memory life:

**1. Parsed documents are budgeted.** Prefetched documents are evicted against
`MAX_PULL_REQUEST_DOCUMENT_BYTES` = 32 MiB of measured in-memory size (`diff_document_size`,
`src/app.rs:7062`), so holding a huge PR open cannot grow the heap without bound even though
every individual read was capped.

**2. The disk cache has its own ceilings.** A single file's patch is cached only under
`MAX_CACHED_PATCH_BYTES` = 1 MiB, "so one file cannot crowd out the rest of a pull request"
(`src/git/github/mod.rs:40-42`), and the whole cache prunes oldest-first past 128 MiB or 2,048
entries. An oversized cache file found on disk is deleted on sight during reads, which is how
the store self-heals when limits shrink between versions.

The chain of custody for one patch is therefore bounded at every hop: the server built a pack
the fetch flags kept small; the pipe read kept inflation under 8 MiB; the parse kept documents
under 32 MiB total; and the cache kept the durable copy under 1 MiB or not at all.

## A worked end-to-end example: opening bun#30412

Everything above assembles into one traceable sequence. This section follows a cold
`quinjet pr files 30412` run against the benchmark clone at `/tmp/bun-test` (the 389 MB shallow
`blob:none` clone of `oven-sh/bun` described earlier), with the cache pointed at a throwaway
root the way the session benchmarked it:

```bash
QUINJET_CACHE_DIR=$(mktemp -d) quinjet pr files 30412
```

The subject is the "Rewrite Bun in Rust" pull request: 2,188 changed files, +1,009,257 added
lines. The cold timings quoted at each phase are the session's, with their context restated.

### Phase 1: metadata, one TSV record

The lookup resolves the repository identity from the clone's remotes (offline for a github.com
URL), then runs `gh pr view 30412 --repo <url> --json <18 fields> --jq <TSV>` under the 2 MiB
metadata cap. One TSV record comes back carrying, among its 18 fields, the four values the rest
of the pipeline is built on: `baseRefOid`, `headRefOid`, `additions`, `deletions`, and
`changedFiles`. The session's first verification round recorded this step as "Metadata in 1.7s"
(`pr view` against bun#30412, cold). Cached under `pull-request-v3\n{url}\n30412` with a
five-minute TTL; every later phase keys off the immutable OID pair instead.

### Phase 2: the local probe

`prepare_pull_request_diff` issues the two `git cat-file -e <oid>^{commit}` probes against the
opened repository. Each probe is an index lookup as described in
[the pack index section](#the-pack-index-fan-out-and-binary-search): fan-out bracket, binary
search, exit code. bun squash-merged this PR, so its head commit lives on GitHub's
`refs/pull/30412/head` rather than in `main`'s history; when the probe for the head OID misses,
the whole load takes the disposable-workspace path, and the cold benchmark numbers below
include that workspace prepare.

### Phase 3: the API hints

Before any fetch, two metadata questions are asked and cached forever against the OID pair:

**1. The merge base.** One compare-API call returns `merge_base_commit.sha`. This replaces the
deepening ladder in the common case: no base-branch history will be fetched at all.

**2. Per-file counts.** The pulls files endpoint is paged at 100 records per page; 2,188 files
is 22 pages, well under the 64-page cap. Each page is one `gh api -i` call whose `Link` header
drives continuation. The result is the complete `(path, additions, deletions, status)` table
that phase 6 will attach to the index and that the prefetch estimator will consume, obtained
without inflating a single blob.

### Phase 4: the workspace

`TemporaryBareRepository::new` creates `<cache_root>/tmp/pr-<pid>-<counter>.git` with
`git init --bare --quiet` (0700 directory; leaked siblings older than 24 hours are swept
first). `borrow_local_objects` writes one line into `objects/info/alternates`: the path of
`/tmp/bun-test/.git/objects`. From this moment, every object lookup inside the workspace can
see the 389 MB of commits and trees (and any previously lazy-fetched blobs) that the benchmark
clone already holds, ahead of any network fallback.

### Phase 5: two small fetches

**1. The head, at depth 64.** `git fetch --quiet --force --no-tags --filter=blob:none
--depth=64 origin +refs/pull/30412/head:refs/quinjet/head`. The wire pack holds at most 64
commits and their trees; blobs are filtered; thin deltas against whatever negotiation finds
common shrink it further.

**2. The merge base, at depth 1.** The hint refspec `+<sha>:refs/quinjet/merge-base` fetched at
`--depth=1` delivers the single-commit pack laid out
[earlier](#small-packs-by-construction-depth-1-and-blobnone): one commit object, its trees,
nothing else. `preferred_fetched_commit` then confirms the fetched head still equals the
metadata's `headRefOid`; it does, so the function returns `(hint, head)` and the base branch's
history is never fetched. The `FetchingBase` progress state is skipped entirely on this run.

### Phase 6: enumeration without blobs

`git diff --name-status -z --find-renames <merge_base> <head> --` runs in the workspace under
the 8 MiB / 16,384-entry caps. This is a tree-to-tree comparison: Git walks the two root trees
(fetched in phase 5), recursing only into subtrees whose OIDs differ, and emits status letters
and paths. No blob contents are needed for adds, deletes, and modifications, and exact renames
match by blob OID equality alone. The 2,188 records come back as NUL-separated bytes, get the
phase-3 counts attached per path, and the raw listing is cached immutably under
`pr-files-v1\n<merge_base>\n<head>`.

This phase is where the pre-#49 design paid its blob storm: the old `--numstat` companion pass
would have lazy-fetched essentially every changed blob right here, inside one uninterruptible
invocation. On the fixed code the enumeration completes with zero blob reads, and the first
verification round measured the whole cold path to this point, phases 1 through 6, as: "The
rewrite PR enumerates all 2,188 files with real counts in 18.5s cold." After the review-fix
round the final binary brought it to "cold index 6.3s, warm 0.04s".

### Phase 7: patches, on demand and in the background

The prepared workspace now serves patch requests. In the TUI, the viewport-anchored prefetch
walks the index from the first visible file, packing batches of up to 32 files under the 6 MiB
estimate budget; 2,188 files means at least 69 batched `git diff` invocations if every batch
reached 32 files, each batch's combined patch split back into per-file documents at its
`diff --git` boundaries. Every blob read inside those diffs consults the alternates first;
contents that exist anywhere in the benchmark clone's packs resolve locally, and only truly
GitHub-only blobs trigger lazy promisor fetches, batched naturally by the multi-path
invocation. A directly selected file jumps the queue through the preview lane with the full
8 MiB cap to itself; the session measured "Single-file patches: 0.1s." on the cold run.
Complete per-file patches at or under 1 MiB are written through to the immutable
`pr-patch-v1\n<mb>\n<head>\n<path>` cache as a side effect.

### Phase 8: the warm reopen

Close and reopen the PR. Metadata may still be inside its five-minute TTL; the merge base, the
counts table, the file listing, and every cached patch are immutable entries keyed by OIDs that
have not changed, so they are served from disk without a subprocess. The measured warm index
time was 0.04s, and the post-install smoke test ("`q pr files 30412` lists all 2,188 files of
the 1M-line rewrite PR in 1.4s", warm metadata, real cache) shows the steady state a user
actually lives in. The only work a warm open repeats on the workspace path is the workspace
prepare itself, because a dropped `TemporaryBareRepository` deletes its packs; the immutable
caches in front of it are what make that acceptable
(see [caching](../github/caching.md) for the full key inventory).

### The phase table

| Phase | Dominant pack mechanism | Bytes bounded by |
| --- | --- | --- |
| 1 metadata | none (API metadata) | 2 MiB gh cap |
| 2 probe | idx fan-out lookup, no inflation | exit code only |
| 3 hints | server-side bitmaps answer compare | 2 MiB / 8 MiB caps |
| 4 workspace | alternates in front of promisor | one path written |
| 5 fetches | depth + filter + thin packs | 128 KiB diagnostics |
| 6 enumeration | tree walk, zero blob inflation | 8 MiB / 16,384 entries |
| 7 patches | chain resolution + lazy fetch, alternates first | 8 MiB per read |
| 8 warm | no packs touched at all | cache ceilings |

## Design alternatives and why they lost

Each alternative below was either explicitly considered during the optimization stack or is the
obvious road not taken; each lost to the shipped design for reasons that follow from pack
mechanics.

**1. Linking a Git library instead of spawning subprocesses.** An in-process `libgit2` or
`gitoxide` would remove process-spawn overhead and give structured answers without byte
parsing. It lost on authority and surface: Quinjet treats the `git` binary as the single source
of truth for repository semantics (hooks, config, filters, partial clone, promisor behavior all
included), and partial-clone plus promisor lazy-fetch semantics in particular are exactly the
kind of evolving machinery where shelling out inherits every upstream fix for free. The
subprocess costs that matter were bounded instead: one process per *batch* of patches, capped
pipes, and coalescing mailboxes upstream (see [concurrency](../rendering/concurrency.md)).

**2. Cloning the PR into the workspace without filters.** A plain shallow fetch (no
`--filter`) would make every later read local and fast. It lost on the first number that
matters: transfer. The whole reason the disposable workspace is viable per-PR is that commits
plus trees are small; blobs are where repositories keep their mass. A blobby fetch of a
million-line PR would move that mass up front for files the reader may never open, when the
progressive design needs only the opened files' blobs, later, lazily, and often from the
alternates. The filter fallback path (retry without `blob:none`) exists for servers that force
the blobby world, but it is the degraded case, not the design point.

**3. Computing the merge base locally as the primary strategy.** The pre-stack code did this,
with a deepening ladder capped at 4,096 commits, and it hard-failed on long-lived rewrite
branches while wasting up to eight progressively deeper fetches. It lost to the compare API
because a server that maintains commit-graphs and bitmaps can answer the ancestor question in
milliseconds, whereas a shallow client cannot even use a commit-graph and must download history
just to look at it. The ladder survives as the fallback, extended to 16,384, with the
force-push guard keeping the hint honest.

**4. Keeping the local `--numstat` on the workspace path.** Numstat's counts are exact,
including binary detection. It lost because in a promisor workspace its cost is not CPU but a
per-blob network fetch storm, the single dominant cold-load cost the stack removed. The
accepted price: GitHub occasionally reports 0/0 for huge files (rendered as `+·· -··` skeletons
and backfilled from the patch when it arrives) and the `· binary` label is absent on the
workspace path. The local path kept numstat, where it is both exact and cheap.

**5. Fetching PR refs into the opened repository.** One fetch into the user's clone
(`refs/pull/N/head`) would make every PR a local diff and dissolve the workspace entirely, and
it is precisely what the session recommended *the user* do manually for a repeatedly revisited
PR. Quinjet doing it silently lost to invariant 9: the opened repository receives no ref
mutation, ever. A tool that quietly grows `refs/quinjet/*` or remote-tracking refs in the
user's repository changes `git fetch --prune` behavior, ref listings, and disk usage behind the
user's back. The alternates borrow is the inversion that preserves the invariant: instead of
writing into the user's store, the workspace reads from it.

**6. Reading subprocess output whole, then truncating.** Simpler code, and the common library
default. It lost because the bound must hold at the allocator, not after it: a 10 GiB inflated
diff must never exist in memory even momentarily. Kill-at-cap also stops the child's *work*,
not just its output; a killed `git diff` stops inflating chains and stops issuing lazy fetches,
which collect-then-truncate would let run to completion.

**7. Layering smallest-first ordering under the viewport anchor.** After #55, the #50 tiers
could have been kept for the wrapped-around remainder of the walk. They were removed instead:
the anchor already guarantees the reader's visible files come first, the raised 4,096-file cap
means the walk covers the bun-scale index anyway, and a second ordering rule would have made
batch composition depend on scroll position *and* size simultaneously, which is harder to
reason about and to test for no observed win. Superseded code that no longer buys anything is
deleted, not preserved (the #50 commit remains in history).

**8. Caching patches of any size.** The 1 MiB per-file cache ceiling means the biggest patches,
which cost the most to regenerate, are exactly the ones never cached. The alternative lost to
arithmetic: under the 128 MiB / 2,048-entry cache budget, a handful of 8 MiB patches would
evict hundreds of small entries (the analysis that set the ceiling: one file must not "crowd
out the rest of a pull request"). Big patches are regenerable from the workspace at a bounded
0.1s-scale cost, while the cache's value density lives in the long tail of small files.

## Failure modes and edge cases

**1. The server refuses filters.** `uploadpack.allowFilter` is not universal. `fetch_ref`
falls back to the identical fetch without `--filter=blob:none`; depth still bounds the
transfer. The workspace then holds blobs it did not strictly need, but every downstream read
gets cheaper, and correctness is unaffected. Only a second failure is an error.

**2. A force-push lands mid-open.** The metadata snapshot's OIDs and the refs' current tips can
diverge between phases. Two mechanisms pin the result: `preferred_fetched_commit` resolves the
advertised OIDs in preference to ref tips, and the merge-base hint is used only when the
fetched head equals the snapshot head. The diff shown is the diff of the metadata the user was
shown, and the immutable caches are keyed by that same OID pair, so a racing push can cost a
retry but never a wrong cached answer.

**3. The fork is gone.** For a cross-repository PR whose fork was deleted, GitHub's
`refs/pull/N/head` on the base repository usually still serves the head. When even that fails
and there is no fork to fall back to, the error is contextualized ("the base repository no
longer exposes the PR head and its fork was deleted") rather than a raw fetch failure.

**4. The merge base is farther than 16,384 commits.** The ladder refuses to continue:
"Unable to find the PR merge base within 16,384 commits; refusing an unbounded history fetch".
This is a deliberate hard edge; past that divergence, the transfer being avoided is
approximately a full clone, and the API hint path is the intended answer for such branches.

**5. The listing outgrows its caps.** Past 8 MiB of `--name-status -z` bytes or 16,384 parsed
entries, the stream is cut to the last NUL, the index is marked truncated, and the displayed
total falls back to `max(GitHub's changedFiles, parsed count)`. The bun PR's 2,188 files sit
comfortably inside both caps; index chunking past 16,384 entries was considered during the
session and consciously deferred.

**6. Inexact rename detection touches blobs.** A general-theory caveat rather than an observed
incident: `--find-renames` matches exact renames by OID equality, but scoring *inexact* rename
candidates compares contents, and content reads on the promisor path can lazy-fetch. The
candidate set is limited to delete/add pairs within the diff, so the exposure is bounded by the
shape of the change, not by the size of the PR; it is the one place blob reads can precede
patch generation on the workspace path.

**7. A crash leaks a workspace.** `Drop` cannot run if the process dies. The next workspace
creation sweeps siblings matching `pr-*.git` older than 24 hours (scanning at most 256
directory entries), so leaked bare repositories cost at most a day of disk.

**8. GitHub reports 0/0 for a real change.** Some huge generated files come back from the files
endpoint with zero counts. The parser refuses to present `+0 -0` for them (only `renamed`
records legitimately keep zeros); their headers show count skeletons until the arrived patch
backfills real numbers. This was tuned twice: the first fix dropped all 0/0 records, which
wrongly hid pure renames' honest zeros, and #55 restored the rename exception.

**9. The alternate store is imperfect.** `borrow_local_objects` is best-effort by design: a
missing common dir, a non-directory objects path, or a failed write all silently skip the
borrow. Correctness never depended on it; every miss simply falls through to the lazy fetch
that would have run anyway. The same holds if the user relocates or prunes their repository
mid-session: alternates are consulted per lookup, and a vanished alternate degrades to
network, not to corruption.

**10. No cache root resolves.** When `QUINJET_CACHE_DIR` and every platform default fail,
caching silently disables (every helper is best-effort), the workspace parent falls back to the
system temp directory, and the whole pipeline runs correctly with only the performance profile
of a permanently cold cache.

## Related pages

- [Git internals overview](./README.md): the group hub and reading order.
- [The object model](./object-model.md): loose objects, hashing, and why OIDs are immutable
  cache keys.
- [Shallow and partial clone](./shallow-and-partial-clone.md): protocol v2, negotiation, filters,
  and the fetch ladder in protocol terms.
- [Merge bases and history](./merge-bases-and-history.md): the DAG theory behind the compare-API
  resolution and the deepening fallback.
- [Plumbing and porcelain](./plumbing-and-porcelain.md): the full catalog of Git invocations and
  their byte-oriented parsers.
- [Refs, index, and worktrees](./refs-index-and-worktrees.md): the non-object half of the
  repository Quinjet reads.
- [The PR workspace](../github/pr-workspace.md): the lifecycle of the disposable bare repository
  this page's fetches populate.
- [API strategy](../github/api-strategy.md): the GitHub endpoints that substitute metadata for
  materialization.
- [Prefetch](../github/prefetch.md): the batch scheduler that spends the 8 MiB budget.
- [Caching](../github/caching.md): the immutable-versus-TTL key design the OID pairs enable.
- [The diff pipeline](../diff/pipeline.md): what happens to patch bytes after they survive the
  capped pipe.
- [Progressive loading](../rendering/progressive-loading.md): the #55 viewport-first design in
  full.
- [Benchmarking](../benchmarking.md): the bun#30412 methodology and every measured number in
  context.
- [Techniques](../techniques.md): the catalog entry for each pattern this page grounds.

## Optimization review matrix

Use this matrix during performance reviews. Each row combines a cost lens, repository context, and observable signal without claiming that every combination needs a standalone benchmark.

| ID | Review condition | Evidence to capture |
| ---: | --- | --- |
| 1 | Check latency for Packfiles and Deltas in a small local repository | Record time to first useful rows |
| 2 | Check latency for Packfiles and Deltas in a small local repository | Record steady frame cost |
| 3 | Check latency for Packfiles and Deltas in a small local repository | Record bytes accepted from child output |
| 4 | Check latency for Packfiles and Deltas in a small local repository | Record Git and gh process count |
| 5 | Check latency for Packfiles and Deltas in a small local repository | Record maximum retained document bytes |
| 6 | Check latency for Packfiles and Deltas in a small local repository | Record cache disposition and complete key |
| 7 | Check latency for Packfiles and Deltas in a small local repository | Record stale reply rejection |
| 8 | Check latency for Packfiles and Deltas in a small local repository | Record visible state after failure |
| 9 | Check latency for Packfiles and Deltas in a monorepo with many changed paths | Record time to first useful rows |
| 10 | Check latency for Packfiles and Deltas in a monorepo with many changed paths | Record steady frame cost |
| 11 | Check latency for Packfiles and Deltas in a monorepo with many changed paths | Record bytes accepted from child output |
| 12 | Check latency for Packfiles and Deltas in a monorepo with many changed paths | Record Git and gh process count |
| 13 | Check latency for Packfiles and Deltas in a monorepo with many changed paths | Record maximum retained document bytes |
| 14 | Check latency for Packfiles and Deltas in a monorepo with many changed paths | Record cache disposition and complete key |
| 15 | Check latency for Packfiles and Deltas in a monorepo with many changed paths | Record stale reply rejection |
| 16 | Check latency for Packfiles and Deltas in a monorepo with many changed paths | Record visible state after failure |
| 17 | Check latency for Packfiles and Deltas in a pull request containing generated files | Record time to first useful rows |
| 18 | Check latency for Packfiles and Deltas in a pull request containing generated files | Record steady frame cost |
| 19 | Check latency for Packfiles and Deltas in a pull request containing generated files | Record bytes accepted from child output |
| 20 | Check latency for Packfiles and Deltas in a pull request containing generated files | Record Git and gh process count |
| 21 | Check latency for Packfiles and Deltas in a pull request containing generated files | Record maximum retained document bytes |
| 22 | Check latency for Packfiles and Deltas in a pull request containing generated files | Record cache disposition and complete key |
| 23 | Check latency for Packfiles and Deltas in a pull request containing generated files | Record stale reply rejection |
| 24 | Check latency for Packfiles and Deltas in a pull request containing generated files | Record visible state after failure |
| 25 | Check latency for Packfiles and Deltas in a deeply diverged branch | Record time to first useful rows |
| 26 | Check latency for Packfiles and Deltas in a deeply diverged branch | Record steady frame cost |
| 27 | Check latency for Packfiles and Deltas in a deeply diverged branch | Record bytes accepted from child output |
| 28 | Check latency for Packfiles and Deltas in a deeply diverged branch | Record Git and gh process count |
| 29 | Check latency for Packfiles and Deltas in a deeply diverged branch | Record maximum retained document bytes |
| 30 | Check latency for Packfiles and Deltas in a deeply diverged branch | Record cache disposition and complete key |
| 31 | Check latency for Packfiles and Deltas in a deeply diverged branch | Record stale reply rejection |
| 32 | Check latency for Packfiles and Deltas in a deeply diverged branch | Record visible state after failure |
| 33 | Check latency for Packfiles and Deltas in an unavailable network | Record time to first useful rows |
| 34 | Check latency for Packfiles and Deltas in an unavailable network | Record steady frame cost |
| 35 | Check latency for Packfiles and Deltas in an unavailable network | Record bytes accepted from child output |
| 36 | Check latency for Packfiles and Deltas in an unavailable network | Record Git and gh process count |
| 37 | Check latency for Packfiles and Deltas in an unavailable network | Record maximum retained document bytes |
| 38 | Check latency for Packfiles and Deltas in an unavailable network | Record cache disposition and complete key |
| 39 | Check latency for Packfiles and Deltas in an unavailable network | Record stale reply rejection |
| 40 | Check latency for Packfiles and Deltas in an unavailable network | Record visible state after failure |
| 41 | Check latency for Packfiles and Deltas in rapid keyboard navigation | Record time to first useful rows |
| 42 | Check latency for Packfiles and Deltas in rapid keyboard navigation | Record steady frame cost |
| 43 | Check latency for Packfiles and Deltas in rapid keyboard navigation | Record bytes accepted from child output |
| 44 | Check latency for Packfiles and Deltas in rapid keyboard navigation | Record Git and gh process count |
| 45 | Check latency for Packfiles and Deltas in rapid keyboard navigation | Record maximum retained document bytes |
| 46 | Check latency for Packfiles and Deltas in rapid keyboard navigation | Record cache disposition and complete key |
| 47 | Check latency for Packfiles and Deltas in rapid keyboard navigation | Record stale reply rejection |
| 48 | Check latency for Packfiles and Deltas in rapid keyboard navigation | Record visible state after failure |
| 49 | Check latency for Packfiles and Deltas in a linked worktree | Record time to first useful rows |
| 50 | Check latency for Packfiles and Deltas in a linked worktree | Record steady frame cost |
| 51 | Check latency for Packfiles and Deltas in a linked worktree | Record bytes accepted from child output |
| 52 | Check latency for Packfiles and Deltas in a linked worktree | Record Git and gh process count |
| 53 | Check latency for Packfiles and Deltas in a linked worktree | Record maximum retained document bytes |
| 54 | Check latency for Packfiles and Deltas in a linked worktree | Record cache disposition and complete key |
| 55 | Check latency for Packfiles and Deltas in a linked worktree | Record stale reply rejection |
| 56 | Check latency for Packfiles and Deltas in a linked worktree | Record visible state after failure |
| 57 | Check latency for Packfiles and Deltas in cold and warm cache states | Record time to first useful rows |
| 58 | Check latency for Packfiles and Deltas in cold and warm cache states | Record steady frame cost |
| 59 | Check latency for Packfiles and Deltas in cold and warm cache states | Record bytes accepted from child output |
| 60 | Check latency for Packfiles and Deltas in cold and warm cache states | Record Git and gh process count |
| 61 | Check latency for Packfiles and Deltas in cold and warm cache states | Record maximum retained document bytes |
| 62 | Check latency for Packfiles and Deltas in cold and warm cache states | Record cache disposition and complete key |
| 63 | Check latency for Packfiles and Deltas in cold and warm cache states | Record stale reply rejection |
| 64 | Check latency for Packfiles and Deltas in cold and warm cache states | Record visible state after failure |
| 65 | Check peak memory for Packfiles and Deltas in a small local repository | Record time to first useful rows |
| 66 | Check peak memory for Packfiles and Deltas in a small local repository | Record steady frame cost |
| 67 | Check peak memory for Packfiles and Deltas in a small local repository | Record bytes accepted from child output |
| 68 | Check peak memory for Packfiles and Deltas in a small local repository | Record Git and gh process count |
| 69 | Check peak memory for Packfiles and Deltas in a small local repository | Record maximum retained document bytes |
| 70 | Check peak memory for Packfiles and Deltas in a small local repository | Record cache disposition and complete key |
| 71 | Check peak memory for Packfiles and Deltas in a small local repository | Record stale reply rejection |
| 72 | Check peak memory for Packfiles and Deltas in a small local repository | Record visible state after failure |
| 73 | Check peak memory for Packfiles and Deltas in a monorepo with many changed paths | Record time to first useful rows |
| 74 | Check peak memory for Packfiles and Deltas in a monorepo with many changed paths | Record steady frame cost |
| 75 | Check peak memory for Packfiles and Deltas in a monorepo with many changed paths | Record bytes accepted from child output |
| 76 | Check peak memory for Packfiles and Deltas in a monorepo with many changed paths | Record Git and gh process count |
| 77 | Check peak memory for Packfiles and Deltas in a monorepo with many changed paths | Record maximum retained document bytes |
| 78 | Check peak memory for Packfiles and Deltas in a monorepo with many changed paths | Record cache disposition and complete key |
| 79 | Check peak memory for Packfiles and Deltas in a monorepo with many changed paths | Record stale reply rejection |
| 80 | Check peak memory for Packfiles and Deltas in a monorepo with many changed paths | Record visible state after failure |
| 81 | Check peak memory for Packfiles and Deltas in a pull request containing generated files | Record time to first useful rows |
| 82 | Check peak memory for Packfiles and Deltas in a pull request containing generated files | Record steady frame cost |
| 83 | Check peak memory for Packfiles and Deltas in a pull request containing generated files | Record bytes accepted from child output |
| 84 | Check peak memory for Packfiles and Deltas in a pull request containing generated files | Record Git and gh process count |
| 85 | Check peak memory for Packfiles and Deltas in a pull request containing generated files | Record maximum retained document bytes |
| 86 | Check peak memory for Packfiles and Deltas in a pull request containing generated files | Record cache disposition and complete key |
| 87 | Check peak memory for Packfiles and Deltas in a pull request containing generated files | Record stale reply rejection |
| 88 | Check peak memory for Packfiles and Deltas in a pull request containing generated files | Record visible state after failure |
| 89 | Check peak memory for Packfiles and Deltas in a deeply diverged branch | Record time to first useful rows |
| 90 | Check peak memory for Packfiles and Deltas in a deeply diverged branch | Record steady frame cost |
| 91 | Check peak memory for Packfiles and Deltas in a deeply diverged branch | Record bytes accepted from child output |
| 92 | Check peak memory for Packfiles and Deltas in a deeply diverged branch | Record Git and gh process count |
| 93 | Check peak memory for Packfiles and Deltas in a deeply diverged branch | Record maximum retained document bytes |
| 94 | Check peak memory for Packfiles and Deltas in a deeply diverged branch | Record cache disposition and complete key |
| 95 | Check peak memory for Packfiles and Deltas in a deeply diverged branch | Record stale reply rejection |
| 96 | Check peak memory for Packfiles and Deltas in a deeply diverged branch | Record visible state after failure |
| 97 | Check peak memory for Packfiles and Deltas in an unavailable network | Record time to first useful rows |
| 98 | Check peak memory for Packfiles and Deltas in an unavailable network | Record steady frame cost |
| 99 | Check peak memory for Packfiles and Deltas in an unavailable network | Record bytes accepted from child output |
| 100 | Check peak memory for Packfiles and Deltas in an unavailable network | Record Git and gh process count |
| 101 | Check peak memory for Packfiles and Deltas in an unavailable network | Record maximum retained document bytes |
| 102 | Check peak memory for Packfiles and Deltas in an unavailable network | Record cache disposition and complete key |
| 103 | Check peak memory for Packfiles and Deltas in an unavailable network | Record stale reply rejection |
| 104 | Check peak memory for Packfiles and Deltas in an unavailable network | Record visible state after failure |
| 105 | Check peak memory for Packfiles and Deltas in rapid keyboard navigation | Record time to first useful rows |
| 106 | Check peak memory for Packfiles and Deltas in rapid keyboard navigation | Record steady frame cost |
| 107 | Check peak memory for Packfiles and Deltas in rapid keyboard navigation | Record bytes accepted from child output |
| 108 | Check peak memory for Packfiles and Deltas in rapid keyboard navigation | Record Git and gh process count |
| 109 | Check peak memory for Packfiles and Deltas in rapid keyboard navigation | Record maximum retained document bytes |
| 110 | Check peak memory for Packfiles and Deltas in rapid keyboard navigation | Record cache disposition and complete key |
| 111 | Check peak memory for Packfiles and Deltas in rapid keyboard navigation | Record stale reply rejection |
| 112 | Check peak memory for Packfiles and Deltas in rapid keyboard navigation | Record visible state after failure |
| 113 | Check peak memory for Packfiles and Deltas in a linked worktree | Record time to first useful rows |
| 114 | Check peak memory for Packfiles and Deltas in a linked worktree | Record steady frame cost |
| 115 | Check peak memory for Packfiles and Deltas in a linked worktree | Record bytes accepted from child output |
| 116 | Check peak memory for Packfiles and Deltas in a linked worktree | Record Git and gh process count |
| 117 | Check peak memory for Packfiles and Deltas in a linked worktree | Record maximum retained document bytes |
| 118 | Check peak memory for Packfiles and Deltas in a linked worktree | Record cache disposition and complete key |
| 119 | Check peak memory for Packfiles and Deltas in a linked worktree | Record stale reply rejection |
| 120 | Check peak memory for Packfiles and Deltas in a linked worktree | Record visible state after failure |
| 121 | Check peak memory for Packfiles and Deltas in cold and warm cache states | Record time to first useful rows |
| 122 | Check peak memory for Packfiles and Deltas in cold and warm cache states | Record steady frame cost |
| 123 | Check peak memory for Packfiles and Deltas in cold and warm cache states | Record bytes accepted from child output |
| 124 | Check peak memory for Packfiles and Deltas in cold and warm cache states | Record Git and gh process count |
| 125 | Check peak memory for Packfiles and Deltas in cold and warm cache states | Record maximum retained document bytes |
| 126 | Check peak memory for Packfiles and Deltas in cold and warm cache states | Record cache disposition and complete key |
| 127 | Check peak memory for Packfiles and Deltas in cold and warm cache states | Record stale reply rejection |
| 128 | Check peak memory for Packfiles and Deltas in cold and warm cache states | Record visible state after failure |
| 129 | Check network transfer for Packfiles and Deltas in a small local repository | Record time to first useful rows |
| 130 | Check network transfer for Packfiles and Deltas in a small local repository | Record steady frame cost |
| 131 | Check network transfer for Packfiles and Deltas in a small local repository | Record bytes accepted from child output |
| 132 | Check network transfer for Packfiles and Deltas in a small local repository | Record Git and gh process count |
| 133 | Check network transfer for Packfiles and Deltas in a small local repository | Record maximum retained document bytes |
| 134 | Check network transfer for Packfiles and Deltas in a small local repository | Record cache disposition and complete key |
| 135 | Check network transfer for Packfiles and Deltas in a small local repository | Record stale reply rejection |
| 136 | Check network transfer for Packfiles and Deltas in a small local repository | Record visible state after failure |
| 137 | Check network transfer for Packfiles and Deltas in a monorepo with many changed paths | Record time to first useful rows |
| 138 | Check network transfer for Packfiles and Deltas in a monorepo with many changed paths | Record steady frame cost |
| 139 | Check network transfer for Packfiles and Deltas in a monorepo with many changed paths | Record bytes accepted from child output |
| 140 | Check network transfer for Packfiles and Deltas in a monorepo with many changed paths | Record Git and gh process count |
| 141 | Check network transfer for Packfiles and Deltas in a monorepo with many changed paths | Record maximum retained document bytes |
| 142 | Check network transfer for Packfiles and Deltas in a monorepo with many changed paths | Record cache disposition and complete key |
| 143 | Check network transfer for Packfiles and Deltas in a monorepo with many changed paths | Record stale reply rejection |
| 144 | Check network transfer for Packfiles and Deltas in a monorepo with many changed paths | Record visible state after failure |
| 145 | Check network transfer for Packfiles and Deltas in a pull request containing generated files | Record time to first useful rows |
| 146 | Check network transfer for Packfiles and Deltas in a pull request containing generated files | Record steady frame cost |
| 147 | Check network transfer for Packfiles and Deltas in a pull request containing generated files | Record bytes accepted from child output |
| 148 | Check network transfer for Packfiles and Deltas in a pull request containing generated files | Record Git and gh process count |
| 149 | Check network transfer for Packfiles and Deltas in a pull request containing generated files | Record maximum retained document bytes |
| 150 | Check network transfer for Packfiles and Deltas in a pull request containing generated files | Record cache disposition and complete key |
| 151 | Check network transfer for Packfiles and Deltas in a pull request containing generated files | Record stale reply rejection |
| 152 | Check network transfer for Packfiles and Deltas in a pull request containing generated files | Record visible state after failure |
| 153 | Check network transfer for Packfiles and Deltas in a deeply diverged branch | Record time to first useful rows |
| 154 | Check network transfer for Packfiles and Deltas in a deeply diverged branch | Record steady frame cost |
| 155 | Check network transfer for Packfiles and Deltas in a deeply diverged branch | Record bytes accepted from child output |
| 156 | Check network transfer for Packfiles and Deltas in a deeply diverged branch | Record Git and gh process count |
| 157 | Check network transfer for Packfiles and Deltas in a deeply diverged branch | Record maximum retained document bytes |
| 158 | Check network transfer for Packfiles and Deltas in a deeply diverged branch | Record cache disposition and complete key |
| 159 | Check network transfer for Packfiles and Deltas in a deeply diverged branch | Record stale reply rejection |
| 160 | Check network transfer for Packfiles and Deltas in a deeply diverged branch | Record visible state after failure |
| 161 | Check network transfer for Packfiles and Deltas in an unavailable network | Record time to first useful rows |
| 162 | Check network transfer for Packfiles and Deltas in an unavailable network | Record steady frame cost |
| 163 | Check network transfer for Packfiles and Deltas in an unavailable network | Record bytes accepted from child output |
| 164 | Check network transfer for Packfiles and Deltas in an unavailable network | Record Git and gh process count |
| 165 | Check network transfer for Packfiles and Deltas in an unavailable network | Record maximum retained document bytes |
| 166 | Check network transfer for Packfiles and Deltas in an unavailable network | Record cache disposition and complete key |
| 167 | Check network transfer for Packfiles and Deltas in an unavailable network | Record stale reply rejection |
| 168 | Check network transfer for Packfiles and Deltas in an unavailable network | Record visible state after failure |
| 169 | Check network transfer for Packfiles and Deltas in rapid keyboard navigation | Record time to first useful rows |
| 170 | Check network transfer for Packfiles and Deltas in rapid keyboard navigation | Record steady frame cost |
| 171 | Check network transfer for Packfiles and Deltas in rapid keyboard navigation | Record bytes accepted from child output |
| 172 | Check network transfer for Packfiles and Deltas in rapid keyboard navigation | Record Git and gh process count |
| 173 | Check network transfer for Packfiles and Deltas in rapid keyboard navigation | Record maximum retained document bytes |
| 174 | Check network transfer for Packfiles and Deltas in rapid keyboard navigation | Record cache disposition and complete key |
| 175 | Check network transfer for Packfiles and Deltas in rapid keyboard navigation | Record stale reply rejection |
| 176 | Check network transfer for Packfiles and Deltas in rapid keyboard navigation | Record visible state after failure |
| 177 | Check network transfer for Packfiles and Deltas in a linked worktree | Record time to first useful rows |
| 178 | Check network transfer for Packfiles and Deltas in a linked worktree | Record steady frame cost |
| 179 | Check network transfer for Packfiles and Deltas in a linked worktree | Record bytes accepted from child output |
| 180 | Check network transfer for Packfiles and Deltas in a linked worktree | Record Git and gh process count |
| 181 | Check network transfer for Packfiles and Deltas in a linked worktree | Record maximum retained document bytes |
| 182 | Check network transfer for Packfiles and Deltas in a linked worktree | Record cache disposition and complete key |
| 183 | Check network transfer for Packfiles and Deltas in a linked worktree | Record stale reply rejection |
| 184 | Check network transfer for Packfiles and Deltas in a linked worktree | Record visible state after failure |
| 185 | Check network transfer for Packfiles and Deltas in cold and warm cache states | Record time to first useful rows |
| 186 | Check network transfer for Packfiles and Deltas in cold and warm cache states | Record steady frame cost |
| 187 | Check network transfer for Packfiles and Deltas in cold and warm cache states | Record bytes accepted from child output |
| 188 | Check network transfer for Packfiles and Deltas in cold and warm cache states | Record Git and gh process count |
| 189 | Check network transfer for Packfiles and Deltas in cold and warm cache states | Record maximum retained document bytes |
| 190 | Check network transfer for Packfiles and Deltas in cold and warm cache states | Record cache disposition and complete key |
| 191 | Check network transfer for Packfiles and Deltas in cold and warm cache states | Record stale reply rejection |
| 192 | Check network transfer for Packfiles and Deltas in cold and warm cache states | Record visible state after failure |
| 193 | Check subprocess count for Packfiles and Deltas in a small local repository | Record time to first useful rows |
| 194 | Check subprocess count for Packfiles and Deltas in a small local repository | Record steady frame cost |
| 195 | Check subprocess count for Packfiles and Deltas in a small local repository | Record bytes accepted from child output |
| 196 | Check subprocess count for Packfiles and Deltas in a small local repository | Record Git and gh process count |
| 197 | Check subprocess count for Packfiles and Deltas in a small local repository | Record maximum retained document bytes |
| 198 | Check subprocess count for Packfiles and Deltas in a small local repository | Record cache disposition and complete key |
| 199 | Check subprocess count for Packfiles and Deltas in a small local repository | Record stale reply rejection |
| 200 | Check subprocess count for Packfiles and Deltas in a small local repository | Record visible state after failure |
| 201 | Check subprocess count for Packfiles and Deltas in a monorepo with many changed paths | Record time to first useful rows |
| 202 | Check subprocess count for Packfiles and Deltas in a monorepo with many changed paths | Record steady frame cost |
| 203 | Check subprocess count for Packfiles and Deltas in a monorepo with many changed paths | Record bytes accepted from child output |
| 204 | Check subprocess count for Packfiles and Deltas in a monorepo with many changed paths | Record Git and gh process count |
| 205 | Check subprocess count for Packfiles and Deltas in a monorepo with many changed paths | Record maximum retained document bytes |
| 206 | Check subprocess count for Packfiles and Deltas in a monorepo with many changed paths | Record cache disposition and complete key |
| 207 | Check subprocess count for Packfiles and Deltas in a monorepo with many changed paths | Record stale reply rejection |
| 208 | Check subprocess count for Packfiles and Deltas in a monorepo with many changed paths | Record visible state after failure |
| 209 | Check subprocess count for Packfiles and Deltas in a pull request containing generated files | Record time to first useful rows |
| 210 | Check subprocess count for Packfiles and Deltas in a pull request containing generated files | Record steady frame cost |
| 211 | Check subprocess count for Packfiles and Deltas in a pull request containing generated files | Record bytes accepted from child output |
| 212 | Check subprocess count for Packfiles and Deltas in a pull request containing generated files | Record Git and gh process count |
| 213 | Check subprocess count for Packfiles and Deltas in a pull request containing generated files | Record maximum retained document bytes |
| 214 | Check subprocess count for Packfiles and Deltas in a pull request containing generated files | Record cache disposition and complete key |
| 215 | Check subprocess count for Packfiles and Deltas in a pull request containing generated files | Record stale reply rejection |
| 216 | Check subprocess count for Packfiles and Deltas in a pull request containing generated files | Record visible state after failure |
| 217 | Check subprocess count for Packfiles and Deltas in a deeply diverged branch | Record time to first useful rows |
| 218 | Check subprocess count for Packfiles and Deltas in a deeply diverged branch | Record steady frame cost |
| 219 | Check subprocess count for Packfiles and Deltas in a deeply diverged branch | Record bytes accepted from child output |
| 220 | Check subprocess count for Packfiles and Deltas in a deeply diverged branch | Record Git and gh process count |
| 221 | Check subprocess count for Packfiles and Deltas in a deeply diverged branch | Record maximum retained document bytes |
| 222 | Check subprocess count for Packfiles and Deltas in a deeply diverged branch | Record cache disposition and complete key |
| 223 | Check subprocess count for Packfiles and Deltas in a deeply diverged branch | Record stale reply rejection |
| 224 | Check subprocess count for Packfiles and Deltas in a deeply diverged branch | Record visible state after failure |
| 225 | Check subprocess count for Packfiles and Deltas in an unavailable network | Record time to first useful rows |
| 226 | Check subprocess count for Packfiles and Deltas in an unavailable network | Record steady frame cost |
| 227 | Check subprocess count for Packfiles and Deltas in an unavailable network | Record bytes accepted from child output |
| 228 | Check subprocess count for Packfiles and Deltas in an unavailable network | Record Git and gh process count |
| 229 | Check subprocess count for Packfiles and Deltas in an unavailable network | Record maximum retained document bytes |
| 230 | Check subprocess count for Packfiles and Deltas in an unavailable network | Record cache disposition and complete key |
| 231 | Check subprocess count for Packfiles and Deltas in an unavailable network | Record stale reply rejection |
| 232 | Check subprocess count for Packfiles and Deltas in an unavailable network | Record visible state after failure |
| 233 | Check subprocess count for Packfiles and Deltas in rapid keyboard navigation | Record time to first useful rows |
| 234 | Check subprocess count for Packfiles and Deltas in rapid keyboard navigation | Record steady frame cost |
| 235 | Check subprocess count for Packfiles and Deltas in rapid keyboard navigation | Record bytes accepted from child output |
| 236 | Check subprocess count for Packfiles and Deltas in rapid keyboard navigation | Record Git and gh process count |
| 237 | Check subprocess count for Packfiles and Deltas in rapid keyboard navigation | Record maximum retained document bytes |
| 238 | Check subprocess count for Packfiles and Deltas in rapid keyboard navigation | Record cache disposition and complete key |
| 239 | Check subprocess count for Packfiles and Deltas in rapid keyboard navigation | Record stale reply rejection |
| 240 | Check subprocess count for Packfiles and Deltas in rapid keyboard navigation | Record visible state after failure |
| 241 | Check subprocess count for Packfiles and Deltas in a linked worktree | Record time to first useful rows |
| 242 | Check subprocess count for Packfiles and Deltas in a linked worktree | Record steady frame cost |
| 243 | Check subprocess count for Packfiles and Deltas in a linked worktree | Record bytes accepted from child output |
| 244 | Check subprocess count for Packfiles and Deltas in a linked worktree | Record Git and gh process count |
| 245 | Check subprocess count for Packfiles and Deltas in a linked worktree | Record maximum retained document bytes |
| 246 | Check subprocess count for Packfiles and Deltas in a linked worktree | Record cache disposition and complete key |
| 247 | Check subprocess count for Packfiles and Deltas in a linked worktree | Record stale reply rejection |
| 248 | Check subprocess count for Packfiles and Deltas in a linked worktree | Record visible state after failure |
| 249 | Check subprocess count for Packfiles and Deltas in cold and warm cache states | Record time to first useful rows |
| 250 | Check subprocess count for Packfiles and Deltas in cold and warm cache states | Record steady frame cost |
| 251 | Check subprocess count for Packfiles and Deltas in cold and warm cache states | Record bytes accepted from child output |
| 252 | Check subprocess count for Packfiles and Deltas in cold and warm cache states | Record Git and gh process count |
| 253 | Check subprocess count for Packfiles and Deltas in cold and warm cache states | Record maximum retained document bytes |
| 254 | Check subprocess count for Packfiles and Deltas in cold and warm cache states | Record cache disposition and complete key |
| 255 | Check subprocess count for Packfiles and Deltas in cold and warm cache states | Record stale reply rejection |
| 256 | Check subprocess count for Packfiles and Deltas in cold and warm cache states | Record visible state after failure |
| 257 | Check cache identity for Packfiles and Deltas in a small local repository | Record time to first useful rows |
| 258 | Check cache identity for Packfiles and Deltas in a small local repository | Record steady frame cost |
| 259 | Check cache identity for Packfiles and Deltas in a small local repository | Record bytes accepted from child output |
| 260 | Check cache identity for Packfiles and Deltas in a small local repository | Record Git and gh process count |
| 261 | Check cache identity for Packfiles and Deltas in a small local repository | Record maximum retained document bytes |
| 262 | Check cache identity for Packfiles and Deltas in a small local repository | Record cache disposition and complete key |
| 263 | Check cache identity for Packfiles and Deltas in a small local repository | Record stale reply rejection |
| 264 | Check cache identity for Packfiles and Deltas in a small local repository | Record visible state after failure |
| 265 | Check cache identity for Packfiles and Deltas in a monorepo with many changed paths | Record time to first useful rows |
| 266 | Check cache identity for Packfiles and Deltas in a monorepo with many changed paths | Record steady frame cost |
| 267 | Check cache identity for Packfiles and Deltas in a monorepo with many changed paths | Record bytes accepted from child output |
| 268 | Check cache identity for Packfiles and Deltas in a monorepo with many changed paths | Record Git and gh process count |
| 269 | Check cache identity for Packfiles and Deltas in a monorepo with many changed paths | Record maximum retained document bytes |
| 270 | Check cache identity for Packfiles and Deltas in a monorepo with many changed paths | Record cache disposition and complete key |
| 271 | Check cache identity for Packfiles and Deltas in a monorepo with many changed paths | Record stale reply rejection |
| 272 | Check cache identity for Packfiles and Deltas in a monorepo with many changed paths | Record visible state after failure |
| 273 | Check cache identity for Packfiles and Deltas in a pull request containing generated files | Record time to first useful rows |
| 274 | Check cache identity for Packfiles and Deltas in a pull request containing generated files | Record steady frame cost |
| 275 | Check cache identity for Packfiles and Deltas in a pull request containing generated files | Record bytes accepted from child output |
| 276 | Check cache identity for Packfiles and Deltas in a pull request containing generated files | Record Git and gh process count |
| 277 | Check cache identity for Packfiles and Deltas in a pull request containing generated files | Record maximum retained document bytes |
| 278 | Check cache identity for Packfiles and Deltas in a pull request containing generated files | Record cache disposition and complete key |
| 279 | Check cache identity for Packfiles and Deltas in a pull request containing generated files | Record stale reply rejection |
| 280 | Check cache identity for Packfiles and Deltas in a pull request containing generated files | Record visible state after failure |
| 281 | Check cache identity for Packfiles and Deltas in a deeply diverged branch | Record time to first useful rows |
| 282 | Check cache identity for Packfiles and Deltas in a deeply diverged branch | Record steady frame cost |
| 283 | Check cache identity for Packfiles and Deltas in a deeply diverged branch | Record bytes accepted from child output |
| 284 | Check cache identity for Packfiles and Deltas in a deeply diverged branch | Record Git and gh process count |
| 285 | Check cache identity for Packfiles and Deltas in a deeply diverged branch | Record maximum retained document bytes |
| 286 | Check cache identity for Packfiles and Deltas in a deeply diverged branch | Record cache disposition and complete key |
| 287 | Check cache identity for Packfiles and Deltas in a deeply diverged branch | Record stale reply rejection |
| 288 | Check cache identity for Packfiles and Deltas in a deeply diverged branch | Record visible state after failure |
| 289 | Check cache identity for Packfiles and Deltas in an unavailable network | Record time to first useful rows |
| 290 | Check cache identity for Packfiles and Deltas in an unavailable network | Record steady frame cost |
| 291 | Check cache identity for Packfiles and Deltas in an unavailable network | Record bytes accepted from child output |
| 292 | Check cache identity for Packfiles and Deltas in an unavailable network | Record Git and gh process count |
| 293 | Check cache identity for Packfiles and Deltas in an unavailable network | Record maximum retained document bytes |
| 294 | Check cache identity for Packfiles and Deltas in an unavailable network | Record cache disposition and complete key |
| 295 | Check cache identity for Packfiles and Deltas in an unavailable network | Record stale reply rejection |
| 296 | Check cache identity for Packfiles and Deltas in an unavailable network | Record visible state after failure |
| 297 | Check cache identity for Packfiles and Deltas in rapid keyboard navigation | Record time to first useful rows |
| 298 | Check cache identity for Packfiles and Deltas in rapid keyboard navigation | Record steady frame cost |
| 299 | Check cache identity for Packfiles and Deltas in rapid keyboard navigation | Record bytes accepted from child output |
| 300 | Check cache identity for Packfiles and Deltas in rapid keyboard navigation | Record Git and gh process count |
| 301 | Check cache identity for Packfiles and Deltas in rapid keyboard navigation | Record maximum retained document bytes |
| 302 | Check cache identity for Packfiles and Deltas in rapid keyboard navigation | Record cache disposition and complete key |
| 303 | Check cache identity for Packfiles and Deltas in rapid keyboard navigation | Record stale reply rejection |
| 304 | Check cache identity for Packfiles and Deltas in rapid keyboard navigation | Record visible state after failure |
| 305 | Check cache identity for Packfiles and Deltas in a linked worktree | Record time to first useful rows |
| 306 | Check cache identity for Packfiles and Deltas in a linked worktree | Record steady frame cost |
| 307 | Check cache identity for Packfiles and Deltas in a linked worktree | Record bytes accepted from child output |
| 308 | Check cache identity for Packfiles and Deltas in a linked worktree | Record Git and gh process count |
| 309 | Check cache identity for Packfiles and Deltas in a linked worktree | Record maximum retained document bytes |
| 310 | Check cache identity for Packfiles and Deltas in a linked worktree | Record cache disposition and complete key |
| 311 | Check cache identity for Packfiles and Deltas in a linked worktree | Record stale reply rejection |
| 312 | Check cache identity for Packfiles and Deltas in a linked worktree | Record visible state after failure |
| 313 | Check cache identity for Packfiles and Deltas in cold and warm cache states | Record time to first useful rows |
| 314 | Check cache identity for Packfiles and Deltas in cold and warm cache states | Record steady frame cost |
| 315 | Check cache identity for Packfiles and Deltas in cold and warm cache states | Record bytes accepted from child output |
| 316 | Check cache identity for Packfiles and Deltas in cold and warm cache states | Record Git and gh process count |
| 317 | Check cache identity for Packfiles and Deltas in cold and warm cache states | Record maximum retained document bytes |
| 318 | Check cache identity for Packfiles and Deltas in cold and warm cache states | Record cache disposition and complete key |
| 319 | Check cache identity for Packfiles and Deltas in cold and warm cache states | Record stale reply rejection |
| 320 | Check cache identity for Packfiles and Deltas in cold and warm cache states | Record visible state after failure |
| 321 | Check concurrency ordering for Packfiles and Deltas in a small local repository | Record time to first useful rows |
| 322 | Check concurrency ordering for Packfiles and Deltas in a small local repository | Record steady frame cost |
| 323 | Check concurrency ordering for Packfiles and Deltas in a small local repository | Record bytes accepted from child output |
| 324 | Check concurrency ordering for Packfiles and Deltas in a small local repository | Record Git and gh process count |
| 325 | Check concurrency ordering for Packfiles and Deltas in a small local repository | Record maximum retained document bytes |
| 326 | Check concurrency ordering for Packfiles and Deltas in a small local repository | Record cache disposition and complete key |
| 327 | Check concurrency ordering for Packfiles and Deltas in a small local repository | Record stale reply rejection |
| 328 | Check concurrency ordering for Packfiles and Deltas in a small local repository | Record visible state after failure |
| 329 | Check concurrency ordering for Packfiles and Deltas in a monorepo with many changed paths | Record time to first useful rows |
| 330 | Check concurrency ordering for Packfiles and Deltas in a monorepo with many changed paths | Record steady frame cost |
| 331 | Check concurrency ordering for Packfiles and Deltas in a monorepo with many changed paths | Record bytes accepted from child output |
| 332 | Check concurrency ordering for Packfiles and Deltas in a monorepo with many changed paths | Record Git and gh process count |
| 333 | Check concurrency ordering for Packfiles and Deltas in a monorepo with many changed paths | Record maximum retained document bytes |
| 334 | Check concurrency ordering for Packfiles and Deltas in a monorepo with many changed paths | Record cache disposition and complete key |
| 335 | Check concurrency ordering for Packfiles and Deltas in a monorepo with many changed paths | Record stale reply rejection |
| 336 | Check concurrency ordering for Packfiles and Deltas in a monorepo with many changed paths | Record visible state after failure |
| 337 | Check concurrency ordering for Packfiles and Deltas in a pull request containing generated files | Record time to first useful rows |
| 338 | Check concurrency ordering for Packfiles and Deltas in a pull request containing generated files | Record steady frame cost |
| 339 | Check concurrency ordering for Packfiles and Deltas in a pull request containing generated files | Record bytes accepted from child output |
| 340 | Check concurrency ordering for Packfiles and Deltas in a pull request containing generated files | Record Git and gh process count |
| 341 | Check concurrency ordering for Packfiles and Deltas in a pull request containing generated files | Record maximum retained document bytes |
| 342 | Check concurrency ordering for Packfiles and Deltas in a pull request containing generated files | Record cache disposition and complete key |
| 343 | Check concurrency ordering for Packfiles and Deltas in a pull request containing generated files | Record stale reply rejection |
| 344 | Check concurrency ordering for Packfiles and Deltas in a pull request containing generated files | Record visible state after failure |
| 345 | Check concurrency ordering for Packfiles and Deltas in a deeply diverged branch | Record time to first useful rows |
| 346 | Check concurrency ordering for Packfiles and Deltas in a deeply diverged branch | Record steady frame cost |
| 347 | Check concurrency ordering for Packfiles and Deltas in a deeply diverged branch | Record bytes accepted from child output |
| 348 | Check concurrency ordering for Packfiles and Deltas in a deeply diverged branch | Record Git and gh process count |
| 349 | Check concurrency ordering for Packfiles and Deltas in a deeply diverged branch | Record maximum retained document bytes |
| 350 | Check concurrency ordering for Packfiles and Deltas in a deeply diverged branch | Record cache disposition and complete key |
| 351 | Check concurrency ordering for Packfiles and Deltas in a deeply diverged branch | Record stale reply rejection |
| 352 | Check concurrency ordering for Packfiles and Deltas in a deeply diverged branch | Record visible state after failure |
| 353 | Check concurrency ordering for Packfiles and Deltas in an unavailable network | Record time to first useful rows |
| 354 | Check concurrency ordering for Packfiles and Deltas in an unavailable network | Record steady frame cost |
| 355 | Check concurrency ordering for Packfiles and Deltas in an unavailable network | Record bytes accepted from child output |
| 356 | Check concurrency ordering for Packfiles and Deltas in an unavailable network | Record Git and gh process count |
| 357 | Check concurrency ordering for Packfiles and Deltas in an unavailable network | Record maximum retained document bytes |
| 358 | Check concurrency ordering for Packfiles and Deltas in an unavailable network | Record cache disposition and complete key |
| 359 | Check concurrency ordering for Packfiles and Deltas in an unavailable network | Record stale reply rejection |
| 360 | Check concurrency ordering for Packfiles and Deltas in an unavailable network | Record visible state after failure |
| 361 | Check concurrency ordering for Packfiles and Deltas in rapid keyboard navigation | Record time to first useful rows |
| 362 | Check concurrency ordering for Packfiles and Deltas in rapid keyboard navigation | Record steady frame cost |
| 363 | Check concurrency ordering for Packfiles and Deltas in rapid keyboard navigation | Record bytes accepted from child output |
| 364 | Check concurrency ordering for Packfiles and Deltas in rapid keyboard navigation | Record Git and gh process count |
| 365 | Check concurrency ordering for Packfiles and Deltas in rapid keyboard navigation | Record maximum retained document bytes |
| 366 | Check concurrency ordering for Packfiles and Deltas in rapid keyboard navigation | Record cache disposition and complete key |
| 367 | Check concurrency ordering for Packfiles and Deltas in rapid keyboard navigation | Record stale reply rejection |
| 368 | Check concurrency ordering for Packfiles and Deltas in rapid keyboard navigation | Record visible state after failure |
| 369 | Check concurrency ordering for Packfiles and Deltas in a linked worktree | Record time to first useful rows |
| 370 | Check concurrency ordering for Packfiles and Deltas in a linked worktree | Record steady frame cost |
| 371 | Check concurrency ordering for Packfiles and Deltas in a linked worktree | Record bytes accepted from child output |
| 372 | Check concurrency ordering for Packfiles and Deltas in a linked worktree | Record Git and gh process count |
| 373 | Check concurrency ordering for Packfiles and Deltas in a linked worktree | Record maximum retained document bytes |
| 374 | Check concurrency ordering for Packfiles and Deltas in a linked worktree | Record cache disposition and complete key |
| 375 | Check concurrency ordering for Packfiles and Deltas in a linked worktree | Record stale reply rejection |
| 376 | Check concurrency ordering for Packfiles and Deltas in a linked worktree | Record visible state after failure |
| 377 | Check concurrency ordering for Packfiles and Deltas in cold and warm cache states | Record time to first useful rows |
| 378 | Check concurrency ordering for Packfiles and Deltas in cold and warm cache states | Record steady frame cost |
| 379 | Check concurrency ordering for Packfiles and Deltas in cold and warm cache states | Record bytes accepted from child output |
| 380 | Check concurrency ordering for Packfiles and Deltas in cold and warm cache states | Record Git and gh process count |
| 381 | Check concurrency ordering for Packfiles and Deltas in cold and warm cache states | Record maximum retained document bytes |
| 382 | Check concurrency ordering for Packfiles and Deltas in cold and warm cache states | Record cache disposition and complete key |
| 383 | Check concurrency ordering for Packfiles and Deltas in cold and warm cache states | Record stale reply rejection |
| 384 | Check concurrency ordering for Packfiles and Deltas in cold and warm cache states | Record visible state after failure |
| 385 | Check failure degradation for Packfiles and Deltas in a small local repository | Record time to first useful rows |
| 386 | Check failure degradation for Packfiles and Deltas in a small local repository | Record steady frame cost |
| 387 | Check failure degradation for Packfiles and Deltas in a small local repository | Record bytes accepted from child output |
| 388 | Check failure degradation for Packfiles and Deltas in a small local repository | Record Git and gh process count |
| 389 | Check failure degradation for Packfiles and Deltas in a small local repository | Record maximum retained document bytes |
| 390 | Check failure degradation for Packfiles and Deltas in a small local repository | Record cache disposition and complete key |
| 391 | Check failure degradation for Packfiles and Deltas in a small local repository | Record stale reply rejection |
| 392 | Check failure degradation for Packfiles and Deltas in a small local repository | Record visible state after failure |
| 393 | Check failure degradation for Packfiles and Deltas in a monorepo with many changed paths | Record time to first useful rows |
