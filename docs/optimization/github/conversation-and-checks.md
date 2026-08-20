# Conversation and checks

This page documents the two GitHub read paths that carry a pull request's human activity into the
terminal: the conversation (issue comments, reviews, inline review comments, commits, and lifecycle
events flattened into one ordered thread) and the checks surface (check runs, their GitHub Actions
job steps, and raw runner logs). Both paths are exercises in reading an unbounded remote resource
under fixed local caps without ever dropping the part the reader actually wants. The conversation
half centers on PR #48, which turned an uncapped oldest-first stream into newest-first bounded
paging so the 500-entry cap can only ever drop the oldest activity. The checks half implements
invariant 11a of `ARCHITECTURE.md`: whole-second step attachment for sub-second runner lines,
tailing a running job by re-reading its growing log, and warming settled logs in the background so
browsing the check list costs disk reads instead of round trips.

## Contents

- [The problem: capped reads that kept the wrong half](#the-problem-capped-reads-that-kept-the-wrong-half)
- [GitHub pagination on the wire](#github-pagination-on-the-wire)
- [The two conversation streams](#the-two-conversation-streams)
- [Reading a stream newest-first](#reading-a-stream-newest-first)
- [The conversation record format](#the-conversation-record-format)
- [Flattening into one ordered thread](#flattening-into-one-ordered-thread)
- [The conversation in the terminal](#the-conversation-in-the-terminal)
- [Check runs: the list](#check-runs-the-list)
- [Actions job steps](#actions-job-steps)
- [The raw log: fetching, tailing, and 404 before the blob](#the-raw-log-fetching-tailing-and-404-before-the-blob)
- [Parsing runner output](#parsing-runner-output)
- [Attaching lines to steps by whole seconds](#attaching-lines-to-steps-by-whole-seconds)
- [Time math without a date library](#time-math-without-a-date-library)
- [Adaptive polling and per-stream floors](#adaptive-polling-and-per-stream-floors)
- [Warming settled logs on the warm lane](#warming-settled-logs-on-the-warm-lane)
- [Cache keys and lifetimes](#cache-keys-and-lifetimes)
- [Design alternatives and why they lost](#design-alternatives-and-why-they-lost)
- [Edge cases and failure modes](#edge-cases-and-failure-modes)

## The problem: capped reads that kept the wrong half

Every long-lived resource Quinjet reads from GitHub is bounded. `ARCHITECTURE.md` invariant 5 puts
the conversation cap in one sentence:

> conversations at 500 entries fetched newest-first in bounded pages (review comments descending,
> the timeline from its last Link page) so the cap can only ever drop the oldest activity

That sentence is the result of PR #48 (`feat: newest-first paged conversations and jump-to-bottom`,
squash-merged as commit `b75be82` on 2026-08-20; 5 files changed, 346 insertions, 88 deletions).
Before it, the conversation reader had a defect that is easy to make and easy to miss: it combined
a cap with a stream that arrives in the wrong order.

**1. REST pagination is oldest-first by default.** GitHub's issue timeline endpoint serves events
in the order they happened, page 1 first. The pull request review comments endpoint defaults to
the same ascending creation order. A client that walks pages forward therefore receives the oldest
activity first, and receives the newest activity last.

**2. A cap applied to an oldest-first stream keeps the oldest entries.** The old reader ran
`gh api --paginate` on both endpoints, streaming every page of a thread through one uncapped
invocation, then truncated the merged result at `MAX_CONVERSATION_ENTRIES = 500`. On a thread with
more than 500 entries the entries that fell off the end were the newest ones: the review that just
landed, the comment the reader opened the pull request to see. The cap protected memory and
rendering time while destroying exactly the information the view exists to show.

**3. One uncapped invocation is also the wrong shape for a bound.** `--paginate` concatenates
pages inside `gh` itself, so the only cap Quinjet could apply was its capped-pipe read of the
child's stdout. Hitting that pipe cap kills the child mid-page and truncates mid-record; there is
no way to say "stop after roughly 500 records" from outside the process. Worse, an oversized
single response failed the read entirely instead of degrading, which is the hard-fail defect fixed
inside #48 (its third commit bullet reads `fix: degrade oversized conversation pages instead of
failing`; the mechanics are covered in
[Reading a stream newest-first](#reading-a-stream-newest-first)).

The benchmark that motivated the whole optimization stack made this concrete. The Bun rewrite pull
request (oven-sh/bun#30412, the stress target described in [../benchmarking.md](../benchmarking.md))
carries a thread of roughly 1,100 entries. The PR #48 evidence comment states the outcome exactly:

> the bun rewrite pr's thread has ~1,100 entries; the conversation now reads newest-first in
> bounded pages, so the tail is always the latest activity and only the oldest is dropped at the
> cap

and gives the one measured timing this page is allowed to cite for the conversation path:

> cold read finishes in 26s over ~12 bounded 100-entry pages instead of one uncapped --paginate
> stream

The rest of this page walks the machinery that makes those two sentences true, then crosses to the
checks surface, where the same bounded-read discipline meets a different problem: a log that is
still being written while the reader watches it.

## GitHub pagination on the wire

Quinjet's fix depends on two properties of GitHub's REST API that are worth understanding
precisely, because the code exploits both to the letter: the `Link` response header that names
page positions, and conditional requests that make an unchanged answer free. Both are described in
the [GitHub REST documentation](https://docs.github.com/en/rest); this section covers the exact
subset the code relies on.

### The Link header

A paginated GitHub response carries an HTTP `Link` header (RFC 8288 web linking) whose value is a
comma-separated list of segments, each a URL in angle brackets followed by a `rel` parameter
naming the relationship of that URL to the current page:

```text
link: <https://api.github.com/repositories/1300192/issues?page=2>; rel="next",
      <https://api.github.com/repositories/1300192/issues?page=12>; rel="last"
```

The four relations GitHub emits are:

| Relation | Meaning | Present when |
| --- | --- | --- |
| `rel="next"` | The page after this one | This is not the last page |
| `rel="prev"` | The page before this one | This is not the first page |
| `rel="first"` | Page 1 | This is not the first page |
| `rel="last"` | The final page number | The listing has more than one page |

Two absences carry information. A response with no `rel="next"` is the final page, which is how a
forward walk knows to stop. A first-page response with no `Link` header at all is the whole
listing, which is how a single-page thread is recognized without a second request. Page size is a
query parameter, `per_page`, capped at 100 by GitHub for these endpoints; Quinjet always asks for
the maximum via `CONVERSATION_PAGE_SIZE`:

```rust
const CONVERSATION_PAGE_SIZE: usize = 100;
```

The `rel="last"` segment is the one #48 leans on: it names the highest page number, which means a
client can start reading from the end of an ascending listing without walking to it. That is the
entire trick behind reading the timeline newest-first from an API that only sorts oldest-first.

### Parsing the last page

`src/git/github/mod.rs` extracts the last page number with a parser whose shape is dictated by a
real hazard in the header text:

```rust
/// The page number GitHub advertises as `rel="last"`, when the response is one
/// page of a longer listing.
fn last_page(head: &str) -> Option<usize> {
    let link = header_value(head, "link")?;
    link.split(',').find_map(|segment| {
        if !segment.contains("rel=\"last\"") {
            return None;
        }
        let url = segment.trim().strip_prefix('<')?.split('>').next()?;
        url.split(['?', '&']).find_map(|parameter| {
            parameter
                .strip_prefix("page=")
                .and_then(|value| value.parse().ok())
        })
    })
}
```

The hazard is that the URL inside the `rel="last"` segment contains both `page=12` and
`per_page=100`, and the string `per_page=100` contains the substring `page=100`. A naive
`find("page=")` over the raw URL matches inside `per_page` first and reports 100 as the last page,
which on a 12-page thread would send the reader to ten pages that do not exist. Splitting the URL
on `['?', '&']` isolates whole query parameters, so `strip_prefix("page=")` can only match the
parameter actually named `page`. The unit test in `src/git/github/mod.rs`
(`the_link_header_names_the_newest_timeline_page`) pins three cases: a header advertising
`page=2; rel="next"` and `page=12; rel="last"` yields `Some(12)`, a bare `HTTP/2.0 200 OK` head
yields `None` with the assertion message "a single page advertises no last page", and a URL
ordered `?page=7&per_page=100` yields `Some(7)` with the message "per_page never shadows the page
parameter".

The companion predicate is one line and answers the other question a walk needs:

```rust
fn has_next_page(head: &str) -> bool {
    header_value(head, "link").is_some_and(|link| link.contains("rel=\"next\""))
}
```

Both functions read the response head, which exists because every paginated read runs `gh api -i`:
the `-i` flag makes `gh` print the HTTP response head, a blank line, then the body, and
`split_http_response` (`src/git/github/mod.rs`) splits the two apart at the first `\r\n\r\n` or
`\n\n` boundary.

### Conditional requests

The second wire property is the conditional request. GitHub returns an `ETag` header with most
responses; a later request carrying that value in `If-None-Match` is answered `304 Not Modified`
with no body when nothing changed, and per GitHub's documented rate-limit accounting a 304 costs
nothing against the hourly quota. Quinjet wraps this in `validated_gh`, whose doc comment in
`src/git/github/mod.rs` states the design in full:

```rust
/// A validated read: GitHub is asked whether the answer changed, and answers
/// `304 Not Modified` when it did not. That reply carries no body and costs
/// nothing against the rate limit, which is what lets an unchanged thread be
/// re-checked as often as it is worth checking.
///
/// The entry holds the validator on its first line and the body after it, so
/// the two can never be stored out of step with each other.
pub(crate) struct ValidatedRead {
    pub data: Vec<u8>,
    pub unchanged: bool,
    pub complete: bool,
    pub truncated: bool,
    pub last_page: Option<usize>,
}
```

The mechanics: `validated_gh` reads its cache entry, peels the first line off as the stored ETag
(`split_validator`), and issues `gh api -i` with `-H "If-None-Match: <etag>"` when a validator
exists. A `304` status line returns the cached body with `unchanged: true`. A fresh answer
computes `complete = !output.stdout_truncated && !has_next_page(head)` and stores
`etag\nbody` back into the cache only when complete, so a partial first page can never be
validated as if it were the whole listing. The reply also carries `last_page` parsed from the
`Link` header, which is what hands the conversation reader its backward-walk starting point for
free on the same request that fetched page 1.

Two of those fields were added by #48 and are load-bearing for the hard-fail fix. `truncated`
reports that the capped pipe cut the child's stdout, and the bail condition inside `validated_gh`
was relaxed from `!output.status.success()` to
`!output.status.success() && !output.stdout_truncated`: a child killed at the cap exits non-zero
through no fault of the response, so its partial data is surfaced with `truncated: true` instead
of being converted into an error that would fail the whole conversation.

### The bounded transport underneath

Every one of these reads runs through the same capped-pipe subprocess plumbing, documented in
depth in [./api-strategy.md](./api-strategy.md) and summarized here because the conversation and
checks caps are calibrated against it. `run_gh` bounds stdout at
`MAX_GH_METADATA_BYTES = 2 * 1024 * 1024` (2 MiB); the raw-log path uses its own 8 MiB bound. The
core loop in `run_bounded_command` (`src/git/github/mod.rs`) reads stdout in 64 KiB chunks and
kills the child the moment the limit is crossed, rather than first collecting everything and
truncating afterward:

```rust
        let remaining = stdout_limit.saturating_sub(collected.len());
        if read > remaining {
            collected.extend_from_slice(buffer.get(..remaining).unwrap_or(&buffer));
            truncated = true;
            drop(child.kill());
            break;
        }
```

A killed child means the collected bytes can end mid-record. The per-page reader `api_page`
(`src/git/github/mod.rs`) repairs that at the byte level before anything downstream sees the data:

```rust
        let mut data = body.to_vec();
        if output.stdout_truncated {
            while data.last().is_some_and(|byte| *byte != b'\n') {
                let _ = data.pop();
            }
        }
```

Popping bytes until the buffer ends on `\n` guarantees only whole records survive a truncation,
which is what lets the conversation parser treat every newline-terminated line as a complete
TSV record without defending against a half-written one. Its doc comment names the contract: "One
bounded page of a listing endpoint: its body trimmed to whole records, plus whether GitHub
advertises another page after it."

## The two conversation streams

A pull request's conversation is not one resource on GitHub's side. It is assembled from two
endpoints with different capabilities, and the difference between those capabilities is why the
reader needs two paging strategies.

### The issue timeline

Every pull request is also an issue, and the issue timeline endpoint interleaves everything that
ever happened to it: comments, review submissions, pushed commits, force pushes, label changes,
renames, review requests, assignments, cross-references, and more. `src/git/github/conversation.rs`
builds the endpoint:

```rust
fn timeline_endpoint(pull_request: &PullRequest) -> String {
    format!(
        "repos/{}/issues/{}/timeline?per_page={CONVERSATION_PAGE_SIZE}",
        pull_request.base_repository.name_with_owner, pull_request.number
    )
}
```

The timeline has one crucial limitation: it serves events oldest-first and accepts no sort or
direction parameter. There is no way to ask GitHub for the newest timeline page as page 1. The
newest page is reachable only by number, and its number is exactly what `rel="last"` advertises.

### The review comments listing

Inline review comments (the ones anchored to a file and line) live on their own endpoint, and that
endpoint does accept a sort:

```rust
fn review_comment_endpoint(pull_request: &PullRequest) -> String {
    format!(
        "repos/{}/pulls/{}/comments?per_page={CONVERSATION_PAGE_SIZE}&sort=created&direction=desc",
        pull_request.base_repository.name_with_owner, pull_request.number
    )
}
```

The `&sort=created&direction=desc` suffix was added by #48. With a descending sort, page 1 is the
newest page and a plain forward walk is already newest-first; no `Link` gymnastics required.

Why read inline comments from their own endpoint at all, when the timeline also carries them? The
doc comment on `Repository::pull_request_conversation` answers:

```rust
    /// Read the whole pull-request conversation: issue comments, reviews and
    /// their inline comments, pushed commits, force pushes, and the lifecycle
    /// events GitHub shows between them.
    ///
    /// Inline review comments are read from their own endpoint rather than
    /// trusted to the timeline, which only groups them into `line-commented`
    /// entries for some pull requests.
    pub(crate) fn pull_request_conversation(
```

The timeline's `line-commented` and `commit-commented` events group inline comments, but GitHub
emits those groups inconsistently across pull requests. Trusting the timeline alone would silently
lose inline comments on the pull requests where the grouping does not appear. Reading both and
deduplicating (covered in [Flattening into one ordered thread](#flattening-into-one-ordered-thread))
gets completeness from the dedicated endpoint and ordering context from the timeline.

### Two strategies, one enum

The difference in capability is encoded as a two-variant enum whose doc comment is the shortest
correct statement of this whole design:

```rust
/// How a stream reaches its newest entries. Review comments accept a
/// descending sort, so their newest page is page one. The timeline API only
/// serves oldest-first, so its newest page is the one `rel="last"` names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ConversationPaging {
    NewestFirst,
    LastPageFirst,
}
```

Each stream is described by one value of a small struct, so the reader function is written once
and parameterized rather than duplicated per endpoint:

```rust
struct ConversationStream {
    cache_key: String,
    validator_key: String,
    endpoint: String,
    jq: String,
    paging: ConversationPaging,
}
```

`pull_request_conversation` constructs exactly two of these. The timeline stream uses cache key
`conversation-timeline-v2\n{stamp}`, validator key
`conversation-timeline-validator-v2\n{identity}`, the timeline endpoint, the timeline jq program,
and `LastPageFirst`, with error context "unable to load the pull-request timeline". The comments
stream uses `conversation-comments-v2\n{stamp}`, `conversation-comments-validator-v2\n{identity}`,
the descending comments endpoint, `REVIEW_COMMENT_TSV_JQ`, and `NewestFirst`, with error context
"unable to load pull-request review comments". The `v2` suffixes exist because #48 changed the
stored format; a `v1` entry written by an older build simply never matches a `v2` key and ages out
of the size-bounded store described in [./caching.md](./caching.md).

The two key materials are built from the pull request itself:

```text
stamp    = "{base repository url, trailing '/' trimmed}\n{number}\n{updated_at}"
identity = "{base repository url, trailing '/' trimmed}\n{number}"
```

The `stamp` embeds `updated_at`, which GitHub moves on any activity, so a stamped snapshot is an
immutable answer to an immutable question: "what did the thread look like as of this update
instant". Any new comment produces a brand-new key rather than staleness in an old one, which is
the conversation's instance of invariant 12 (cached content whose key names its identity never
expires). The `identity` key deliberately omits `updated_at` because it stores the ETag validator,
which must survive across updates to be useful: the validator's whole job is to notice that the
answer changed.

### Server-side reduction: the jq programs

Both streams hand `gh` a jq program so the JSON-to-record reduction happens inside the `gh`
process and only compact TSV crosses the pipe. The review-comment program is a single fixed
mapping, quoted from `src/git/github/conversation.rs`:

```rust
const REVIEW_COMMENT_TSV_JQ: &str = r#".[] | ["review_comment", (.user.login // ""), (.created_at // ""), ((.path // "") + ":" + (((.line // .original_line) // 0)|tostring)), (.body // ""), (.html_url // ""), ((.pull_request_review_id // 0)|tostring), (.diff_hunk // "")] | @tsv"#;
```

Every field is defended with jq's alternative operator `//`, so a missing field becomes an empty
string or zero instead of a null that would derail `@tsv`. The `detail` field is synthesized as
`path:line`, falling back from `line` to `original_line` (the anchor a comment keeps after its
line moved in a later push) and then to `0`.

The timeline program is larger because, as its doc comment says, "GitHub gives each event type its
own field names, so the mapping has to be explicit; anything unrecognized still arrives with an
actor and a timestamp." It opens by discarding the events the app never renders:

```rust
/// Events GitHub records but never renders in a pull-request conversation.
/// Dropping them in the query keeps the response small and the thread readable.
const HIDDEN_TIMELINE_EVENTS: &str = r#"["subscribed","unsubscribed","mentioned","referenced","milestoned","demilestoned","user_blocked","connected","disconnected","transferred","pinned","unpinned","locked","unlocked","marked_as_duplicate","unmarked_as_duplicate","comment_deleted","deployed","deployment_environment_changed","automatic_base_change_succeeded","automatic_base_change_failed"]"#;
```

The filter is a `select` at the top of the program,
`select(.event as $event | ({HIDDEN_TIMELINE_EVENTS} | index($event)) == null)`, so the noise is
dropped inside `gh` before it can consume any of the 2 MiB pipe budget or any record toward the
entry cap. The explicit branches then map each known event shape into the same fixed eight-field
record the comments endpoint produces. A condensed view of the mapping:

| Timeline event | kind | detail | reference |
| --- | --- | --- | --- |
| `commented` | `comment` | empty | empty |
| `reviewed` | `review` | `.state` (the verdict, e.g. `APPROVED`) | review id |
| `committed` | `commit` | `.sha[0:7]` abbreviated | full sha |
| `head_ref_force_pushed` | `force_push` | empty | `.commit_id` |
| `labeled` / `unlabeled` | same word | label name | empty |
| `renamed` | `renamed` | old title | new title |
| `cross-referenced` | `cross_referenced` | `<number> <title>` of the source issue | empty |
| `review_requested` / removed | same word | reviewer login or team name | empty |
| `assigned` / `unassigned` | same word | assignee login | empty |
| `line-commented` / `commit-commented` | `review_comment` per grouped comment | `path:line` | review id |
| anything else | the raw event name | empty | empty |

The `line-commented` branch expands `.comments[]?` into records identical in shape to
`REVIEW_COMMENT_TSV_JQ` output, which is exactly why the merge step can deduplicate by URL: both
sources emit the same `html_url` for the same comment. The fallback `else` branch is the forward
compatibility valve:

```rust
  else
    [(.event // "event"), (.actor.login // .user.login // .author.name // ""), (.created_at // .submitted_at // .author.date // ""), "", "", (.html_url // ""), "", ""]
  end
```

An event type GitHub invents next year still arrives with an actor and a timestamp, parses into
`ConversationKind::Other`, and renders as a dated line instead of breaking the read. The test
`parses_every_conversation_shape_the_query_can_emit` pins this with a `weird_new_event` record and
the assertion message "an event GitHub adds later still renders with its actor and time".

### The kind vocabulary

On the Rust side the wire strings become a closed enum, `ConversationKind`
(`src/git/github/conversation.rs`), with 23 variants: `Opened`, `Comment`, `Review`,
`ReviewComment`, `Commit`, `ForcePush`, `Merged`, `Closed`, `Reopened`, `Labeled`, `Unlabeled`,
`Renamed`, `ReadyForReview`, `ConvertedToDraft`, `ReviewRequested`, `ReviewRequestRemoved`,
`Assigned`, `Unassigned`, `CrossReferenced`, `HeadRefDeleted`, `HeadRefRestored`,
`BaseRefChanged`, and `Other`. Two details of `ConversationKind::parse` are worth noting. First,
`"convert_to_draft" | "converted_to_draft"` both map to `ConvertedToDraft`, because GitHub has
used both spellings for the same event. Second, the catch-all arm maps every unknown string to
`Other` rather than erroring, matching the jq fallback branch above.

The enum also answers one rendering question centrally:

```rust
    /// Whether the entry carries prose worth rendering under its header.
    pub(crate) const fn has_body(self) -> bool {
        matches!(
            self,
            Self::Opened | Self::Comment | Self::Review | Self::ReviewComment | Self::Commit
        )
    }
}
```

A label change or a review request is a one-line header; a comment, review, inline comment,
commit, or the opening description gets its body rendered underneath. Keeping this decision on
the kind rather than on "is the body empty" means an empty comment still renders as a comment and
a lifecycle event with incidental text never renders a body block.

## Reading a stream newest-first

`Repository::conversation_records` (`src/git/github/conversation.rs`) is the function that reads
one stream. Its doc comment is the specification, and every clause in it corresponds to a branch
in the body:

```rust
    /// Read one stream page by page, newest pages first, stopping at the entry
    /// cap. A capped read keeps only the newest pages, so the omitted activity
    /// is genuinely the oldest; an oversized or failed validated first page
    /// degrades to the bounded page loop instead of failing the conversation.
    /// Only a pipe-truncated page prevents caching.
    fn conversation_records(
        &self,
        stream: &ConversationStream,
        error_context: &str,
    ) -> Result<ConversationRecords> {
```

The function returns a private three-field result, `ConversationRecords { entries, truncated,
from_cache }`. Its control flow has five stages.

### Stage 1: the stamped snapshot

```rust
        if let Some(entry) = cache_read(&stream.cache_key, CacheLife::Immutable) {
            let (complete, body) = split_conversation_cache(&entry);
            return Ok(ConversationRecords {
                entries: parse_conversation(body).context(error_context.to_owned())?,
                truncated: !complete,
                from_cache: true,
            });
        }
```

Because the cache key embeds `updated_at`, a hit means the thread has not changed since this exact
snapshot was written, so the entry is read with `CacheLife::Immutable` (any age accepted) and no
network happens at all. The entry's first line is a completeness marker, peeled off by
`split_conversation_cache`; a snapshot written by a capped read replays as `truncated: true`, so
the "entries were dropped" notice survives cache round trips instead of silently vanishing on the
second render. This is stage 5's marker being honored, covered below.

### Stage 2: the validated first page

```rust
        let first = match self.validated_gh(
            &stream.validator_key,
            vec![
                OsString::from(&stream.endpoint),
                OsString::from("--jq"),
                OsString::from(&stream.jq),
            ],
        ) {
            Ok(read) if !read.truncated => Some(read),
            Ok(_) | Err(_) => None,
        };
```

On a stamp miss the reader fetches page 1 through `validated_gh`, sending the stored ETag when one
exists. The match guard is the heart of the hard-fail fix that landed inside #48: a validated read
is accepted only when its stdout was not pipe-truncated, and both a truncated read and an outright
error collapse to `None`, which sends control into the bounded page loop rather than up the error
path. Before this fix, a first page large enough to trip the 2 MiB metadata cap failed the entire
conversation; after it, the same response is simply refetched a page at a time. The corresponding
change inside `validated_gh` (accepting a non-zero exit from a killed-at-cap child, described in
[GitHub pagination on the wire](#github-pagination-on-the-wire)) is the other half of the same
finding.

### Stage 3: the single-page fast path

```rust
        if let Some(read) = &first
            && read.complete
        {
            let entries = parse_conversation(&read.data).context(error_context.to_owned())?;
            cache_write(
                &stream.cache_key,
                &conversation_cache_entry(true, &read.data),
            );
            return Ok(ConversationRecords {
                entries,
                truncated: false,
                from_cache: read.unchanged,
            });
        }
```

`read.complete` means the response was not truncated and advertised no `rel="next"`: the whole
stream fit in one page. This is the overwhelmingly common case for real pull requests, and it
costs exactly one request, or zero rate-limit cost when GitHub answered `304 Not Modified`
(`read.unchanged` becomes the stream's `from_cache`, which the view surfaces as the `cached`
label). The complete page is written under the stamped key with a `complete` marker, so every
subsequent open of the same thread at the same `updated_at` is stage 1's pure disk read.

### Stage 4: the bounded page loop

When the stream spans multiple pages, the reader switches to per-page accumulation:

```rust
        let mut collected: Vec<u8> = Vec::new();
        let mut lines = 0_usize;
        let mut pipe_truncated = false;
        let mut complete = true;
        let (first_data, first_last_page, has_more) = if let Some(read) = first {
            (read.data, read.last_page, true)
        } else {
            let read = self.api_page(&stream.endpoint, &stream.jq, 1, error_context)?;
            pipe_truncated |= read.truncated;
            (read.data, read.last_page, read.has_next)
        };
```

Page 1's bytes come either from the retained validated read (whose `last_page` was parsed from the
same response) or, when the validated read was rejected, from a fresh `api_page` fetch whose
truncation only whole-record-trims the data rather than failing. Then the two strategies split:

```rust
        match (stream.paging, first_last_page.filter(|last| *last >= 2)) {
            (ConversationPaging::LastPageFirst, Some(last)) => {
                for page in (2..=last).rev() {
                    if lines >= MAX_CONVERSATION_ENTRIES {
                        complete = false;
                        break;
                    }
                    let read = self.api_page(&stream.endpoint, &stream.jq, page, error_context)?;
                    pipe_truncated |= read.truncated;
                    append_records(&mut collected, &mut lines, &read.data);
                }
                if complete {
                    append_records(&mut collected, &mut lines, &first_data);
                }
            }
            (ConversationPaging::NewestFirst, _) | (ConversationPaging::LastPageFirst, None) => {
                append_records(&mut collected, &mut lines, &first_data);
                let mut next = has_more.then_some(2_usize);
                while let Some(page) = next {
                    if lines >= MAX_CONVERSATION_ENTRIES {
                        complete = false;
                        break;
                    }
                    let read = self.api_page(&stream.endpoint, &stream.jq, page, error_context)?;
                    pipe_truncated |= read.truncated;
                    append_records(&mut collected, &mut lines, &read.data);
                    next = read.has_next.then(|| page.saturating_add(1));
                }
            }
        }
```

**The timeline arm.** `(LastPageFirst, Some(last))` iterates `(2..=last).rev()`: the final page
first, walking backward toward page 2, newest activity to oldest. Before each fetch the record
counter is checked against `MAX_CONVERSATION_ENTRIES`; tripping it marks the stream incomplete
and stops. Page 1, the oldest chunk, was already in hand from stage 2, and it is appended at the
end only `if complete`. When the cap trips, the oldest page is precisely the data that gets
discarded, despite having been fetched: correctness of the cap's direction is worth one page of
already-spent bytes.

**The comments arm and the degenerate timeline.** The second arm covers two situations. For
review comments (`NewestFirst`) the endpoint itself sorts descending, so the plain forward walk
`2, 3, ...` while `has_next` is still newest-first: later pages are older, and stopping at the cap
again drops only the oldest. The same arm also catches a timeline whose first response named no
usable last page (`(LastPageFirst, None)`, meaning no `rel="last"` or `last_page < 2`). Without a
named last page there is no way to start from the end, so the reader degrades to a bounded
oldest-first walk. In practice GitHub includes `rel="last"` alongside `rel="next"` on these
listings, so the degenerate arm is a defensive posture for a header GitHub is not obligated to
send, not a path multi-page threads normally take.

### Stage 5: parse, cache with a marker, report honestly

```rust
        let entries = parse_conversation(&collected).context(error_context.to_owned())?;
        if !pipe_truncated {
            cache_write(
                &stream.cache_key,
                &conversation_cache_entry(complete, &collected),
            );
        }
        Ok(ConversationRecords {
            entries,
            truncated: pipe_truncated || !complete,
            from_cache: false,
        })
```

Only a pipe truncation prevents caching, because a pipe-truncated page may have lost records in
the middle of the stream's coverage; a merely capped read is a well-defined "newest N" answer and
is cached with the `partial` marker so replays keep saying so. The marker format is two trivial
functions:

```rust
fn conversation_cache_entry(complete: bool, data: &[u8]) -> Vec<u8> {
    let mut entry = Vec::with_capacity(data.len().saturating_add(12));
    entry.extend_from_slice(if complete { b"complete" } else { b"partial" });
    entry.push(b'\n');
    entry.extend_from_slice(data);
    entry
}

fn split_conversation_cache(entry: &[u8]) -> (bool, &[u8]) {
    entry
        .iter()
        .position(|byte| *byte == b'\n')
        .map_or((true, entry), |index| {
            let (marker, rest) = entry.split_at(index);
            (marker == b"complete", rest.get(1..).unwrap_or_default())
        })
}
```

An entry with no newline at all is treated as `(true, entry)`, a forgiving default for a corrupt
or foreign entry that at worst omits the truncation notice. The test
`cache_entries_remember_whether_the_read_was_complete` pins both markers round-tripping.

### The record accounting

The counter that drives the cap counts raw records at the byte level, in `append_records`:

```rust
fn append_records(collected: &mut Vec<u8>, lines: &mut usize, data: &[u8]) {
    if data.is_empty() {
        return;
    }
    let mut added = data.split(|byte| *byte == b'\n').count().saturating_sub(1);
    collected.extend_from_slice(data);
    if collected.last() != Some(&b'\n') {
        collected.push(b'\n');
        added = added.saturating_add(1);
    }
    *lines = lines.saturating_add(added);
}
```

Splitting on `\n` and subtracting one counts newline-terminated records; when an appended chunk
does not end with a newline (a whole-record-trimmed truncation can still leave a final terminator
absent in principle), one is pushed so the next chunk cannot fuse with the tail record, and the
tail counts as one more record. The test `appended_records_always_end_on_a_record_boundary`
asserts that appending `b"a\tb\nc\td"` then `b"e\tf\n"` yields exactly `b"a\tb\nc\td\ne\tf\n"`
with `lines == 3`, with the message "an unterminated tail still counts as one record".

Two properties of this accounting matter for the cap's semantics. First, the check `lines >=
MAX_CONVERSATION_ENTRIES` happens before each fetch, so a stream can overshoot 500 by up to one
page of 100: the fetch that carries the counter past the cap is never itself interrupted. Second,
each of the two streams is bounded independently near 500 raw records, before deduplication and
before the merged thread's own exact-500 cap. The final cap in `pull_request_conversation`
(covered below) is what produces the precise entry count; the per-stream cap is a bandwidth bound
that keeps either endpoint from consuming unbounded pages.

### A worked example: the 12-page timeline

Take a thread shaped like bun#30412's: roughly 1,150 timeline records at 100 per page, so GitHub
advertises `rel="last"` naming page 12 (11 full pages plus a 50-record tail). The read proceeds:

```text
step  action                          lines before  lines after  note
----  ------------------------------  ------------  -----------  -------------------------------
 0    validated read of page 1              -             -      oldest 100 records held aside
 1    fetch page 12                         0            50      newest tail page
 2    fetch page 11                        50           150
 3    fetch page 10                       150           250
 4    fetch page 9                        250           350
 5    fetch page 8                        350           450
 6    fetch page 7                       450           550      check passed at 450, page adds 100
 7    check before page 6: 550 >= 500       -             -      complete = false, stop
 8    page 1 NOT appended                   -             -      the oldest 100 are the drop
```

Seven requests instead of twelve, 550 records collected, all of them the newest 550 the endpoint
holds, and the discarded data is exactly the oldest. The evidence comment's "~12 bounded
100-entry pages" for the 26-second cold read counts both streams' pages together; the point is
that each request is individually bounded to one 100-record page, so no single invocation can
blow the 2 MiB pipe and the read's total cost scales with the cap, not with the thread.

## The conversation record format

Everything between `gh` and the renderer travels as tab-separated records, eight fields wide, one
record per line. The width is a named constant checked at parse time:

```rust
const CONVERSATION_FIELDS: usize = 8;
```

### Field layout

Every record, from either stream and every timeline branch, has the same shape:

| # | Field | Content |
| --- | --- | --- |
| 1 | `kind` | Wire string mapped by `ConversationKind::parse` |
| 2 | `actor` | Login or author name, empty when unknown |
| 3 | `timestamp` | RFC 3339 UTC string straight from GitHub |
| 4 | `detail` | Header qualifier: verdict, label, `path:line`, abbreviated commit, old title |
| 5 | `body` | Prose, bounded to 64 KiB |
| 6 | `url` | `html_url` of the underlying object, empty for events without one |
| 7 | `reference` | Stable identity: commit OID, review id, or post-rename title |
| 8 | `context` | Supporting text, currently a review comment's diff hunk, bounded to 8 KiB |

A concrete inline review comment record, with tabs written as `\t` for legibility (the real bytes
are `0x09`):

```text
review_comment \t reviewer \t 2026-08-01T11:00:01Z \t src/main.rs:42 \t Extract this \t https://example.test/rc/1 \t 99 \t @@ -1 +1 @@ \n
```

### TSV escaping and the parser

jq's `@tsv` filter escapes the four characters that would break the framing: a literal tab becomes
`\t`, a newline `\n`, a carriage return `\r`, and a backslash `\\`. That makes the framing
unambiguous at the byte level: a raw `0x09` is always a field separator and a raw `0x0a` is
always a record separator, no matter what a comment body contains. The shared parser
`parse_tsv_record::<FIELDS>` (`src/git/github/mod.rs`) strips a trailing `\r` (defensive against a
CRLF-mangled pipe), splits on raw tabs, unescapes each field with `unescape_tsv`, and converts the
field list into a fixed-size array, erroring with "expected {FIELDS} tab-separated fields,
received {n}" when the count is wrong. The conversation parser wraps it per record:

```rust
fn parse_conversation(output: &[u8]) -> Result<Vec<ConversationEntry>> {
    let mut entries = Vec::new();
    for (index, record) in output.split(|byte| *byte == b'\n').enumerate() {
        if record.is_empty() {
            continue;
        }
        let [
            kind,
            actor,
            timestamp,
            detail,
            body,
            url,
            reference,
            context,
        ] = parse_tsv_record::<CONVERSATION_FIELDS>(record)
            .with_context(|| format!("invalid conversation record {}", index + 1))?;
        entries.push(ConversationEntry {
            kind: ConversationKind::parse(&kind),
            actor,
            timestamp,
            detail,
            body: bounded_text(&body, MAX_CONVERSATION_BODY_BYTES),
            url,
            reference,
            context: bounded_text(&context, MAX_CONVERSATION_CONTEXT_BYTES),
        });
    }
    Ok(entries)
}
```

The error context carries a 1-based record number, so a malformed record in a 500-line stream is
locatable. The test `parses_every_conversation_shape_the_query_can_emit` feeds seven records
covering every interesting shape, including a body containing the escaped sequence `\\n` and
asserting it unescapes into a real newline ("Looks good to me\nship it"), and
`rejects_records_that_do_not_match_the_query_shape` asserts a three-field record errors rather
than being padded or silently skipped.

### The per-entry byte caps

Two caps are applied per entry at parse time:

```rust
const MAX_CONVERSATION_BODY_BYTES: usize = 64 * 1024;
const MAX_CONVERSATION_CONTEXT_BYTES: usize = 8 * 1024;
```

A 64 KiB body is far beyond any comment a human intends to read in a terminal pane, and an 8 KiB
context comfortably holds any diff hunk GitHub attaches to an inline comment; both caps exist so
one pathological entry (a comment someone pasted a log into, a hunk from a generated file) cannot
make the renderer's per-frame wrapping work unbounded. The truncation itself is UTF-8-safe,
`bounded_text` in `src/git/github/mod.rs`:

```rust
fn bounded_text(value: &str, maximum: usize) -> String {
    if value.len() <= maximum {
        return value.to_owned();
    }
    let mut end = maximum;
    while !value.is_char_boundary(end) {
        end = end.saturating_sub(1);
    }
    format!("{}…", value.get(..end).unwrap_or_default())
}
```

The cut point walks backward to a character boundary so a multi-byte character is never split,
and a single ellipsis character is appended so a truncated body is visibly truncated rather than
just oddly short.

### The entry type

The parsed record becomes `ConversationEntry` (`src/git/github/conversation.rs`), whose field doc
comments document the vocabulary the renderer works with:

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ConversationEntry {
    pub kind: ConversationKind,
    pub actor: String,
    pub timestamp: String,
    /// Short qualifier for the header: a review verdict, a label name, a
    /// `path:line` anchor, an abbreviated commit, or a renamed title.
    pub detail: String,
    pub body: String,
    pub url: String,
    /// Stable identity for the underlying object: a commit OID, a review id, or
    /// the post-rename title.
    pub reference: String,
    /// Supporting text shown with the entry, currently a review comment's hunk.
    pub context: String,
}
```

The type derives `Serialize` because the same structure feeds the `quinjet pr conversation`
subcommand's `--json` output; one parse serves both the terminal pane and the machine interface,
which is the single-vocabulary principle of invariant 1a applied to a read.

## Flattening into one ordered thread

`pull_request_conversation` merges the two streams into the single flat thread the pane renders.
The merge has four moves: seed, extend, deduplicate, sort. Then the cap is applied with one
carefully ordered splice.

### The synthetic opened entry

The thread always begins with an entry no endpoint serves: the pull request's own opening, built
locally from metadata already in hand:

```rust
fn opened_entry(pull_request: &PullRequest) -> ConversationEntry {
    ConversationEntry {
        kind: ConversationKind::Opened,
        actor: pull_request.author.clone(),
        timestamp: pull_request.created_at.clone(),
        detail: format!("{} into {}", pull_request.head_ref, pull_request.base_ref),
        body: pull_request.description.clone(),
        url: pull_request.url.clone(),
        reference: pull_request.head_oid.clone(),
        context: String::new(),
    }
}
```

Its detail reads like `feature-branch into main`, its body is the pull request description, and
its timestamp is `created_at`, which precedes every timeline event by construction. Synthesizing
it costs zero requests and guarantees the thread has a head even when both streams are empty.

### Merge and dedupe

```rust
        let from_cache = timeline.from_cache && comments.from_cache;
        let mut entries = vec![opened_entry(pull_request)];
        entries.extend(timeline.entries);
        for comment in comments.entries {
            if entries
                .iter()
                .any(|entry| !entry.url.is_empty() && entry.url == comment.url)
            {
                continue;
            }
            entries.push(comment);
        }
        entries.sort_by(|left, right| left.timestamp.cmp(&right.timestamp));
```

The dedupe exists because the timeline's `line-commented` groups emit inline comments the
dedicated endpoint also serves. Both sources carry the same `html_url` for the same comment, so a
review comment is skipped when any existing entry already holds its URL. The guard
`!entry.url.is_empty()` matters: plenty of timeline events (force pushes, label changes) have no
URL, and without the guard the first review comment whose URL happened to be empty would collide
with them. Only a non-empty URL can testify to identity.

`from_cache` is the conjunction of both streams: the thread claims to have come from cache only
when neither stream transferred anything, matching the doc comment on
`PullRequestConversation::from_cache`: "True when nothing had to be transferred: either the
thread was already held for this update stamp, or GitHub confirmed it had not changed."

### Sorting strings as instants

The sort compares timestamps as plain strings. That is correct, not approximate, because of a
format property the notes for both modules state up front: every timestamp in the conversation
and checks paths is an RFC 3339 UTC string straight from GitHub, of the form
`2026-08-01T11:00:00Z`. In that fixed-width, most-significant-field-first layout, lexicographic
byte order equals chronological order, so `left.timestamp.cmp(&right.timestamp)` is a correct
comparator with zero parsing. The sort is Rust's stable sort, so entries sharing a timestamp
(a review and its first inline comment often share a second) keep their merge order: timeline
context first, then the appended comments, which reads naturally in the pane.

The one place string comparison is deliberately NOT used is log-line-to-step attachment, where
sub-second precision meets whole-second precision and text order gives the wrong answer; that
inversion is the subject of
[Attaching lines to steps by whole seconds](#attaching-lines-to-steps-by-whole-seconds).

### The cap that can only drop the oldest

```rust
        let overflowing = entries.len() > MAX_CONVERSATION_ENTRIES;
        if overflowing {
            let opened = entries.remove(0);
            let dropped = entries.len() - (MAX_CONVERSATION_ENTRIES - 1);
            drop(entries.drain(..dropped));
            entries.insert(0, opened);
        }
        let truncated = timeline.truncated || comments.truncated || overflowing;
```

The opened entry sorted to index 0 (creation precedes all activity), so it is lifted out first,
the oldest `dropped` real entries are drained from the front, and the opened entry is reinserted
at index 0. Worked arithmetic: suppose the merged, sorted thread holds 553 entries including the
opened one. `overflowing` is true. Removing the opened entry leaves 552. The dropped count is
`552 - (500 - 1) = 53`, so the 53 oldest real entries are drained, leaving 499. Reinserting the opened entry
makes exactly 500. Three properties fall out:

- The result is exactly `MAX_CONVERSATION_ENTRIES` entries, never approximately.
- The opened entry always survives, so the thread always says who opened the pull request, from
  which branch, with what description.
- Every dropped entry is older than every kept one (the opened entry aside), because the drain
  takes a prefix of an ascending sort that was fed newest-first data by both streams.

The doc comment on the constant explains why 500 is the number and what the cap actually protects:

```rust
/// The renderer wraps every entry to the pane width on each redraw, so this cap
/// is what keeps that work bounded. It is far above any real thread; the entries
/// dropped are the oldest, and the view says so.
const MAX_CONVERSATION_ENTRIES: usize = 500;
```

The cap is a rendering budget, not a network budget: wrapped-row layout is rebuilt when the pane
width or content changes (see [../rendering/viewport.md](../rendering/viewport.md) for the row
cache that makes redraws cheap), and 500 wrapped entries is the ceiling on that rebuild.

`truncated` ORs three flags: either stream's bounded read stopping early, either stream's pipe
truncation, or the merged cap overflowing. The view renders one honest notice for all three,
visible in the PR #48 evidence transcript against bun#30412:

```console
$ quinjet pr conversation 30412 | tail -4
@robobun referenced this in 39158 Blob: drop is_bun_file, a duplicate of needs_to_read_file  (Sat Aug 15 10:55 PM)
@andreiujica referenced this in 39566 `bun install` from a workspace member fails to resolve `workspace:*` ...  (Tue Aug 18 9:41 PM)
[the conversation reached Quinjet's entry cap and older entries were dropped]
```

The tail of the output is the latest activity, and the notice sits at the boundary where history
was cut, which is the visible proof of the newest-first inversion: before #48 the same command's
tail would have been mid-2025 activity with the newest three months missing and no line saying so.

### Derived counts

One small consumer worth noting: the sidebar's conversation row shows a comment count, computed
from the flattened thread rather than from a separate API call:

```rust
    pub(crate) fn comment_count(&self) -> usize {
        self.entries
            .iter()
            .filter(|entry| {
                matches!(
                    entry.kind,
                    ConversationKind::Comment | ConversationKind::ReviewComment
                )
            })
            .count()
    }
```

Only prose comments count; reviews, commits, and lifecycle events do not inflate the number. On a
capped thread this is necessarily a count of the retained 500, which is consistent with what the
pane can actually show.

## The conversation in the terminal

The read path above runs on a worker thread. What the reader experiences is governed by the app
state machinery in `src/app.rs` and the renderer in `src/ui/mod.rs`, and #48 added the last piece
of it: a way to reach the bottom of the thread instantly.

### Its own lane, its own generation

`LoadPullRequestConversation` is routed to a dedicated worker lane, `WorkerLane::Conversation`,
with its own OS thread (`quinjet-conversation`) and its own coalescing mailbox slot. The routing
table in `src/git/worker.rs` shows the isolation:

```rust
const fn worker_lane(command: &WorkerCommand) -> WorkerLane {
    match command {
        WorkerCommand::PrepareLocalDiff { .. } | WorkerCommand::LoadLocalDiffFile { .. } => {
            WorkerLane::LocalPreview
        }
        WorkerCommand::LoadGitHubRepositories { .. }
        | WorkerCommand::LookupPullRequest { .. }
        | WorkerCommand::LoadPullRequestChecks { .. }
        | WorkerCommand::LoadCheckRunLog { .. } => WorkerLane::GitHubMetadata,
        WorkerCommand::LoadPullRequestConversation { .. } => WorkerLane::Conversation,
        WorkerCommand::PrefetchCheckRunLogs { .. } => WorkerLane::Warm,
        WorkerCommand::PreparePullRequest { .. }
        | WorkerCommand::LoadPullRequestFile { .. }
        | WorkerCommand::LoadPullRequestFileBatch { .. } => WorkerLane::PullRequestPreview,
        _ => WorkerLane::Background,
    }
}
```

The reason the conversation is not on the metadata lane is the 26-second cold read: a multi-page
conversation walk is the slowest read in the pull request view, and invariant 3 requires that "a
large conversation cannot block interactive check logs". On its own lane, a reader who opens
bun#30412 sees checks, metadata, and the file index land while the conversation is still paging.
The lane model as a whole is documented in
[../rendering/concurrency.md](../rendering/concurrency.md).

Replies are generation-gated. `request_pull_request_conversation` (`src/app.rs`) bumps
`pull_request_conversation_generation` before issuing, and the `WorkerEvent::PullRequestConversation`
handler drops any reply whose generation does not match, so a slow reply for a pull request the
reader already left can never install itself. The request function also coalesces:

```rust
    fn request_pull_request_conversation(&mut self, refresh: bool, effects: &mut Vec<AppEffect>) {
        if self.pull_request_conversation_loading {
            self.pull_request_conversation_refresh_again |= refresh;
            return;
        }
        if !refresh && !self.pull_request_conversation.entries.is_empty() {
            return;
        }
```

A refresh requested while a load is in flight sets one boolean; when the reply lands, the handler
replays exactly one follow-up request. Any number of poll ticks during a slow page walk collapse
into a single trailing refresh instead of a queue. And a non-refresh request against entries
already held is free: merely re-entering the Overview section costs nothing.

The reply handler applies one more economy: it replaces state only when the conversation actually
changed (`error was set || entries empty || conversation != stored`), and only then invalidates
the pull-request content row cache. An adaptive poll that finds nothing new therefore does not
rebuild the wrapped rows, which is the #46 invalidation discipline described in
[../rendering/viewport.md](../rendering/viewport.md) doing its job on this stream.

### Jump to bottom

A newest-first thread makes the bottom the interesting end, so #48 also added the control that
takes the reader there. The action handler in `src/app.rs` is two lines:

```rust
            ScmAction::JumpToBottom => {
                self.set_focus(Focus::Content, effects);
                self.content_scroll = usize::MAX;
            }
```

`usize::MAX` is the "bottom" idiom shared with the log-follow path: the app never knows the row
count (the renderer owns layout), so it asks for an impossible scroll and lets the next draw clamp
it to the real maximum. The renderer records the outcome of that clamp in one field whose doc
comment in `src/app.rs` explains the division of labor: "Whether the last draw left the content
pane scrolled to its end. The renderer owns the row count, so it reports this back for the one
decision that needs it: whether a growing log should keep following." Both content paths set it,
`app.content_at_bottom = app.content_scroll == max_scroll` on the overview path
(`src/ui/mod.rs:2279`) and `>= max_scroll` on the document path (`src/ui/mod.rs:3332`).

The clickable control itself, `draw_jump_to_bottom` (`src/ui/mod.rs`):

```rust
/// A clickable shortcut to the end of whatever the content pane holds, shown on
/// its bottom border whenever the reader is not already there. On a huge diff
/// or conversation it replaces paging through thousands of rows.
fn draw_jump_to_bottom(
    frame: &mut Frame<'_>,
    content: Rect,
    app: &App,
    theme: &Theme,
) -> Option<ScmActionHit> {
    if app.content_at_bottom || app.modal.is_some() || content.width < 20 || content.height < 3 {
        return None;
    }
    let label = " ↓ Bottom ";
```

It renders as a bold accent-colored label on the content pane's bottom border, right-aligned, and
returns a hit rectangle that plugs into the existing `ScmAction` mouse dispatch, so the mouse path
and the keyboard path converge on the same two-line handler. It hides itself when already at the
bottom, when a modal owns the screen, or when the pane is too small for the label to make sense.
The test `a_scrollable_content_pane_offers_a_jump_to_bottom_control` draws a 200-line document on
a 100x24 test backend, asserts the hit exists, then sets `content_scroll = usize::MAX` and asserts
the hit disappears after the redraw.

## Check runs: the list

The checks surface begins with the list: every check run attached to the pull request's head
commit, rendered in the Overview sidebar grouped by status. This read has three unusual
properties, and each shaped the code.

### Recognizing an answer by content, not exit status

The list is read through `gh pr checks`, and that command encodes check outcomes in its own exit
status. The doc comment on `Repository::pull_request_checks` (`src/git/github/checks.rs`) states
the consequence:

```rust
    /// `gh pr checks` exits non-zero when any run failed, so a useful response
    /// has to be recognized by its content rather than by the exit status. That
    /// is why this reads `gh` directly instead of going through the cached
    /// helper, and caches the accepted body itself.
```

The acceptance predicate:

```rust
        let output = self.run_gh(pull_request_checks_args(pull_request))?;
        let accepted_status = output.status.success()
            || matches!(output.status.code(), Some(1 | 8)) && !output.stdout.is_empty();
        if output.stdout_truncated {
            bail!("pull-request checks exceeded the metadata limit");
        }
```

`gh` uses exit 1 when checks failed and exit 8 when checks are pending; both are answers about
the pull request, not failures of the command, so they are accepted whenever a body came with
them (`&&` binds tighter than `||`, so the non-empty-stdout requirement applies only to the
non-zero codes). A response that tripped the 2 MiB metadata pipe is refused outright with
"pull-request checks exceeded the metadata limit" rather than parsed as a plausible-looking
partial list. A genuinely rejected status gets one more content check: a stderr containing
"no checks" (case-insensitive) returns an empty `PullRequestChecks::default()` instead of an
error, because a pull request without CI is a normal state, not a failure to report.

The command itself, built argv-direct as always (invariant 7):

```rust
fn pull_request_checks_args(pull_request: &PullRequest) -> Vec<OsString> {
    vec![
        OsString::from("pr"),
        OsString::from("checks"),
        OsString::from(pull_request.number.to_string()),
        OsString::from("--repo"),
        OsString::from(pull_request.base_repository.selector()),
        OsString::from("--json"),
        OsString::from("bucket,completedAt,description,link,name,startedAt,state,workflow"),
        OsString::from("--jq"),
        OsString::from(CHECK_TSV_JQ),
    ]
}
```

with the same TSV reduction pattern as the conversation:

```rust
const CHECK_TSV_JQ: &str = r#".[] | [.name, .workflow, .state, .bucket, (.description // ""), (.link // ""), (.startedAt // ""), (.completedAt // "")] | @tsv"#;
```

### The one clock-based cache

The check list is the single entry on this whole page whose cache runs on a clock:

```rust
/// Check state is the one thing here that genuinely changes minute to minute,
/// so it is the one thing kept on a clock rather than on an identity.
const CHECK_LIST_CACHE_TTL: Duration = Duration::from_secs(30);
```

The key is `checks-v1\n{base url trimmed}\n{number}\n{head_oid}`: keyed to the head commit, so a
force push asks a different question rather than aging an old answer, but within one head the
answer legitimately changes as runs start and finish, hence the 30-second TTL. A read with
`refresh: false` (arriving in the section, redrawing) is served from a fresh-enough entry with
`from_cache: true`; the adaptive poll passes `refresh: true` and always goes to the network. This
split is the boundary case of the cache taxonomy in [./caching.md](./caching.md): everything else
on this page is either immutable by key construction or never cached at all.

One side effect of the short TTL is recorded in `src/app.rs` on
`pull_request_served_from_cache`, the predicate behind the view's `cached` label: "Check state is
deliberately held for only thirty seconds, so including it here made the answer almost always
false and the label never appeared at all." The label therefore considers only the pull request
snapshot and the conversation, both of which have meaningful cache lifetimes.

### Bucket mapping and stable order

`parse_pull_request_checks` maps `gh`'s `bucket` field (its own normalization of GitHub's
status/conclusion pairs) into the app's status enum:

| `bucket` value | `PullRequestCheckStatus` |
| --- | --- |
| `pass` | `Passed` |
| `fail` | `Failed` |
| `pending` | `Pending` |
| `skipping` | `Skipped` |
| `cancel` | `Cancelled` |
| anything else | `Unknown` |

and finishes with an ordering decision that matters more than it looks:

```rust
    checks.sort_by_key(|check| (check.workflow.to_lowercase(), check.name.to_lowercase()));
```

GitHub's API order for check runs is not stable across polls. Without this sort, every 5-second
poll while a run executes could reorder the sidebar under the reader's cursor. Sorting
case-insensitively by workflow then name makes the list a pure function of its contents, so an
unchanged set of checks renders identically regardless of arrival order; the test
`parses_live_pull_request_checks_in_stable_name_order` pins it by feeding records out of order
and asserting the sorted result.

The `is_running` predicate that gates most of the tailing machinery is one line on the status:

```rust
    pub(crate) const fn is_running(self) -> bool {
        matches!(self, Self::Pending)
    }
```

### Extracting the job identity

Everything the log machinery does needs a GitHub Actions job id, and a check run exposes it in
exactly one place:

```rust
    /// GitHub Actions check links end in `/actions/runs/<run>/job/<job>`, which
    /// is the only place a check run exposes the job identity its logs need.
    pub(crate) fn job_id(&self) -> Option<u64> {
        let (_, job) = self.link.rsplit_once("/job/")?;
        let job = job.split(['?', '#', '/']).next()?;
        job.parse().ok()
    }
```

`rsplit_once` takes the last `/job/` segment (defensive against a hypothetical path containing an
earlier one), and the split on `['?', '#', '/']` trims query strings, fragments, and trailing path
segments before parsing. The tests pin `.../runs/123/job/456` and `.../job/456?pr=7` both
yielding 456, and a non-Actions link (external CI) or an empty link yielding `None`, which is the
signal that this check has no readable log.

Built on top of it, `identity()` produces the stable string used for selection preservation and
warm-up dedupe: the job id as a decimal string when there is one, else the link, else (for a
check with no link at all) `"{workflow}\n{name}\n{started_at}"`. The checks reply handler in
`src/app.rs` re-finds the previously selected check by `identity()` after every refresh, so the
stable sort plus the stable identity together mean a poll can never silently move the reader's
selection to a different run.

## Actions job steps

Selecting a check whose link names an Actions job replaces the content pane with that run's steps
and log. The steps come from the Actions jobs endpoint,
`repos/{repository}/actions/jobs/{job}`, reduced server-side by the second jq program in
`src/git/github/checks.rs`:

```rust
const JOB_STEPS_TSV_JQ: &str = r#".steps[]? | [((.number // 0)|tostring), (.name // ""), (.status // ""), (.conclusion // ""), (.started_at // ""), (.completed_at // "")] | @tsv"#;
```

The `?` in `.steps[]?` makes a job document without a `steps` array produce zero records instead
of a jq error, and every field is defended with `//` as usual. The six fields land in
`CheckStep { number, name, status, conclusion, started_at, completed_at, lines }`, with `lines`
empty until log attachment fills it.

### Deriving a status from two fields

The jobs API reports a step's state as a `status` string (`queued`, `in_progress`, `completed`)
plus a `conclusion` string that is meaningful only once completed. The app folds both into one
enum with a rule that reads exactly like GitHub's documentation:

```rust
    fn from_conclusion(status: &str, conclusion: &str) -> Self {
        if !status.eq_ignore_ascii_case("completed") {
            return Self::Pending;
        }
        match conclusion.to_ascii_lowercase().as_str() {
            "success" => Self::Passed,
            "failure" | "timed_out" | "action_required" => Self::Failed,
            "skipped" | "neutral" => Self::Skipped,
            "cancelled" | "stale" => Self::Cancelled,
            _ => Self::Unknown,
        }
    }
```

Anything not yet completed is `Pending`, which is also what `is_running()` matches, so a step and
a whole check share one vocabulary. `timed_out` and `action_required` counting as failures, and
`neutral` counting as skipped, follow the semantics GitHub assigns those conclusions in the
checks UI.

### Order and resilience

`parse_check_steps` finishes with `steps.sort_by_key(|step| step.number)`, so steps render in the
runner's own numeric order regardless of the API's array order; the test
`parses_job_steps_and_derives_status_from_the_conclusion` feeds records numbered 1, 3, 2 and
asserts they come out 1, 2, 3, and additionally that an `in_progress` step with an empty
conclusion parses as `Pending`. A step whose number field fails to parse defaults to
`index + 1` (`number.parse().unwrap_or(index + 1)`), preserving relative order instead of
erroring on one malformed record.

The caller treats the whole steps read as optional decoration:

```rust
        match response {
            Err(_) => Ok(Vec::new()),
            Ok(response) => parse_check_steps(&response.data),
        }
```

If the jobs endpoint fails, the log still renders, all of it as loose lines. A log without step
grouping is degraded; a step list without a log is normal (the pre-blob window); an error page in
place of either would be strictly worse than both.

### Steps ride the cached metadata helper

Unlike the check list, the steps read goes through the shared `checked_cached_gh` helper
(`src/git/github/mod.rs`), which bounds the response at the 2 MiB metadata cap, serves a fresh
cache entry when the entry's age passes `life.accepts(age)`, falls back to a stale entry when the
network call itself errors (a steps list from thirty seconds ago beats an error), and writes back
only successful, non-truncated responses. The cache key has one subtlety:

```rust
        let response = self.checked_cached_gh(
            &format!("check-steps-v1\n{repository}\n{job}\n{life:?}"),
            life,
            false,
            [
```

The `{life:?}` suffix embeds the Debug rendering of the cache life into the key itself. A settled
job's steps are read with `CacheLife::Immutable` and keyed with the literal text `Immutable`; a
running job's steps are read with `CacheLife::Ttl(Duration::ZERO)` and keyed with `Ttl(0ns)`. The
two families can never collide, so the moment a job settles, its first settled read starts a
fresh immutable entry rather than adopting whatever partial steps snapshot the running-phase key
last held. The `Ttl(0ns)` entry is written but never fresh (zero TTL accepts no age), existing
only to satisfy the helper's shape; its one real use is the stale-on-error fallback while a job
runs.

## The raw log: fetching, tailing, and 404 before the blob

The log itself comes from `repos/{repository}/actions/jobs/{job}/logs`. Understanding this
endpoint's behavior over a job's lifetime is the key to the whole tailing design, and the doc
comment on `pull_request_check_log` compresses it into four sentences:

```rust
    /// Read a check run's steps and its raw log, then attach every log line to
    /// the step whose run window contains it. Runner output is timestamped in
    /// UTC and the steps API reports the same clock, so the ranges map exactly
    /// without guessing at group headings.
    ///
    /// The log endpoint serves whatever a running job has written so far, so
    /// repeating this call while a job runs is what makes the view tail it. Only
    /// the first seconds of a job answer 404, before the blob exists at all.
    pub(crate) fn pull_request_check_log(
```

### The endpoint's lifecycle

The log endpoint's observable behavior over a job's life has three phases, and the code has a
distinct answer for each:

| Phase | Endpoint behavior | Quinjet behavior |
| --- | --- | --- |
| First seconds of a job | 404, the log blob does not exist yet | `log_pending: true`, steps render alone |
| Job running, blob exists | Serves the partial log written so far | Re-read on every request, never cached |
| Job settled | Serves the complete log, immutable | Cached once, never re-read |
| Retention expired | 410 Gone | Empty log, steps still render |

The 404/410 handling is a deliberate non-error, `log_not_published`:

```rust
/// GitHub answers the log endpoint with 404 until a job has finished writing
/// its archive, and with 410 once retention expires. Neither is a failure worth
/// showing: the run itself is still readable from its steps.
fn log_not_published(output: &BoundedOutput) -> bool {
    let error = String::from_utf8_lossy(&output.stderr).to_ascii_lowercase();
    ["404", "410", "not found", "gone"]
        .into_iter()
        .any(|marker| error.contains(marker))
}
```

Both the numeric status and its phrase are matched because `gh` renders errors in more than one
shape. The test `an_unpublished_log_is_pending_rather_than_a_failure` pins `gh: HTTP 404` and
`gh: Gone (HTTP 410)` as non-errors ("a job that has not finished writing its archive is not an
error", "expired retention is not either") and `gh: HTTP 500 Internal Server Error` as a real
one. When the predicate matches, `check_run_raw_log` returns `(Vec::new(), false)`: an empty log,
not an error, and the caller distinguishes the two empty-log meanings. If steps are also empty,
the result is `CheckRunLog::unavailable("GitHub has not published anything for this check
yet")`. If steps exist, the result carries `log_pending: raw.is_empty()`, whose doc comment nails
the window: "The runner has not written anything yet. This is only true for the first seconds of
a job: GitHub serves a growing partial log from then on, so a running job tails rather than
waiting for its own completion."

A check that never had a log path at all short-circuits before any of this: no job id in the
link means `CheckRunLog::unavailable("{name} does not publish logs through GitHub Actions")`,
which is what an external CI provider's check renders.

### Cache life selects the mode

One `if` chooses between tailing and archival:

```rust
        let life = if check.status.is_running() {
            CacheLife::Ttl(Duration::ZERO)
        } else {
            CacheLife::Immutable
        };
```

This single expression implements the invariant 12 clause "a run still in progress is never
cached, because re-reading it is what tails it". With `Ttl(Duration::ZERO)` no cached answer is
ever fresh, so every request refetches both steps and the growing log; that IS the tail, no
streaming protocol required. With `Immutable`, `check_run_raw_log` tries the disk first:

```rust
        let key = format!("check-log-v1\n{repository}\n{job}");
        if life == CacheLife::Immutable
            && let Some(cached) = super::cache_read_bounded(&key, life, MAX_CHECK_LOG_BYTES)
        {
            return Ok((cached, false));
        }
```

and after a successful network read writes back under the same conditions:

```rust
        if life == CacheLife::Immutable && !output.stdout_truncated && !output.stdout.is_empty() {
            super::cache_write_bounded(&key, &output.stdout, MAX_CHECK_LOG_BYTES);
        }
        Ok((output.stdout, output.stdout_truncated))
```

A settled run's log is written once and answered from disk forever after; a running job's partial
blob never touches the cache, so there is no stale-partial-log state to reason about. The key
names the job alone (no `life:?` here) because only immutable reads ever touch it. Both the read
and the write are bounded by the same constant as the fetch:

```rust
const MAX_CHECK_LOG_BYTES: usize = 8 * 1024 * 1024;
```

so a poisoned oversized cache entry cannot smuggle more than 8 MiB back into memory either.

### The escape-sequence flag and its retry

The fetch itself is `gh api --allow-escape-sequences {endpoint}` through `run_gh_log`, which is
`run_gh_bounded(args, MAX_CHECK_LOG_BYTES)`: the raw-log read is the one `gh` invocation allowed
8 MiB instead of 2 MiB. The flag exists because `gh` guards its output against raw terminal
escape sequences, and runner logs are full of ANSI color; Quinjet wants the bytes untouched so
its own stripper (next section) can handle them under its own rules. Old `gh` versions do not
know the flag, so the call self-heals:

```rust
        let output = if output.status.success() || !rejects_unknown_flag(&output) {
            output
        } else {
            self.run_gh_log([OsString::from("api"), OsString::from(&endpoint)])?
        };
```

`rejects_unknown_flag` checks stderr for "unknown flag"; only that specific failure triggers the
one retry without the flag. Any other failure proceeds to the 404/410 classification above.

A log larger than 8 MiB comes back with `stdout_truncated: true` from the kill-at-cap pipe, flows
into the result's `truncated` flag, and is not cached (a truncated archive is not the immutable
answer). The reader sees the first 8 MiB with a truncation notice. Runner logs read top-down,
provisioning first, so the retained prefix still shows the job's structure, its early steps, and
usually the window where a failure began.

### How the tail actually ticks

The re-read loop lives in app state, not in the repository layer. Three cooperating pieces in
`src/app.rs`:

**1. The poll floor.** While the selected check `is_running()`, the adaptive poll re-requests the
log on its own 8-second floor, `PULL_REQUEST_LOG_POLL`, whose comment reads: "A running job's log
grows continuously, so this is a tail interval rather than a staleness bound." The floor is
per-run: switching selection resets `pull_request_log_read_at` to `None`, so a newly selected
running job reads immediately rather than inheriting the previous run's clock.

**2. The checks piggyback.** When a checks snapshot arrives and the selected check `was_running`
before the update, the handler immediately issues `request_check_run_log(true, ...)`
(`src/app.rs:3374-3376`), so step transitions (a step finishing, the next starting) land together
with the check state that announced them.

**3. In-place update.** `request_check_run_log` distinguishes a selection change from a refresh
of the same run by comparing `check.identity()` against the stored target. Its doc comment: "A
selection change starts from a clean slate; a live refresh of the same run updates in place so
the reader keeps their scroll position while a job is still writing output. A log already held
for the selected run is only re-read when `refresh` asks for it, so redrawing or re-entering the
section costs nothing." A different identity clears the log, the error, and the expanded-step
set, and bumps the generation so an in-flight reply for the previous run is dropped rather than
rendered under the new run's name.

Follow-the-tail is decided at reply time, before the new log is installed:

```rust
                let following = self.content_at_bottom
                    && self
                        .selected_pull_request_check()
                        .is_some_and(|check| check.status.is_running());
```

and applied after: `if following { self.content_scroll = usize::MAX; }`. The view sticks to the
newest output only when the reader was already at the end of a running job's log; a reader who
scrolled up to study an earlier step stays exactly where they are while new output accumulates
below. A finished run never auto-follows. This is the invariant 11a sentence "The view follows
the newest output only while the reader is already at the end" in code, and the test at
`src/app.rs` pins it as "a running log follows its own tail unless the reader scrolled up".

## Parsing runner output

The blob that arrives is not display-ready. GitHub Actions runner logs are one timestamped line
per row, salted with ANSI color and `##[...]` workflow commands. `parse_check_log`
(`src/git/github/checks.rs`) reduces each raw line to the triple the renderer wants:

```rust
/// Runner logs are one timestamped line per row, carrying ANSI color and
/// `##[...]` workflow commands. Both are stripped here so the renderer only
/// deals with text plus a severity.
fn parse_check_log(raw: &[u8]) -> (Vec<CheckLogLine>, bool) {
    let text = String::from_utf8_lossy(raw);
    let text = text.strip_prefix('\u{feff}').unwrap_or(&text);
    let mut lines = Vec::new();
    let mut limit_reached = false;
    for raw_line in text.lines() {
        if lines.len() >= MAX_CHECK_LOG_LINES {
            limit_reached = true;
            break;
        }
        let (timestamp, rest) = split_log_timestamp(raw_line);
        let rest = strip_ansi(rest);
        let (severity, text) = split_log_marker(&rest);
        lines.push(CheckLogLine {
            timestamp: timestamp.to_owned(),
            text,
            severity,
        });
    }
    (lines, limit_reached)
}
```

Four decoding stages, in order.

### Stage 1: lossy UTF-8 and the BOM

`from_utf8_lossy` means an invalid byte sequence in a log (a test that printed binary) becomes
replacement characters instead of a parse error; a log viewer must never refuse to show a log
because a program under test misbehaved. GitHub's log blobs start with a UTF-8 byte order mark,
`\u{feff}`, which would otherwise become an invisible first character of line one and break the
timestamp check for that line; it is stripped once at the top. The line cap:

```rust
const MAX_CHECK_LOG_LINES: usize = 200_000;
```

pairs with the 8 MiB byte cap: bytes bound the transfer and the allocation, lines bound the
per-line parse and the renderer's row arithmetic. Whichever trips first sets the log's
`truncated` flag.

### Stage 2: the timestamp split

A runner line looks like:

```text
2026-08-14T18:59:57.3510133Z Current runner version: '2.336.0'
```

The stamp is RFC 3339 UTC with seven fractional digits. `split_log_timestamp` splits on the first
space and keeps the head as the timestamp only if it passes a cheap structural check:

```rust
fn is_log_timestamp(value: &str) -> bool {
    value.len() >= 20
        && value.ends_with('Z')
        && value
            .as_bytes()
            .get(..4)
            .is_some_and(|year| year.iter().all(u8::is_ascii_digit))
        && value.as_bytes().get(4) == Some(&b'-')
        && value.contains('T')
}
```

Length at least 20 (the shortest `YYYY-MM-DDTHH:MM:SSZ` form), a trailing `Z`, four digits then a
hyphen, and a `T` somewhere: enough to reject prose that happens to start a line while accepting
every stamp shape the runner emits, without a date library and without allocating. A line that
fails the check keeps an empty `timestamp` and its full text, which is exactly what a wrapped
continuation line of a multi-line program output should do; the step-attachment pass gives those
lines "stick with the current step" semantics.

### Stage 3: stripping ANSI

`strip_ansi` is a hand-rolled state walk over the two escape-sequence families that appear in
real logs:

```rust
fn strip_ansi(value: &str) -> String {
    let mut output = String::with_capacity(value.len());
    let mut characters = value.chars();
    while let Some(character) = characters.next() {
        if character != '\u{1b}' {
            output.push(character);
            continue;
        }
        match characters.next() {
            Some('[') => {
                for next in characters.by_ref() {
                    if !matches!(next, '0'..='9' | ';' | '?' | ':') {
                        break;
                    }
                }
            }
            Some(']') => {
                for next in characters.by_ref() {
                    if next == '\u{7}' || next == '\u{1b}' {
                        break;
                    }
                }
            }
            _ => {}
        }
    }
    output
}
```

The theory: a CSI sequence is `ESC [`, parameter bytes (digits, `;`, `?`, `:`), then one final
byte that names the command (`m` for color, `K` for erase, and so on); the walk skips parameter
bytes and drops the final byte with the loop's terminating `break`. An OSC sequence is `ESC ]`
running to a BEL (`\u{7}`) or an ESC-introduced terminator; the walk consumes to either. Any
other escape introducer drops just the two-character pair. Everything that is not part of a
sequence is copied through. This is not a complete ANSI implementation, and does not need to be:
it covers what runners and the tools they invoke actually emit, in one allocation, with no regex
compilation on a path that can run 200,000 times per log. The test asserts
`\u{1b}[36mHosted Compute Agent\u{1b}[0m` renders as `Hosted Compute Agent` with the message
"color codes never reach the renderer".

### Stage 4: workflow command markers

The runner brackets its own structure with `##[...]` commands at the start of lines. Quinjet maps
them to a severity enum the theme layer can color, in checked order:

```rust
fn split_log_marker(value: &str) -> (CheckLogSeverity, String) {
    for (marker, severity) in [
        ("##[error]", CheckLogSeverity::Error),
        ("##[warning]", CheckLogSeverity::Warning),
        ("##[notice]", CheckLogSeverity::Notice),
        ("##[command]", CheckLogSeverity::Command),
        ("##[group]", CheckLogSeverity::Command),
        ("##[debug]", CheckLogSeverity::Normal),
        ("[command]", CheckLogSeverity::Command),
    ] {
        if let Some(rest) = value.strip_prefix(marker) {
            return (severity, rest.to_owned());
        }
    }
    if value.starts_with("##[endgroup]") || value.starts_with("##[section]") {
        return (CheckLogSeverity::Normal, String::new());
    }
    (CheckLogSeverity::Normal, value.to_owned())
}
```

A matched marker is stripped so the reader sees `cargo test failed` in error color rather than
`##[error]cargo test failed` in monochrome. `##[group]` headings render as commands (they are the
runner's own fold titles, visually a heading), while `##[endgroup]` and `##[section]` carry no
text worth showing and become empty spacer lines. The bare `[command]` prefix is the older marker
some runner versions emit. Everything unmatched is a normal line, text intact. The test
`strips_timestamps_ansi_and_workflow_commands_from_log_lines` covers the BOM, a group heading, a
colored line, an endgroup, an `##[error]`, and an untimestamped trailing line in one six-line
fixture.

## Attaching lines to steps by whole seconds

The steps API and the log blob describe the same job from two angles: the steps know their names,
statuses, and start/end instants; the log knows every line that was written and when. Joining
them turns a flat 200,000-line wall into a foldable per-step view where the failed step opens
directly onto its own output. The join is `assign_lines_to_steps`, and its correctness hinges on
one clock subtlety that invariant 11a promotes to architecture:

> Check logs are read from the GitHub Actions job endpoint and attached to steps by comparing
> whole-second instants, because runner lines are sub-second while the steps API reports seconds.

### The precision mismatch, concretely

Runner log lines carry seven fractional digits: `2026-08-14T18:59:58.4821004Z`. The steps API
reports whole seconds: `"started_at": "2026-08-14T18:59:58Z"`. Both are the same clock, but
comparing them as text puts the fractional stamp after the whole-second stamp:

```text
"2026-08-14T18:59:58.4821004Z"  >  "2026-08-14T18:59:58Z"   as text? compare position 20:
                                                            '.' (0x2E)  vs  'Z' (0x5A)
                                                            0x2E < 0x5A, so the fractional
                                                            stamp sorts BEFORE the plain one
```

String comparison actually sorts the fractional stamp before its own second's plain form, and
either direction of error is fatal to the join: whichever way the tie breaks, every line written
during the boundary second lands in the wrong step. The function's doc comment states the failure
mode from the code's perspective: "comparing those as text puts everything written during a
step's final second into the step before it." The cure is to truncate both sides to whole-second
integers and compare numbers, accepting that a step boundary inside a second is unobservable: a
line stamped anywhere within second `S` belongs to the last step whose start is `<= S`.

The test `a_step_boundary_splits_on_whole_seconds_not_on_text_order` pins the exact scenario:
step 1 runs `18:59:57` to `18:59:58`, step 2 starts at `18:59:58`, and the line stamped
`18:59:58.4821004Z` must land in step 2, even though as text it sits inside step 1's window.

### The algorithm

The whole function, from `src/git/github/checks.rs`:

```rust
/// Distribute timestamped lines across steps in a single forward pass, moving on
/// as soon as the next step has started. Comparing whole seconds matters:
/// runner lines carry sub-second precision while the steps API reports whole
/// seconds, and comparing those as text puts everything written during a step's
/// final second into the step before it.
///
/// Output from before the first step or after the last one is returned loose,
/// which is where provisioning and teardown failures live.
fn assign_lines_to_steps(steps: &mut [CheckStep], lines: Vec<CheckLogLine>) -> Vec<CheckLogLine> {
    if steps.is_empty() {
        return lines;
    }
    let starts: Vec<Option<i64>> = steps
        .iter()
        .map(|step| timestamp_seconds(&step.started_at))
        .collect();
    let mut loose = Vec::new();
    let mut current: Option<usize> = None;
    for line in lines {
        if let Some(seconds) = timestamp_seconds(&line.timestamp) {
            while let Some(next) = current.map_or(Some(0), |index| {
                (index + 1 < steps.len()).then_some(index + 1)
            }) {
                if starts
                    .get(next)
                    .copied()
                    .flatten()
                    .is_some_and(|start| seconds >= start)
                {
                    current = Some(next);
                } else {
                    break;
                }
            }
            let past_last = current.is_some_and(|index| {
                index + 1 == steps.len()
                    && steps.get(index).is_some_and(|step| {
                        timestamp_seconds(&step.completed_at).is_some_and(|end| seconds > end)
                    })
            });
            if past_last {
                loose.push(line);
                continue;
            }
        }
        match current.and_then(|index| steps.get_mut(index)) {
            Some(step) => step.lines.push(line),
            None => loose.push(line),
        }
    }
    loose
}
```

Reading it as a machine: `current` is the step the pass is inside, starting outside all of them
(`None`). Step starts are precomputed once into `starts` as whole-second instants. For each
timestamped line, the inner `while` advances `current` through every step whose start is `<=` the
line's second, then stops at the first step that has not started yet; a line therefore attaches
to the last step that had started when the line was written. Both the log and the step list are
chronological, so `current` only ever moves forward and the whole join is a single O(lines +
steps) merge pass, not a per-line binary search and not O(lines x steps).

Three boundary rules complete the semantics:

- **Before the first step.** Until the first step's start is reached, `current` stays `None` and
  lines fall into `loose`: runner provisioning output, image setup, the things that fail before
  any step exists. The `CheckRunLog::loose_lines` doc comment names this purpose: "Output
  produced before the first step or after the last one, which is where a runner reports
  provisioning and teardown failures."
- **After the last step.** The `past_last` check applies only when `current` is the final step
  and that step has a parseable completion: a line strictly later than the last step's end
  (`seconds > end`, strictly greater, so the final second stays with the step) goes loose as
  teardown output. Intermediate steps need no end comparison at all; the next step's start
  supersedes them, which also means gaps between steps attribute to the earlier step rather than
  vanishing.
- **No timestamp at all.** Untimestamped lines skip the advancement entirely and stick with
  wherever `current` points. A multi-line block (a stack trace, wrapped program output) stays
  with its step even though only its first line carries a stamp.

### A worked pass

The fixture from `attaches_each_log_line_to_the_step_that_was_running`: two steps, "Set up job"
running `18:00:00` to `18:00:10` and "Run cargo test" running `18:00:10` to `18:02:30`, fed five
lines:

```text
line                                   seconds     current after advance   destination
-------------------------------------  ----------  ----------------------  -----------------
17:59:59  "provisioning"               17:59:59    None (step 1 not begun) loose
18:00:01  "setting up"                 18:00:01    step 1                  step 1
18:00:11  "running tests"              18:00:11    step 2                  step 2
(none)    "continuation of the..."     no stamp    step 2 (unchanged)      step 2
18:05:00  "teardown"                   18:05:00    step 2, past 18:02:30   loose
```

The assertions confirm the distribution exactly, plus the rendered durations: step 2's
`duration_label(0)` is `2m 20s` and step 1's is `10s`. And the degenerate case is its own test,
`steps_without_a_log_keep_every_line_loose`: with an empty step list every line is returned
loose, which is how a job whose steps read failed still renders its complete log.

### What the renderer does with the join

The step view is app state from here on. `expanded_check_steps` holds the fold state per step
number; on a log's first arrival with nothing expanded, the reply handler auto-expands
`log.failed_step().or_else(|| log.running_step())` and reveals it, so selecting a failed check
lands the reader directly inside the step that failed with its lines visible. `running_step()`
and `failed_step()` are first-match scans on the step statuses. Step duration labels stay live
even while the blob is still 404: they come from the steps API timestamps and `unix_now()`, not
from the log, so a running step shows `1m 35s…` (the ellipsis marking "still running") the whole
time `log_pending` is true. That is the invariant 11a clause "step status and elapsed time come
from the jobs API and stay live even while that blob is still missing."

## Time math without a date library

Every instant on this page (conversation sort keys aside, which never leave string form) funnels
through one 13-line parser and one 8-line const function. Quinjet takes no date/time dependency
for this path, and the two functions justify that choice.

### timestamp_seconds

```rust
fn timestamp_seconds(value: &str) -> Option<i64> {
    let (date, rest) = value.split_once('T')?;
    let mut date_parts = date.split('-');
    let year: i64 = date_parts.next()?.parse().ok()?;
    let month: i64 = date_parts.next()?.parse().ok()?;
    let day: i64 = date_parts.next()?.parse().ok()?;
    let time = rest.split(['Z', '+', '.']).next()?;
    let mut time_parts = time.split(':');
    let hour: i64 = time_parts.next()?.parse().ok()?;
    let minute: i64 = time_parts.next()?.parse().ok()?;
    let second: i64 = time_parts.next().unwrap_or("0").parse().ok()?;
    Some(days_from_civil(year, month, day) * 86_400 + hour * 3_600 + minute * 60 + second)
}
```

The split on `['Z', '+', '.']` is the whole-second truncation from the previous section: fractional
digits and offset suffixes are dropped before parsing, and every stamp is treated as UTC, which
GitHub's API guarantees for these fields. Seconds default to `"0"` so a hypothetical `HH:MM`
stamp still parses. Any structural surprise returns `None`, and every caller treats `None` as
"this line or step has no usable instant" rather than an error.

### days_from_civil

The date-to-days conversion is Howard Hinnant's civil-from-days inverse, a classic of branchless
calendar arithmetic:

```rust
/// Howard Hinnant's civil-to-days algorithm, valid across the proleptic
/// Gregorian calendar.
#[expect(
    clippy::integer_division,
    reason = "the civil-to-days algorithm is defined in truncating arithmetic"
)]
const fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = (if year >= 0 { year } else { year - 399 }) / 400;
    let year_of_era = year - era * 400;
    let day_of_year = (153 * (if month > 2 { month - 3 } else { month + 9 }) + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}
```

How it works, step by step. The year is shifted so the computational year begins on March 1,
pushing the leap day to the end of the year where it cannot disturb month arithmetic. Time is
then measured in 400-year eras, because the Gregorian calendar repeats exactly every 400 years
with `146_097` days per era (365 x 400 + 97 leap days: one per 4 years, minus one per 100, plus
one per 400). `day_of_year` uses the linear expression `(153 * m' + 2) / 5`, which maps the
March-first month sequence to cumulative day offsets exactly, exploiting the fact that from March
onward month lengths cycle 31, 30, 31, 30, 31 in groups of 153 days per 5 months. `day_of_era`
adds the era-local leap corrections (`/4` and `/100`; the `/400` correction is absorbed by the
era itself), and the final constant `719_468` shifts the epoch from 0000-03-01 to 1970-01-01. The
function is `const fn`, pure integer arithmetic, no table, no branch on leap years, and the test
`measures_elapsed_time_across_month_and_year_boundaries` walks it across a non-leap February
("February ends on the 28th outside a leap year"), a leap February ("a leap year adds the extra
day between the same two dates", `2024-02-28T12:00:00Z` to `2024-03-01T12:30:00Z` measuring
`48h 30m`), and a year boundary.

### Elapsed rendering

The display layer on top is three small functions. `elapsed_seconds` refuses to go negative:
`(end >= start).then_some(end - start)`, so a completion stamp earlier than its start (clock
weirdness, a half-updated API answer) renders as an empty label, never `-3s`; the test message
reads "a completion before its start is reported as unknown, never negative". `format_elapsed`
renders three magnitudes:

```rust
fn format_elapsed(seconds: i64) -> String {
    if seconds < 60 {
        format!("{seconds}s")
    } else if seconds < 3_600 {
        format!("{}m {}s", seconds / 60, seconds % 60)
    } else {
        format!("{}h {}m", seconds / 3_600, (seconds % 3_600) / 60)
    }
}
```

with hours unbounded (a two-day span prints `48h 30m` rather than rolling into days). And
`CheckStep::duration_label(now)` handles the running case: a step with an empty `completed_at`
renders `format_elapsed(now - started)` with a trailing ellipsis, and renders nothing at all
until `now > started`; per the test, "a step reports nothing until at least a second has
passed", which avoids a flickering `0s…` on freshly started steps. `now` is `unix_now()`,
seconds since the epoch via `SystemTime`, defaulting to 0 on any system-clock failure rather
than panicking in a render path.

## Adaptive polling and per-stream floors

Everything above describes single reads. What makes the checks surface feel live is the schedule
those reads run on, and what makes the schedule affordable is that almost every tick is allowed
to do nothing. The design is invariant 11; this section maps it to the code in `src/app.rs`.

### One tick, three cadences

```rust
    /// Watch a running pull request closely and a settled one loosely. The
    /// interval also stretches when the reader is somewhere else, so a loaded
    /// pull request stays fresh without spending requests on an unseen pane.
    fn pull_request_poll_interval(&self) -> Duration {
        if self.view != View::PullRequests {
            return PULL_REQUEST_BACKGROUND_POLL;
        }
        if self
            .pull_request_checks
            .iter()
            .any(|check| check.status.is_running())
        {
            PULL_REQUEST_ACTIVE_POLL
        } else {
            PULL_REQUEST_IDLE_POLL
        }
    }
```

The three constants: `PULL_REQUEST_ACTIVE_POLL = 5 s` while any check runs,
`PULL_REQUEST_IDLE_POLL = 20 s` once everything settles, `PULL_REQUEST_BACKGROUND_POLL = 120 s`
when the reader is in another view. The comment block above the constants gives the reasoning: "A
run in progress changes state in seconds and is worth watching closely; a settled pull request
only needs to notice new comments; a pull request nobody is looking at needs less again."

### The tick is a ceiling, the floors are the schedule

The naive reading of "poll every 5 seconds" would issue five requests per tick: checks, metadata,
conversation, log, and repository identity. The actual design, from the comment at the poll
constants: the tick cadence is "a ceiling rather than a schedule: check state is the only thing
worth reading as often as the tick fires. Metadata, the conversation and a growing log all change
on human or build timescales and hold their own floor." Two per-stream floors implement that:
`PULL_REQUEST_DETAIL_POLL = 20 s` for metadata plus conversation, and `PULL_REQUEST_LOG_POLL =
8 s` for a growing log. `refresh_pull_request_live` runs whichever reads are due:

```rust
        let due = |last: Option<Instant>, interval: Duration| {
            force || last.is_none_or(|last| now.duration_since(last) >= interval)
        };

        if due(
            self.pull_request_checks_read_at,
            self.pull_request_poll_interval(),
        ) {
            let issued = effects.len();
            self.request_pull_request_checks(true, effects);
            if effects.len() > issued {
                self.pull_request_checks_read_at = Some(now);
            }
        }
```

The `issued` pattern is easy to miss and important: the floor timestamp is stamped only when an
effect was actually pushed. A read suppressed by its own loading flag (the previous poll's reply
has not landed yet) does not consume its floor; it stays due and fires on the next tick, which is
the invariant 11 sentence "A stream that coalesces into an in-flight request stays due instead of
being skipped." Without this, a slow conversation read on the 20 s floor could eat its own
refresh slot and drift to 40 s.

On the fast 5 s cadence, then, the steady-state cost is exactly one request per tick (the check
list), with the 20 s streams joining every fourth tick and the log stream on its own 8 s clock,
gated further to fire only while the selected check is running. The test pinning this is quoted
in the notes as "a fast tick only speeds up the reads that change that fast".

### Settled pull requests and the webhook override

```rust
        let settled = self
            .pull_request
            .as_ref()
            .is_some_and(|pull_request| matches!(pull_request.state.as_str(), "MERGED" | "CLOSED"));
        if settled && !force {
            return;
        }
```

A merged or closed pull request keeps only the cheap check-list read on the adaptive interval;
metadata, conversation, and logs stop polling entirely, because a settled pull request's thread
changes rarely and its diff never. The `force` parameter is the webhook path: an optional
loopback listener (paired with `gh webhook forward`, documented in
[./api-strategy.md](./api-strategy.md)) turns a delivery into `refresh_pull_request_live(now,
force = true, ...)`, and `force` short-circuits the `due` closure for every stream, bypassing all
floors and the settled short-circuit at once. A delivery is a signal that something definitely
changed, so paying every read immediately is correct exactly once.

Two more clauses of invariant 11 close the loop. "A finished run's log is never re-read": the log
floor only fires while the selected check `is_running()`, and a settled run's log is immutable
and disk-cached, so re-reading it would be a no-op paid at network prices; the test assertion
reads "a finished run's log never changes, so a poll does not re-read it". And refresh failures
preserve the previous snapshot: every reply handler on this page keeps the last good state and
records the error beside it, so a transient network failure during a poll never blanks a pane the
reader is looking at.

## Warming settled logs on the warm lane

A pull request with thirty check runs would be miserable to browse if every selection paid a
round trip. The warming path makes selection a disk read: as soon as the check list is known,
every settled run's log is fetched into the immutable cache in the background. Invariant 12b:
"Opening a pull request warms settled run logs in the background, on a dedicated lane behind
every interactive read, capped at 32 stable GitHub job identities."

### The repository half

`Repository::prefetch_check_run_logs` (`src/git/github/checks.rs`) is one iterator chain, and the
order of its adapters is the design:

```rust
    /// Read every finished run into the cache so that selecting any of them is
    /// answered from disk. Runs still in progress are skipped: their output is
    /// not cacheable, and re-reading it here would spend requests the live tail
    /// is about to spend anyway.
    pub(crate) fn prefetch_check_run_logs(
        &self,
        pull_request: &PullRequest,
        checks: &[PullRequestCheck],
        wanted: &dyn Fn() -> bool,
    ) -> usize {
        checks
            .iter()
            .filter(|check| !check.status.is_running() && check.job_id().is_some())
            .take(MAX_PREFETCHED_CHECK_LOGS)
            .take_while(|_| wanted())
            .filter(|check| self.pull_request_check_log(pull_request, check).is_ok())
            .count()
    }
```

Reading the chain: only settled Actions jobs qualify (running jobs are the live tail's business,
and non-Actions checks have nothing to fetch); at most `MAX_PREFETCHED_CHECK_LOGS = 32` of them
are considered ("A ceiling on how much a single pull request will warm in the background");
`wanted()` is polled before each job, so cancellation lands between fetches, never mid-fetch; and
each surviving check runs the exact same `pull_request_check_log` the foreground uses, whose
`CacheLife::Immutable` path writes both the steps and the log blob to disk. Warming is therefore
not a second code path that could drift from the real one; it is the real read, executed early.
The placement of `take_while` after `take` also means a cancelled warm-up counts against nothing:
the test `a_warm_up_stops_as_soon_as_the_pull_request_it_serves_is_left` passes `wanted = ||
false` and asserts zero jobs were warmed and no I/O happened, "a superseded warm-up asks for
nothing".

### The app half

`request_check_log_prefetch` (`src/app.rs`) decides what to warm and remembers what it already
asked for:

```rust
        let settled: Vec<PullRequestCheck> = self
            .pull_request_checks
            .iter()
            .filter(|check| !check.status.is_running() && check.job_id().is_some())
            .filter(|check| {
                !self
                    .pull_request_prefetched_logs
                    .contains(&check.identity())
            })
            .take(32_usize.saturating_sub(self.pull_request_prefetched_logs.len()))
            .cloned()
            .collect();
        if settled.is_empty() {
            return;
        }
        self.pull_request_prefetched_logs
            .extend(settled.iter().map(PullRequestCheck::identity));
```

`pull_request_prefetched_logs` is a per-pull-request set of check identities, and identities are
marked before the command is even sent: warming is fire-and-forget from the app's perspective,
and the worker's warm lane owns cancellation. The budget arithmetic
`32 - prefetched.len()` makes 32 a per-pull-request lifetime cap over stable job identities, not
a per-call batch size: the function runs again after every checks refresh, each run picking up
only jobs that newly settled, and the total across all runs never passes 32. A re-run job gets a
fresh job id, hence a fresh identity, and can be warmed within whatever budget remains. The doc
comment gives the why: "Selecting a check then costs a disk read rather than a round trip, which
is the difference between the list being browsable and being a series of waits."

The handler that triggers all this is the checks reply itself, which calls
`request_check_log_prefetch` after installing every snapshot, so warming begins the moment the
first check list lands and keeps up as runs finish one by one during a live poll.

### The lane and the generation

`PrefetchCheckRunLogs` routes to `WorkerLane::Warm`: its own OS thread (`quinjet-warm`), its own
mailbox slot, and the lowest slot priority in the mailbox's `pop` order, behind every interactive
read. The isolation is pinned by the worker test named
`warming_logs_never_shares_a_lane_with_the_reads_a_reader_waits_on`. Cancellation is a single
atomic. `GitWorker::send` stamps every warm command with a fresh generation:

```rust
        if let WorkerCommand::PrefetchCheckRunLogs { generation, .. } = &mut command {
            *generation = self.warm_generation.fetch_add(1, Ordering::SeqCst) + 1;
        }
```

and the warm worker runs each job under a closure comparing that stamp to the shared counter:

```rust
/// The warm-up lane runs one job at a time and answers to nothing but its own
/// generation, so a pull request the reader has left stops costing requests as
/// soon as another one asks to be warmed.
fn run_warm_worker(
    repository: &Repository,
    mailbox: &Arc<SharedMailbox>,
    _events: &Sender<WorkerEvent>,
    generation: &Arc<AtomicU64>,
) {
```

The body executes `Command::WarmCheckRunLogs` with
`&|| generation.load(Ordering::SeqCst) == mine` as the `wanted` closure. Opening a different pull
request issues a new warm command, `send` bumps the atomic, the in-flight chain's next
`wanted()` poll observes the mismatch, and the superseded warm-up stops before its next job. No
handle bookkeeping, no cancellation channel: one `AtomicU64` and a closure. The warm worker also
emits no events (`_events` is unused); its entire output is cache entries on disk, and the
foreground discovers them the ordinary way, by reading. In the CLI command vocabulary the
execution is `Command::WarmCheckRunLogs` with the progress label "Caching check-run logs"
(`src/cli/command.rs`).

### The sibling prefetcher, and an evolution note

The checks warm-up has a sibling on the diff side: the background prefetch of PR file patches,
which fills `LoadPullRequestFileBatch` commands on the `PullRequestPreview` lane's dedicated
prefetch slot. It is documented in full in [./prefetch.md](./prefetch.md) and
[../rendering/progressive-loading.md](../rendering/progressive-loading.md); the shape as of the
current code is viewport-anchored wrap-around order: batches of up to 32 files, sized by
per-file count estimates (80 bytes per changed line plus 4,096, with a 512 KiB fallback for a
file without counts) under a 6 MiB estimated-byte budget, walking the index from the first file
visible in the Files tree and wrapping around the rest, up to a 4,096-file cap. That ordering has
a history worth recording accurately: PR #50 introduced smallest-first size-tiered ordering, in
which prefetch past large thresholds served the smallest files first, and PR #55 subsequently
replaced that ordering with the viewport-anchored wrap-around walk so patches land where the
reader is actually looking. The smallest-first tiers were an evolution step, not current
behavior. Both prefetchers share the deeper principle this page keeps returning to: background
work must be cancellable between units, bounded in total, and incapable of displacing anything a
reader is waiting on (invariant 3).

## Cache keys and lifetimes

Every cache entry on this page follows the taxonomy of invariant 12: an entry whose key already
names its content is immutable and never expires, and only genuinely time-varying reads keep a
clock. The full inventory:

| Key format | Life | Contents |
| --- | --- | --- |
| `conversation-timeline-v2\n{url}\n{number}\n{updated_at}` | Immutable (stamp includes `updated_at`) | `complete`/`partial` marker line + timeline TSV |
| `conversation-timeline-validator-v2\n{url}\n{number}` | Immutable (ETag validated) | ETag line + page-1 TSV body |
| `conversation-comments-v2\n{url}\n{number}\n{updated_at}` | Immutable | marker + review-comment TSV |
| `conversation-comments-validator-v2\n{url}\n{number}` | Immutable (ETag validated) | ETag + page-1 body |
| `checks-v1\n{url}\n{number}\n{head_oid}` | TTL 30 s | `gh pr checks` TSV body |
| `check-steps-v1\n{repo}\n{job}\n{life:?}` | Immutable for settled, TTL 0 for running | job steps TSV (2 MiB bound) |
| `check-log-v1\n{repo}\n{job}` | Immutable, settled runs only | raw log blob, 8 MiB bound both ways |

Reading the table against the taxonomy:

- **Immutable by activity stamp.** The two conversation snapshot keys embed `updated_at`. New
  activity moves the stamp, so a fresh thread is a different question with a different key; the
  old entry is never wrong, only eventually evicted. This is why a conversation can be served
  from disk with zero validation when the pull request metadata says nothing changed.
- **Immutable by validator discipline.** The validator keys are stable across updates by design,
  but the entry stores the ETag beside the body it validates, on one line, so the pair "can never
  be stored out of step with each other". Staleness is impossible because the entry is only ever
  used to ask GitHub whether it is stale.
- **Immutable by object identity.** Steps and logs are keyed by job id. A job's settled steps and
  archived log are as immutable as a Git object: a re-run is a new job id, a new key, a new
  entry. This mirrors the merge-base and patch caches keyed by commit OIDs described in
  [../git-internals/object-model.md](../git-internals/object-model.md).
- **The one clock.** The check list is keyed by head OID but varies within one head as runs
  progress, so it holds the sole TTL, 30 seconds, matching its own poll floor's order of
  magnitude.
- **The deliberate hole.** A running job's log is in no row of this table's cached column: never
  written, never read from disk. The absence is the feature; re-reading is what tails it.

The store beneath these keys (atomic writes, private modes, the 128 MiB / 2,048-entry bound
pruned oldest first, `QUINJET_CACHE_DIR` relocation) is shared with every other GitHub read and
documented in [./caching.md](./caching.md).

## Design alternatives and why they lost

Each major mechanism on this page had at least one plausible competitor. Recording why the
shipped design won is cheap insurance against relitigating them.

**One GraphQL query instead of two REST streams.** GitHub's GraphQL API exposes a timeline
connection that could fetch the thread in one request shape, with cursor pagination from either
end. It lost on three grounds. The transport is `gh api` with jq reduction to TSV, a pipeline the
whole module already uses, with ETag validation that makes an unchanged answer free; GraphQL has
no equivalent of the zero-cost 304 on this path and its responses would need a JSON layer the
byte-oriented parsers deliberately avoid. Cursor pagination also composes poorly with the
completeness marker and `rel="last"` arithmetic the bounded walk is built on: cursors are opaque,
while page numbers let the reader compute exactly which pages it skipped. And the REST timeline's
per-event field naming, the main cost of the choice, is contained in one jq program.

**Trusting the timeline for inline comments.** One stream instead of two would halve the paging
work. It lost because the timeline's `line-commented` grouping is emitted "only... for some pull
requests", per the function's own doc comment; completeness beats elegance, and the URL dedupe
makes the overlap harmless at the cost of one linear scan per comment.

**Capping `--paginate` instead of paging manually.** The pre-#48 design could have been patched
by keeping `--paginate` and truncating harder. It lost because no external cap can change the
order pages stream in: oldest-first delivery plus any cap drops the newest activity. The order
had to be inverted at the request level, which requires per-page control, which `--paginate`
exists to hide.

**A date/time library for the checks time math.** `chrono` or `time` would parse RFC 3339 in one
call. The checks path needs exactly: whole-second Unix instants from a known-UTC fixed format,
elapsed spans, and nothing else. Thirteen lines of split-and-parse plus eight lines of Hinnant's
algorithm cover that with zero dependencies, `const`-evaluable date math, and no panic surface;
the leap-year and boundary tests pin the correctness that a library would otherwise be buying.

**Regex-based ANSI stripping.** A regex over each line is the common answer and would be compiled
once and run 200,000 times per log, allocating per match. The hand-rolled state walk is one pass,
one output allocation per line, and handles the OSC-until-BEL case that naive ANSI regexes miss.

**Caching a running log with a short TTL.** A 2-second TTL on running logs would deduplicate
overlapping readers. It lost to invariant 12's cleaner rule: a running job is never cached. The
tail is the re-read; a TTL would add a staleness window and a cache-invalidation obligation to
save requests that the 8-second floor already bounds.

**The run-level log archive.** GitHub also serves a whole workflow run's logs as one archive. It
lost because the unit the reader selects is the job, the archive multiplies the transfer by the
job count, and an archive needs extraction before the byte caps can even be applied. The per-job
endpoint serves exactly the selected unit, already concatenated, and its partial-while-running
behavior is what makes tailing possible at all.

**Exit-status trust for `gh pr checks`.** Treating non-zero exit as failure is the default
posture everywhere else in the codebase. Here it lost to the tool's own semantics: exit 1 and 8
are verdicts about the checks, not the command, so content recognition (a non-empty body, a
"no checks" stderr) is the only correct acceptance test.

## Edge cases and failure modes

A closing catalog of the boundary behaviors this page's machinery defines, each one sentence of
what happens and why that is the right answer.

- **A timeline response with no `rel="last"`.** The reader degrades to the bounded forward walk;
  a single-page thread never notices, and a hypothetical multi-page response without the header
  stays bounded even though its cap direction degrades to oldest-first.
- **A validated first page that was pipe-truncated or errored.** Both collapse to `None` and the
  page loop refetches page 1 bounded; the conversation renders instead of failing, which is the
  #48 hard-fail fix.
- **The cap trips mid-walk.** The stream stops before its next fetch, overshooting by at most one
  100-record page; the merged thread is then cut to exactly 500 with only the oldest dropped, and
  `truncated` makes the view say so.
- **A page ends without a trailing newline.** `append_records` adds one and counts the tail as a
  record, so the next page cannot fuse with it.
- **A record arrives with the wrong field count.** The parse fails with a 1-based record number;
  a three-field record is an error, never padded.
- **An unknown timeline event.** The jq fallback emits actor and timestamp, the parser maps it to
  `ConversationKind::Other`, and it renders as a dated line.
- **A review comment with an empty URL.** The dedupe guard ignores empty URLs, so it can never
  collide with the many timeline events that also have none.
- **Two entries share a timestamp.** The stable sort keeps merge order: timeline context first,
  appended comments after.
- **A conversation refresh during a load.** One boolean coalesces any number of requests into a
  single follow-up when the reply lands.
- **A 64 KiB body cut mid-character.** `bounded_text` retreats to a char boundary and appends an
  ellipsis; no invalid UTF-8, and the cut is visible.
- **A check with no Actions link.** `job_id()` is `None`, the log view says "{name} does not
  publish logs through GitHub Actions", and the check still renders its state and description.
- **The first seconds of a job.** The logs endpoint answers 404, `log_not_published` maps it to
  an empty log, and `log_pending: true` renders live step statuses and durations while the blob
  does not yet exist.
- **Retention expired.** 410 takes the same path as 404: steps still render, the log is honestly
  absent, and nothing errors.
- **A log over 8 MiB or 200,000 lines.** The child is killed at the byte cap or the parse stops
  at the line cap; either sets `truncated` and the prefix renders.
- **A settled run's log.** Written to disk once, read from disk forever, never re-polled; the
  8-second floor applies only to a selected running check.
- **The reader scrolled up in a tailing log.** `following` is false, the refresh updates in
  place, and the scroll position survives; only a reader already at the bottom is carried along.
- **Selection changes while a log read is in flight.** The generation bumps, the stale reply is
  dropped, and the new run starts from a clean slate with its own 8-second clock.
- **A step numbered unparseably.** It defaults to its array position plus one and keeps its
  relative order.
- **A steps read that fails outright.** Steps become an empty list and the whole log renders
  loose; decoration is never allowed to take the content down with it.
- **A job that settles between polls.** The next steps read uses the `Immutable` life and a
  different `{life:?}` key, so the settled entry starts fresh instead of inheriting the running
  phase's snapshot.
- **A re-run job.** New job id, new identity, new cache keys; it tails as a running job, settles
  into its own immutable entries, and can be warmed within the pull request's remaining 32-slot
  budget.
- **Leaving a pull request mid-warm-up.** The next warm command bumps the shared generation and
  the old chain stops before its next job, having cost nothing further.
- **A completion stamp before its start.** The elapsed label is empty, never negative.
- **A system clock failure.** `unix_now()` returns 0 and running-step durations degrade to empty
  labels instead of panicking a render.

Every entry in this catalog traces back to one of three disciplines: bound every read and make
the bound's direction match what the reader values (newest conversation activity, the selected
run's log), key caches by identity so staleness is structurally impossible, and let background
work be cancelled between units rather than trusted to finish. The same three disciplines,
generalized across the codebase, are the subject of [../techniques.md](../techniques.md).
