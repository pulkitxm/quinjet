# The GitHub API Strategy

Quinjet talks to GitHub through exactly one door: a spawned `gh` process with a pinned
environment, a byte-capped pipe, and a `--jq` program that reduces every response to
tab-separated records before a single byte crosses back into Rust. This page documents the REST
machinery behind that door and the policy layered on top of it: how rate limits shape the design,
how `Link`-header pagination is read and exploited, how conditional requests make re-checking an
unchanged thread nearly free, why per-file line counts come from the pulls files endpoint instead
of a local `git diff --numstat`, how the compare API replaces a deepening fetch ladder with one
metadata request, why `gh pr checks` exit codes 1 and 8 are treated as answers, which byte and
entry cap guards every read, and how the adaptive poll spends as few requests as a live view can
get away with. Each mechanism is explained in general first, then anchored to the exact Quinjet
code that leans on it.

## Contents

- [The transport: gh as the HTTP client](#the-transport-gh-as-the-http-client)
- [Rate limiting and the request economy](#rate-limiting-and-the-request-economy)
- [Pagination and the Link header](#pagination-and-the-link-header)
- [Conditional requests: the validated read](#conditional-requests-the-validated-read)
- [The pulls files endpoint: line counts without blobs](#the-pulls-files-endpoint-line-counts-without-blobs)
- [The compare API: merge bases as metadata](#the-compare-api-merge-bases-as-metadata)
- [Checks endpoints: exit codes as data](#checks-endpoints-exit-codes-as-data)
- [Caps on every read](#caps-on-every-read)
- [The cached wrapper: fresh, network, stale](#the-cached-wrapper-fresh-network-stale)
- [The adaptive poll](#the-adaptive-poll)
- [Repository identity discovery](#repository-identity-discovery)
- [Evolution across the merged stack](#evolution-across-the-merged-stack)
- [Design alternatives and why they lost](#design-alternatives-and-why-they-lost)
- [Reading on](#reading-on)

## The transport: gh as the HTTP client

Quinjet never links an HTTP client, a TLS stack, a JSON parser for API responses, or an OAuth
implementation. Every GitHub interaction is a spawned `gh` subprocess, the same way every Git
interaction is a spawned `git` subprocess. The GitHub CLI already solves the problems that make
API clients large: token storage and refresh, host resolution for GitHub Enterprise, HTTP/2 and
proxy handling, and endpoint versioning. What `gh` does not solve on its own is being a
well-behaved subprocess inside a terminal application that must never block, never prompt, never
paginate interactively, and never emit color codes into a parser. That behavior is imposed from
the outside, at the spawn site.

### The four environment variables

`Repository::run_gh_bounded` in `src/git/github/mod.rs:1337` is the single spawn site for `gh`.
Every invocation, from a pull-request lookup to an 8 MiB check log read, goes through it:

```rust
let mut command = Command::new("gh");
let _ = command
    .current_dir(&self.root)
    .args(args)
    .env("GH_PROMPT_DISABLED", "1")
    .env("GH_PAGER", "cat")
    .env("GH_NO_UPDATE_NOTIFIER", "1")
    .env("NO_COLOR", "1");
run_bounded_command(&mut command, stdout_limit, MAX_GH_ERROR_BYTES).with_context(|| {
    format!(
        "failed to execute GitHub CLI (`gh`) in {}; install it and run `gh auth login`",
        self.root.display()
    )
})
```

Each variable removes one way a subprocess can stop being a function call:

**1. `GH_PROMPT_DISABLED=1` removes interactivity.** `gh` is an interactive tool by default: it
will ask which repository you meant, whether to continue, or which account to use. A worker
thread that hits a prompt does not error, it hangs, holding its mailbox lane until the process
is killed. With prompts disabled, an ambiguous invocation fails immediately with a message on
stderr, which the bounded runner captures and surfaces. This is half of ARCHITECTURE.md
invariant 13: "`gh` runs with prompts, paging, color, and update checks disabled on the worker
thread."

**2. `GH_PAGER=cat` removes the pager.** When stdout looks like a terminal, `gh` pipes long
output through a pager. Quinjet's workers read `gh` through a pipe, so the pager would normally
not engage, but the variable makes the contract explicit and immune to configuration: a user's
`PAGER=less -R` in their shell profile can never leave a `less` process wedged between `gh` and
the reader thread.

**3. `GH_NO_UPDATE_NOTIFIER=1` removes background chatter.** `gh` periodically checks for new
releases of itself and prints an upgrade notice to stderr. Quinjet parses stderr for error
content (`bounded_command_error` at `src/git/github/mod.rs:2296` prefers stderr over stdout when
reporting a failure), so an unrelated "a new release is available" line would pollute every
error message and waste a network round trip inside a latency-sensitive worker.

**4. `NO_COLOR=1` removes escape sequences.** Every response Quinjet reads is parsed as bytes:
TSV records, raw log blobs, HTTP response heads. ANSI color sequences inside those bytes would
corrupt field boundaries. The one place escape sequences are wanted, raw Actions runner logs,
opts back in explicitly with `gh api --allow-escape-sequences` and strips them itself (see
[Checks endpoints: exit codes as data](#checks-endpoints-exit-codes-as-data)).

The working directory is set to the repository root so `gh` resolves its authentication host and
default repository from the repository's own remotes, which matters on GitHub Enterprise where
the token to use depends on the host being addressed.

### The bounded pipe underneath

The second half of the transport is `run_bounded_command` in `src/git/github/mod.rs:2222`, the
universal child runner shared by `gh` and the disposable-workspace `git` invocations. Its core
loop reads stdout on the calling thread in 64 KiB chunks and kills the child the moment the
byte budget is exceeded:

```rust
let remaining = stdout_limit.saturating_sub(collected.len());
if read > remaining {
    collected.extend_from_slice(buffer.get(..remaining).unwrap_or(&buffer));
    truncated = true;
    drop(child.kill());
    break;
}
collected.extend_from_slice(buffer.get(..read).unwrap_or(&buffer));
```

Three properties follow, and each one matters to the API strategy:

**1. Every cap is a memory bound, not a truncation after the fact.** A pathological response,
say a log endpoint that streams gigabytes, costs at most `stdout_limit` bytes of memory plus one
64 KiB read buffer of transfer beyond it. The child is killed mid-stream rather than drained.
The test `bounded_runner_kills_oversized_git_output` (`src/git/github/mod.rs:3090`) pins this:
a 256 KiB blob read under a 1,024-byte cap yields `stdout_truncated == true` and exactly 1,024
retained bytes.

**2. stderr can never deadlock the child.** stderr is drained on a spawned thread by
`read_and_drain` (`src/git/github/mod.rs:2280`), which reads to end of stream but retains at
most its own limit; excess bytes are read and discarded. Without this, a child writing more
stderr than the pipe buffer holds would block on `write(2)` while the parent blocks reading
stdout: a classic two-pipe deadlock.

**3. Truncation is a flag, not an exception.** The result type is `BoundedOutput { status,
stdout, stderr, stdout_truncated }` (`src/git/github/mod.rs:2211`). Callers decide what a
truncated read means for their format: metadata readers treat it as a hard error, page readers
repair the buffer to whole records, log readers mark the document truncated and render what
arrived. That per-caller decision is a recurring pattern on this page.

### argv discipline

No GitHub invocation ever passes through a shell. Arguments are built as `OsString` vectors and
handed to `std::process::Command` directly, so a branch named `; rm -rf` or a path containing a
quote character is just bytes in one argv slot. ARCHITECTURE.md invariant 7 states it as: "Git
and GitHub CLI receive argv directly, never via a shell." The endpoint strings interpolated into
`gh api` calls are built from validated parts only: repository names come from GitHub's own
`nameWithOwner` answers, commit identifiers pass `is_commit_oid` (`src/git/github/mod.rs:1945`,
exactly 40 or 64 ASCII hex digits) before they are embedded anywhere, and pull-request numbers
are `u64` values that were parsed, not strings that were trusted.

### jq to TSV: the wire format after the wire

GitHub answers in JSON, and Quinjet does not want JSON in its hot path. Every listing response
is reduced inside `gh` itself with a `--jq` program that projects the fields Quinjet needs into
`@tsv` records: one line per object, fields separated by tabs, with tab, newline, carriage
return, and backslash escaped as `\t`, `\n`, `\r`, and `\\` by jq's own `@tsv` filter. The Rust
side then needs only `parse_tsv_record::<N>` (`src/git/github/mod.rs:1521`), which strips a
trailing carriage return, splits on the tab byte, unescapes each field with `unescape_tsv`
(`src/git/github/mod.rs:1534`), and arity-checks the record against a compile-time field count.

The choice buys four things:

- **A smaller response.** The projection happens before the bytes hit the capped pipe, so a
  2 MiB metadata budget holds fields Quinjet will render, not the full API object graph. A
  single timeline event carries dozens of fields; the TSV record carries eight.
- **A cacheable body.** Cache entries hold the exact bytes `gh` produced. Replaying a cached
  entry through the same TSV parser as a live response means the cache layer needs no
  serialization format of its own. The pull-request metadata record is even recognized inside
  arbitrary cache files by shape alone: `cached_pull_request_at` (`src/git/github/mod.rs:2447`)
  identifies an entry as PR metadata because its first line parses as an 18-field TSV record
  whose first field is a number and whose eighth field is a `/pull/N` URL.
- **A stable parser.** The record shape is fixed by the jq program, so a field GitHub adds to
  the JSON never shifts a column. A record with the wrong arity is an error with a precise
  message: "expected N tab-separated fields, received M".
- **No serde on the response path.** The one JSON parser dependency Quinjet's GitHub path could
  have needed simply does not exist; jq inside `gh` is the deserializer.

The 18-field pull-request record (`PULL_REQUEST_TSV_FIELDS = 18`, `src/git/github/mod.rs:56`)
is the largest of these shapes, produced by `gh pr view` with a `--json` field list of
`number,title,body,author,state,isDraft,createdAt,updatedAt,url,baseRefName,baseRefOid,headRefName,headRefOid,headRepository,isCrossRepository,additions,deletions,changedFiles`
and a jq template flattening it to one row. Free-text fields are truncated after parsing with
`bounded_text` (`src/git/github/mod.rs:1510`), char-boundary safe, to 16 KiB for a title and
256 KiB for a description, so a pathological PR body cannot bloat state or cache.

The full catalog of `gh` invocations, with every flag explained, lives in
`../git-internals/plumbing-and-porcelain.md` for the Git side; the GitHub side is exactly the
following and nothing more:

```text
gh pr view N --repo URL --json <18 fields> --jq <TSV program>
gh pr checks N --repo URL --json bucket,completedAt,description,link,name,startedAt,state,workflow --jq <TSV>
gh pr merge|close|reopen N --repo URL [--merge|--squash|--rebase] [--delete-branch]
gh repo view [URL] --json nameWithOwner,url --template <TSV template>
gh api -i [-H "If-None-Match: <etag>"] <endpoint> --jq <jq>
gh api repos/{owner}/{repo}/compare/{base}...{head} --jq .merge_base_commit.sha
gh api -i "repos/{owner}/{repo}/pulls/{n}/files?per_page=100&page=N" --jq <TSV>
gh api -i "repos/{owner}/{repo}/issues/{n}/timeline?per_page=100&page=N" --jq <TSV>
gh api -i "repos/{owner}/{repo}/pulls/{n}/comments?per_page=100&sort=created&direction=desc&page=N" --jq <TSV>
gh api repos/{owner}/{repo}/actions/jobs/{job} --jq <steps TSV>
gh api [--allow-escape-sequences] repos/{owner}/{repo}/actions/jobs/{job}/logs
```

Two verbs mutate (`gh pr merge|close|reopen`); everything else reads. There is deliberately no
repository-wide PR-list operation anywhere in the codebase (`src/git/github/mod.rs` is described
in ARCHITECTURE.md as having "no repository-wide PR-list operation"): Quinjet loads exactly the
pull request the reader asked for, which is the first and cheapest rate-limit decision in the
whole design.

## Rate limiting and the request economy

### How GitHub meters REST reads

The REST API meters authenticated callers against a fixed hourly request budget, reported on
every response through the `x-ratelimit-limit`, `x-ratelimit-remaining`, and
`x-ratelimit-reset` headers, and enforces secondary limits on request concurrency and burst
behavior on top of it. The canonical description lives in the
[GitHub REST documentation](https://docs.github.com/en/rest). Two properties of that metering
shape everything on this page:

- **The budget is per token, not per application.** Quinjet shares the reader's `gh` token with
  every other tool the reader uses: their editor integration, their scripts, their own `gh`
  invocations at the shell. A TUI that polls carelessly does not merely slow itself down; it
  starves the rest of the reader's tooling for the remainder of the hour.
- **A `304 Not Modified` answer to a conditional request does not count against the budget.**
  This is the single most exploitable fact in the API's design, and the doc comment on
  Quinjet's validated read (`src/git/github/mod.rs:589`) states it as the reason the mechanism
  exists: the reply "carries no body and costs nothing against the rate limit, which is what
  lets an unchanged thread be re-checked as often as it is worth checking."

Quinjet never reads the rate-limit headers themselves and never queries the rate-limit
endpoint. That is a deliberate simplification: rather than adapting to the remaining budget, the
design spends so little that the budget is never the binding constraint. Every mechanism below
is a way of not sending a request, and the ones that must send one are paced by floors measured
in tens of seconds.

### What one piece of state costs

It is worth tabulating what each kind of on-screen state costs in requests, because the rest of
the page is the machinery that produces these numbers:

| On-screen state | Requests on first load | Requests when unchanged | Requests when changed |
|---|---|---|---|
| PR metadata (title, refs, OIDs) | 1 (`gh pr view`) | 0 within 5 min TTL, then 1 | 1 |
| Repository identity | 1 (`gh repo view`) | 0 within 24 h TTL | 1 |
| Check list | 1 (`gh pr checks`) | 0 within 30 s TTL, then 1 | 1 |
| Conversation (both streams) | 2 to 2 + pages | 0 (stamp hit) or 2 free 304s | 2 + changed pages |
| Merge base | 1 (compare API) | 0 forever (immutable key) | new key, 1 |
| Per-file counts | 1 per 100 files, max 64 | 0 forever (immutable key) | new key, same cost |
| Settled run log | 1 + 1 (steps + log) | 0 forever (immutable key) | never changes |
| Running run log | 1 + 1 per tail read | never cached by design | 1 + 1 per 8 s |
| File patches | 0 (Git, not the API) | 0 | 0 |

The last row is the deepest point on the page: patch bytes never cross the REST API at all.
Diffs come from Git object transfer into a disposable workspace (shallow, blob-less, described
in `./pr-workspace.md` and `../git-internals/shallow-and-partial-clone.md`), which is metered by
Git's own protocol, not by the REST budget. The REST API is used for what it is uniquely good
at: metadata that GitHub has already computed (counts, merge bases, check state, conversation)
and content that only GitHub holds (Actions logs).

### The defense in depth

Every layer between a redraw and a network request, ordered from cheapest to most expensive:

1. **In-memory state.** A redraw renders what the app already holds; the render path issues no
   requests, spawns no processes, and touches no filesystem (invariant 1).
2. **Request coalescing.** Every read class occupies one mailbox slot; a newer request replaces
   an unstarted older one, and in-flight loads set flags that make duplicate requests no-ops
   (`refresh_again` patterns in `src/app.rs`). Typing through pull-request numbers runs one
   lookup, not one per keystroke.
3. **The on-disk cache.** Immutable-keyed entries (OID pairs, job identifiers, activity stamps)
   answer forever; TTL entries answer within their clock. See `./caching.md` for the full
   split.
4. **Conditional requests.** When a validator exists, an unchanged first page costs a free 304.
5. **The floors.** Metadata and conversation reads never fire more often than every 20 seconds,
   a growing log never more often than every 8 seconds, regardless of the tick.
6. **The settled gate.** A merged or closed pull request stops paying for detail streams
   entirely (PR #55; see [The adaptive poll](#the-adaptive-poll)).
7. **Cancellation.** The background log warmer checks a generation before every job and stops
   mid-list the moment the reader moves to another pull request, so abandoned warming never
   finishes spending its budget.
8. **The webhook shortcut.** When a forwarded delivery says something definitely changed, the
   poll clock is bypassed once, in exchange for the steady-state poll being allowed to stay
   slow.

The result is a client whose steady-state cost for watching one settled pull request is a
single 2 MiB-capped `gh pr checks` read every 20 seconds (every 120 seconds from another view),
with everything else answered from disk or from free 304s.

## Pagination and the Link header

### The header, byte by byte

REST listing endpoints return at most one page of results per request and describe the rest of
the collection in the `Link` response header, a comma-separated list of URL references with
relation types:

```text
Link: <https://api.github.com/repositories/1296269/issues/42/timeline?per_page=100&page=2>; rel="next",
      <https://api.github.com/repositories/1296269/issues/42/timeline?per_page=100&page=7>; rel="last"
```

The parts that matter to a client:

| Component | Meaning |
|---|---|
| `<...>` | The target URL, verbatim, including the query string |
| `rel="next"` | A following page exists; its absence means this is the final page |
| `rel="last"` | The URL of the final page, from which the page count can be read |
| `rel="prev"`, `rel="first"` | Present when walking backwards; unused by Quinjet |
| `per_page` | Page size, capped by GitHub at 100 for these endpoints |
| `page` | 1-based page number |

Two subtleties make naive parsing wrong. First, the `rel="last"` segment is only advertised
when GitHub can compute the total cheaply; a client must treat its absence as "unknown length",
not "one page". Second, the URL inside the segment contains both `per_page=100` and `page=7`,
and a substring search for `page=` would match `per_page=` first. Quinjet's `last_page`
(`src/git/github/mod.rs:700`) splits the URL on `?` and `&` and matches only whole parameters
with `strip_prefix("page=")`, which the test
`the_link_header_names_the_newest_timeline_page` (`src/git/github/mod.rs:3206`) pins: a header
where `per_page` precedes `page` still yields the page number, never the page size.

```rust
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

The companion `has_next_page` (`src/git/github/mod.rs:694`) needs only a containment check for
`rel="next"`, and `header_value` (`src/git/github/mod.rs:685`) does case-insensitive header
lookup, since HTTP header names are case-insensitive and `gh` passes them through as the server
sent them.

Reading headers at all requires asking for them: `gh api -i` prints the HTTP response head, a
blank line, then the body. `split_http_response` (`src/git/github/mod.rs:669`) splits at the
first `\r\n\r\n` or `\n\n`, tolerating both line conventions, and hands back the head as text
and the body as bytes.

### One bounded page

`Repository::api_page` (`src/git/github/mod.rs:1202`) is the page primitive every paged reader
shares: the pulls files endpoint, the timeline, and the review-comment stream. It runs one
`gh api -i` with `&page={page}` appended, splits head from body, and returns the pagination
facts alongside the data:

```rust
let (head, body) = split_http_response(&output.stdout);
let has_next = has_next_page(head.as_ref());
let mut data = body.to_vec();
if output.stdout_truncated {
    while data.last().is_some_and(|byte| *byte != b'\n') {
        let _ = data.pop();
    }
}
Ok(ApiPage {
    data,
    truncated: output.stdout_truncated,
    has_next,
    last_page: last_page(head.as_ref()),
})
```

The truncation repair loop is the page-level instance of the per-format repair rule: when the
2 MiB pipe cap cut the body mid-record, bytes are popped until the buffer ends at a newline, so
downstream TSV parsing only ever sees whole records. The doc comment on the method states the
contract: "One bounded page of a listing endpoint: its body trimmed to whole records, plus
whether GitHub advertises another page after it."

### Page order as a correctness tool

Quinjet's paged reads run under caps: the conversation keeps at most 500 entries
(`MAX_CONVERSATION_ENTRIES`, `src/git/github/conversation.rs:13`). A cap forces a choice about
which entries to drop, and the only defensible answer for an activity stream is: the oldest.
That answer is implemented purely through page order, encoded in the `ConversationPaging` enum
(`src/git/github/conversation.rs:99`), whose doc comment frames the problem: "How a stream
reaches its newest entries. Review comments accept a descending sort, so their newest page is
page one. The timeline API only serves oldest-first, so its newest page is the one
rel=\"last\" names."

- **NewestFirst.** The review-comment endpoint accepts server-side ordering:
  `repos/{owner}/{repo}/pulls/{n}/comments?per_page=100&sort=created&direction=desc`. Walking
  pages 1, 2, 3 forward is already newest-to-oldest, so stopping at the cap drops exactly the
  oldest comments.
- **LastPageFirst.** The issue timeline serves oldest-first with no sort parameter. Quinjet
  reads its first page (which doubles as the validated, ETag-checked read), takes `last_page`
  from the `Link` header, then walks pages `(2..=last).rev()`: the final page first, backwards
  toward page 2, appending page 1 (the oldest chunk) only if the whole backward walk fit under
  the cap.

The backward walk in `conversation_records` (`src/git/github/conversation.rs:283`):

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
```

When the timeline response advertises no `rel="last"` at all, the code degrades to the forward
walk (the `(LastPageFirst, None)` arm), which for a single-page or two-page thread is the same
set of entries anyway. ARCHITECTURE.md invariant 5 records the guarantee this buys:
conversations are "fetched newest-first in bounded pages (review comments descending, the
timeline from its last Link page) so the cap can only ever drop the oldest activity."

### Worked example: a seven-page timeline

Consider a long-running pull request whose timeline spans seven pages of 100 events, with the
review-comment stream small enough to fit in one page.

1. The validated first-page read of the timeline returns 100 records, `truncated == false`, a
   `Link` header with `rel="next"` and `rel="last"` naming page 7. Because `rel="next"` is
   present, the read is not `complete`, no validator is stored, and the page loop takes over
   with `first_data` set to page 1's records and `first_last_page == Some(7)`.
2. Pages 7, 6, 5, 4 are fetched in that order. After page 4 the running record count crosses
   500, so `complete = false` and the loop breaks before fetching page 3.
3. Because the walk was incomplete, page 1's oldest chunk is never appended. The collected
   buffer holds pages 7 through 4: the newest roughly 400 timeline records.
4. The merged conversation is capped to exactly 500 entries with the synthetic opened entry
   preserved at the front, and `truncated` is reported so the view can say the oldest activity
   was dropped.

Total spend: five requests for a seven-page thread, and the 500 entries kept are provably the
newest ones. A forward walk under the same cap would have kept pages 1 through 5 and silently
dropped the newest activity, which for a conversation view is the only part the reader
certainly wants. The full merge, dedupe, and cap mechanics live in
`./conversation-and-checks.md`; the strategy decision documented here is that page order,
driven entirely by the `Link` header and one sort parameter, is what makes a hard cap
compatible with a correct view.

## Conditional requests: the validated read

### ETags and If-None-Match in general

HTTP defines a validation model on top of caching: a response may carry an `ETag` header, an
opaque validator naming the exact representation returned. A client that stored the validator
can later ask the same URL with an `If-None-Match: <etag>` request header, and the server
answers one of two ways:

- `200 OK` with a full body and a new `ETag`: the resource changed, here is the new
  representation.
- `304 Not Modified` with no body: the stored representation is still exact; keep using it.

For GitHub's REST API the exchange has the extra property already quoted above: the 304 does
not count against the rate limit. A client that polls with validators pays bandwidth and budget
only for actual change. The protocol also distinguishes strong from weak validators (a `W/`
prefix marks weak ones); GitHub's list endpoints serve validators that `If-None-Match` accepts
as-is, and Quinjet stores and replays them verbatim without interpreting their strength,
because the only operation performed on a validator is equality at the server.

The classic client-side hazard with validators is not the protocol but the bookkeeping: the
validator and the body it validates must be stored and evicted as one unit. A cache that stores
them separately can present a new validator with an old body (the server says 304, the client
shows stale data with full confidence) or an old validator with a new body (harmless but
wasteful). The second hazard is pagination: an `ETag` names one response, meaning one page. A
client that stores page 1's validator next to a body assembled from pages 1 through 5, then
gets a 304 for page 1, would wrongly conclude the whole assembly is current.

Quinjet's implementation is built so both hazards are structurally impossible.

### validated_gh line by line

`Repository::validated_gh` (`src/git/github/mod.rs:605`) is quoted here in full because every
line carries policy:

```rust
pub(crate) fn validated_gh(&self, key: &str, args: Vec<OsString>) -> Result<ValidatedRead> {
    let cached = cache_read(key, CacheLife::Immutable);
    let validator = cached.as_ref().and_then(|entry| split_validator(entry).0);
    let mut request = vec![OsString::from("api"), OsString::from("-i")];
    if let Some(validator) = validator.as_ref() {
        request.push(OsString::from("-H"));
        request.push(OsString::from(format!("If-None-Match: {validator}")));
    }
    request.extend(args);

    let output = self.run_gh(request)?;
    if !output.status.success() && !output.stdout_truncated {
        bail!(
            "{}",
            bounded_command_error("unable to read from GitHub", &output)
        );
    }
    let (head, body) = split_http_response(&output.stdout);
    let head = head.as_ref();
    let status =
        String::from_utf8_lossy(head.lines().next().unwrap_or_default().as_bytes()).to_string();
    if status.contains(" 304")
        && let Some(entry) = cached
    {
        return Ok(ValidatedRead {
            data: split_validator(&entry).1.to_vec(),
            unchanged: true,
            complete: true,
            truncated: false,
            last_page: None,
        });
    }
    let complete = !output.stdout_truncated && !has_next_page(head);
    if let Some(etag) = header_value(head, "etag").filter(|_| complete) {
        let mut entry = etag.into_bytes();
        entry.push(b'\n');
        entry.extend_from_slice(body);
        cache_write(key, &entry);
    }
    Ok(ValidatedRead {
        data: body.to_vec(),
        unchanged: false,
        complete,
        truncated: output.stdout_truncated,
        last_page: last_page(head),
    })
}
```

Walking it:

**1. The cached entry is read under `CacheLife::Immutable`.** A validator entry never expires by
clock; its correctness is enforced by the server on every use, so age is irrelevant. Eviction
by the cache's global size pruning is the only way it disappears, and losing it merely costs
one full-body read.

**2. The validator rides `If-None-Match` only when one exists.** A first read, or a read after
eviction, is an ordinary `gh api -i` call: full body, full budget cost, and a chance to store a
validator for next time.

**3. A `304` status line short-circuits to the cached body.** The check is a containment test
for `" 304"` in the first line of the response head (the status line, e.g.
`HTTP/2.0 304 Not Modified`), guarded by the cached entry actually existing. The returned
`ValidatedRead` carries the stored body with `unchanged: true, complete: true`: the caller
learns both that it has current data and that it transferred nothing. The conversation layer
propagates that bit as `from_cache`, which is ultimately why the view can label a thread
`cached` even though a network round trip technically happened.

**4. `complete` means single whole page.** A response is complete only if the pipe did not
truncate it and the `Link` header advertises no `rel="next"`. Both conditions are essential:
a truncated body is missing records the server sent, and a page with a successor is missing
records the server did not send.

**5. The validator is stored only for complete responses.** The `filter(|_| complete)` on the
`ETag` header is the line that closes the pagination hazard. A multi-page listing's page 1
never earns a validator, so no future 304 can ever vouch for an assembly the validator does not
describe. The test `only_a_single_page_answer_is_worth_a_validator`
(`src/git/github/mod.rs:3168`) pins exactly this.

### A worked exchange, byte by byte

Tracing one review-comment stream through two polls makes the mechanics concrete. First poll,
no validator stored yet. The spawned command is:

```text
gh api -i "repos/acme/widget/pulls/42/comments?per_page=100&sort=created&direction=desc" --jq <TSV program>
```

`gh api -i` writes the response head, a blank line, then the jq-projected body to stdout. A
schematic of what crosses the pipe (headers abbreviated to the ones Quinjet reads):

```text
HTTP/2.0 200 OK
Etag: W/"a18c53411ba96caee2f851ba54561577"
Link: (absent: the thread fits in one page)

reviewer1<TAB>2026-08-19T10:02:11Z<TAB>src/lib.rs:14<TAB>...
reviewer2<TAB>2026-08-18T16:40:03Z<TAB>src/lib.rs:90<TAB>...
```

`split_http_response` cuts at the blank line. The status line does not contain `" 304"`, so
the fresh path runs: `complete` is true (no truncation, no `rel="next"`), the `Etag` header
value is taken verbatim, weak-validator prefix and quotes included, and the cache entry
written under `conversation-comments-validator-v2\n{url}\n{number}` is exactly:

```text
W/"a18c53411ba96caee2f851ba54561577"
reviewer1<TAB>2026-08-19T10:02:11Z<TAB>src/lib.rs:14<TAB>...
reviewer2<TAB>2026-08-18T16:40:03Z<TAB>src/lib.rs:90<TAB>...
```

One file, first line validator, everything after the first newline the body it validates.
(The `<TAB>` markers stand in for real tab bytes; the records are jq `@tsv` output.)

Second poll, twenty seconds later, nothing changed. `split_validator` recovers the first
line, and the spawned command grows one header argument:

```text
gh api -i -H "If-None-Match: W/\"a18c53411ba96caee2f851ba54561577\"" "repos/acme/widget/pulls/42/comments?per_page=100&sort=created&direction=desc" --jq <TSV program>
```

GitHub compares validators, matches, and answers with a head and no body:

```text
HTTP/2.0 304 Not Modified
Etag: W/"a18c53411ba96caee2f851ba54561577"
```

The status-line containment test fires, the stored body is returned with
`unchanged: true`, no bytes beyond the head crossed the network, and the request did not
count against the rate limit. The conversation layer reports `from_cache: true`, and if the
timeline stream answered the same way, the view's `cached` indicator stays honest.

Third poll, after a new comment. The validators no longer match, so GitHub answers `200` with
a full body and a new `Etag`; the old entry is overwritten wholesale with the new
validator-plus-body pair. At no point did the code compare timestamps, parse dates, or
guess at freshness: the server adjudicated every round.

### The storage format: validator and body in one entry

The entry format is the whole answer to the bookkeeping hazard: the cache stores
`{etag}\n{body}` as a single value under a single key. The doc comment above `ValidatedRead`
(`src/git/github/mod.rs:594`) says why: "The entry holds the validator on its first line and
the body after it, so the two can never be stored out of step with each other."

`split_validator` (`src/git/github/mod.rs:655`) is the entire decoder:

```rust
fn split_validator(entry: &[u8]) -> (Option<String>, &[u8]) {
    entry
        .iter()
        .position(|byte| *byte == b'\n')
        .map_or((None, entry), |index| {
            let (validator, body) = entry.split_at(index);
            (
                Some(String::from_utf8_lossy(validator).into_owned()),
                body.get(1..).unwrap_or_default(),
            )
        })
}
```

An entry with no newline degrades to "no validator, whole entry is body", which can only
happen if an entry was written by other code under the same key; the read then behaves like a
first read and overwrites it properly. Because cache writes are atomic (a temp file renamed
into place, see `./caching.md`), there is no window where the validator line exists without its
body or vice versa. The layout is byte-cheap (one separator byte of overhead), and it composes
with the rest of the cache machinery unmodified: the entry is bounded by the same 2 MiB
metadata limit, prefixed by the same magic, pruned by the same oldest-first sweep.

The test `a_cache_entry_keeps_its_validator_beside_the_body_it_validates`
(`src/git/github/mod.rs:3224`) locks the format down as an invariant rather than an
implementation detail.

One deliberate wrinkle: on a 304, `last_page` is returned as `None` even though the 304
response may carry headers. That is correct for how the caller uses it. A stored validator
only exists for a single-page response, so a 304 against it certifies "still one page"; there
is no last page to know about. If the collection has since grown past one page, the server
answers 200 with a body and a `Link` header, the `complete` computation sees `rel="next"`, no
new validator is stored, and the caller falls into its multi-page path with fresh pagination
facts.

### Who uses validated reads

The mechanism has exactly two callers, both in the conversation layer
(`src/git/github/conversation.rs:163`), because the conversation is the one stream with the
right shape for it: re-read often, usually unchanged, served from a stable URL, and fronted by
a content cache that already handles the "definitely unchanged" case for free.

The two cache keys per pull request are:

```text
conversation-timeline-validator-v2\n{repository url}\n{number}
conversation-comments-validator-v2\n{repository url}\n{number}
```

Note what is absent from these keys: the PR's `updated_at` stamp. The content-cache keys
(`conversation-timeline-v2\n{url}\n{number}\n{updated_at}`) include the stamp so that any PR
activity asks a different question; the validator keys deliberately do not, because the
validator's job is to survive across updates and let GitHub itself judge change. The two key
families form a three-layer read, cheapest first:

1. **Stamp hit, zero requests.** The metadata poll delivered an `updated_at`; if a content
   entry exists for that exact stamp, the thread is served from disk with no network at all.
2. **Validator hit, one free request.** The stamp moved (or the content entry was evicted),
   so page 1 is fetched with `If-None-Match`. An answering 304 serves the stored body,
   costs no budget, and reports `from_cache: true`.
3. **Full read, paid requests.** The thread actually changed; the page loop runs under the
   500-entry cap and the result is stored under the new stamp.

A subtle interaction: metadata moves `updated_at` for reasons that do not change the rendered
thread (a label change filtered out by the jq deny list, a base-branch update). Layer 1 misses,
but layer 2 turns what would have been a full multi-page re-read into one free 304 per stream.
This is the practical meaning of "re-checked as often as it is worth checking": the poll can
afford a 20-second conversation floor because the common poll outcome costs nothing.

### Failure modes the shape prevents

- **Stale body behind a fresh validator.** Impossible: they live in one atomic entry.
- **A partial page validated as whole.** Impossible: truncated responses and pages with a
  `rel="next"` successor never store a validator.
- **A validator poisoning the page loop.** An oversized or failed validated read degrades: the
  caller in `conversation_records` treats `Ok` with truncation and `Err` identically
  (`Ok(read) if !read.truncated => Some(read)`, everything else `None`) and falls back to the
  plain bounded page loop, so the conversation renders either way.
- **Cross-endpoint contamination.** Keys embed the repository URL and PR number, and the two
  streams have distinct key prefixes, so a timeline validator can never answer for the
  comments endpoint.
- **Cache unavailability.** Every cache helper is best-effort over an `Option<CacheStore>`;
  with no writable cache root the validated read silently becomes an ordinary read. Losing the
  optimization never loses the feature.

## The pulls files endpoint: line counts without blobs

### Why counts are an API problem

The Files view of a pull request shows every changed file as a header with its `+n -n` line
counts before any patch has loaded (invariant 8a for local diffs; the same rule carries to PR
diffs). Where do those counts come from?

For a pull request whose base and head commits already exist in the opened repository, the
answer is local and cheap: `git diff --numstat -z --find-renames {merge_base} {head}` reads
blobs that are already on disk. But for the other kind of pull request, the one prepared in a
disposable bare workspace (see `./pr-workspace.md`), the workspace was deliberately fetched
with `--filter=blob:none`: commits and trees arrived, file contents did not. Git's partial
clone machinery (documented from the transfer side in
`../git-internals/shallow-and-partial-clone.md`) makes any command that needs blob contents
fetch those blobs lazily, one round trip at a time, from the promisor remote.

`--numstat` needs blob contents for every changed file, because counting added and deleted
lines is a content diff. Running it in a blob-less workspace therefore triggers a lazy
download of every changed blob, old and new side, before a single count renders. On a large PR
that is thousands of blob fetches spent to produce numbers GitHub has already computed and
serves as plain integers. The doc comment on the method
(`src/git/github/mod.rs:1235`) compresses the whole argument into two sentences: "In the
blob-less disposable workspace a local `--numstat` would download every changed blob just to
count lines; GitHub already knows the totals."

This was PR #49's change ("perf: read per-file counts from the pulls files endpoint",
commit `56f4154`), whose body states the goal: "Read per-file additions and deletions from the
pulls files endpoint so a blob-less PR workspace no longer downloads every changed blob just to
show counts." ARCHITECTURE.md invariant 9 now carries the clause "per-file line counts come
from the pull-request files endpoint instead of a blob-materializing local numstat."

### The endpoint

`GET repos/{owner}/{repo}/pulls/{number}/files` lists the changed files of a pull request,
one JSON object per file, paginated like any listing endpoint. The fields Quinjet consumes:

| Field | Type | Use in Quinjet |
|---|---|---|
| `filename` | string | The post-image path, keying the counts map |
| `additions` | integer | Added-line count as GitHub computed it |
| `deletions` | integer | Deleted-line count as GitHub computed it |
| `status` | string | `added`, `modified`, `removed`, `renamed`, and so on; used only for the rename rule |

The endpoint has quirks a consumer must design around. GitHub reports `additions` and
`deletions` of `0` for files whose counts it declined to compute, for example very large or
generated files; a client that stores those zeros as real counts renders a confident, wrong
`+0 -0`. And one class of file legitimately has zero counts: a pure rename moves content
without changing a line. Both cases are handled in the parser below, and getting the second
one wrong is exactly what forced a cache key version bump in PR #55.

### The TSV pipeline

`Repository::pull_request_file_counts_from_api` (`src/git/github/mod.rs:1238`) is the complete
pipeline, quoted from the source:

```rust
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
let mut collected: Vec<u8> = Vec::new();
let mut complete = false;
for page in 1..=MAX_FILE_COUNT_PAGES {
    let read = self
        .api_page(&endpoint, jq, page, "unable to list pull-request files")
        .ok()?;
    if read.truncated {
        return None;
    }
    collected.extend_from_slice(&read.data);
    if collected.last().is_some_and(|byte| *byte != b'\n') {
        collected.push(b'\n');
    }
    if !read.has_next {
        complete = true;
        break;
    }
}
if complete && collected.len() <= MAX_PR_PATH_BYTES {
    cache_write_bounded(&key, &collected, MAX_PR_PATH_BYTES);
}
Some(parse_api_file_counts(&collected))
```

Every decision in order:

**1. The guards make the key immutable.** Before any of this runs, both `base_oid` and
`head_oid` must pass `is_commit_oid` and the repository must have a `name_with_owner`.
Requiring full commit OIDs is what entitles the cache entry to `CacheLife::Immutable`: the
files endpoint's answer for a fixed (base, head) pair can never change, because commits are
content-addressed and immutable (the deeper story is in
`../git-internals/object-model.md`). A force-push produces a new `head_oid` and therefore a
new key: a different question, not a stale answer. Any guard failure returns `None` and the
caller falls back to local numstat, which is always correct, merely expensive.

**2. The jq program flattens each file to a 4-field TSV record.** The response for 100 files
becomes 100 short lines instead of a large JSON array, and the record shape is fixed at the
producer. The reduction happens inside `gh`, before the pipe cap is charged.

**3. Pagination is bounded at `MAX_FILE_COUNT_PAGES = 64`.** With `per_page=100` that is at
most 6,400 files' worth of counts, aligned with the rest of the bounded-index philosophy (the
changed-file index itself admits at most 16,384 paths). A PR beyond the page cap gets counts
for its first 6,400 files and skeleton placeholders for the rest, which the #55 backfill then
fills from arrived patches.

**4. A pipe-truncated page aborts the whole read.** `if read.truncated { return None; }` is
stricter than the repair `api_page` already performed. The repair guarantees whole records,
but a truncated page means an unknown number of files are simply missing from the middle of
the listing; partial counts presented as authoritative would render some files with counts and
others with placeholders on no principled boundary, and, worse, would be cached that way. The
fallback (local numstat, or no counts plus backfill) is honest; the truncated read is not.

**5. Pages are joined on record boundaries.** Each page's data is appended and the buffer is
newline-terminated if the page did not end with one, so page joins can never fuse the last
record of one page with the first record of the next.

**6. Only a complete accumulation is cached.** `complete` is only set when a page arrives
without `rel="next"`. A read that ran out of page budget still returns its partial map (the
counts it did fetch are correct and useful this session) but writes nothing, so the cache
never holds an answer that silently under-describes the pull request. The cache write is
bounded by `MAX_PR_PATH_BYTES` (8 MiB), the same ceiling the changed-file listings use.

**7. The result is returned even when incomplete.** The final `Some(parse_api_file_counts(...))`
sits outside the `complete` check. Counts are a rendering enhancement, never a correctness
requirement; six thousand real counts with placeholders past them beats falling back to a
blob-materializing numstat for the whole index.

### The parser and the 0/0 rule

`parse_api_file_counts` (`src/git/github/mod.rs:1918`):

```rust
fn parse_api_file_counts(data: &[u8]) -> HashMap<PathBuf, DiffLineCounts> {
    let mut counts = HashMap::new();
    for record in data.split(|byte| *byte == b'\n') {
        if record.is_empty() {
            continue;
        }
        let Ok([path, additions, deletions, status]) = parse_tsv_record::<4>(record) else {
            continue;
        };
        let (Ok(additions), Ok(deletions)) = (additions.parse(), deletions.parse()) else {
            continue;
        };
        if additions == 0 && deletions == 0 && status != "renamed" {
            continue;
        }
        let _ = counts.insert(
            PathBuf::from(path),
            DiffLineCounts {
                additions,
                deletions,
                binary: false,
            },
        );
    }
    counts
}
```

Malformed records are skipped, not fatal: a record that fails the 4-field TSV parse or whose
numbers do not parse is dropped and every other record still counts. The interesting line is
the skip rule, which encodes both endpoint quirks:

- `additions == 0 && deletions == 0` with any status other than `renamed` means "GitHub could
  not or did not count this file". Storing nothing leaves the file's `counts` as `None`, which
  renders as the two-middle-dot loading skeleton (`+·· -··`, `src/git/diff.rs`) and remains
  eligible for backfill: when the file's real patch arrives, `backfill_pull_request_counts`
  (`src/app.rs:5881`) counts its added and removed lines and fills the header in.
- A `renamed` record with 0/0 is kept, zeros and all. A pure rename genuinely changed zero
  lines; `+0 -0` is its true answer, and dropping it would show a loading skeleton that no
  later patch would meaningfully improve on.

The test `api_file_counts_parse_and_skip_malformed_records` (`src/git/github/mod.rs:3177`)
feeds the parser a mixture of well-formed, broken, non-numeric, countless, and pure-rename
records and pins the outcome: malformed and countless records are skipped, pure renames are
kept with their zero counts.

### Why the cache key is v3

The key history is a compact lesson in immutable-key cache management:

- **#49 shipped `pr-file-counts-v2`** with a 3-field jq program (no `status`) and a blanket
  rule dropping every 0/0 record. That rule was right for countless files and wrong for pure
  renames, which were indistinguishable without the status field: a renamed file showed the
  unknown-counts placeholder forever.
- **#55 added `.status` to the jq program, widened the parser to 4 fields, added the
  `renamed` exception, and bumped the key to `pr-file-counts-v3`.**

The bump is the load-bearing move. These entries are `CacheLife::Immutable`: no TTL will ever
age out a v2 entry, and its bytes are not wrong as bytes, they are wrong as an interpretation
(three fields where the new parser wants four, renames absent where the new rule wants them
present). Changing the key version orphans every v2 entry at once; they linger harmlessly
until the oldest-first size pruning removes them, and every post-#55 read builds a v3 entry
with the status column. This is the general rule for immutable caches: when the schema or the
semantics of an entry change, you do not migrate or invalidate, you rename the question.

The full key, one line per component:

```text
pr-file-counts-v3        schema version
{repository url}         which GitHub host and repository
{number}                 which pull request (the endpoint is number-addressed)
{base oid}\n{head oid}   which exact pair of commits the counts describe
```

The number and the OID pair are both present because the endpoint is addressed by number while
the answer is determined by the OIDs; keying by both means a reopened PR number on a different
repository, or the same number after a force-push, can never collide.

### Worked example: counts for a 2,188-file pull request

The benchmark target for the optimization stack, `oven-sh/bun` PR #30412 (2,188 changed
files, over a million added lines; the full story is in `../benchmarking.md`), makes the
arithmetic concrete:

1. The metadata read supplies `base_oid`, `head_oid`, and `changedFiles = 2188`. Both OIDs
   pass `is_commit_oid`; the counts pipeline runs before the workspace fetch starts.
2. Cache miss on the v3 key (first visit). The page loop issues
   `repos/oven-sh/bun/pulls/30412/files?per_page=100&page=1` through `page=22`: pages 1
   through 21 advertise `rel="next"`; page 22, holding the last 88 records, does not, so
   `complete = true` after 22 requests, well under the 64-page cap.
3. Each page contributed roughly 100 TSV lines of a few dozen bytes each; the accumulated
   buffer is far below the 8 MiB write bound and is cached under the immutable v3 key.
4. The parser builds a map of about 2,188 entries, minus any countless records GitHub
   declined to total, which stay `None` and wait for patch backfill.
5. `changed_files_in_repository` (`src/git/github/mod.rs:1981`) enumerates paths with
   `git diff --name-status -z` in the blob-less workspace (name-status needs trees only, no
   blob contents) and attaches `counts.get(&path).copied()` to each file.
6. Every header in the Files tree renders its real `+n -n` immediately. Zero blobs were
   downloaded for counts. The blobs that do get downloaded later are the ones patches
   actually need, batched and byte-budgeted by the prefetcher (`./prefetch.md`).
7. Any revisit of the same PR at the same head serves step 2 from disk: zero requests.

Against the pre-#49 behavior, the same view would have started with a `--numstat` that lazily
fetched both sides of every one of the 2,188 changed files before a single count rendered.
The API path replaces an O(files) sequence of blob round trips with ceil(files / 100)
metadata requests, and then amortizes even those to zero via the immutable cache.

### What the counts feed

The counts map earns its keep three more times after the headers render:

- **Prefetch batch sizing.** `estimated_patch_bytes` (`src/app.rs:7052`) turns counts into a
  patch-size estimate: `(additions + deletions) * 80 + 4,096` bytes, with a 512 KiB fallback
  for a file with no counts. Batches fill until the 6 MiB estimate budget or 32 files,
  whichever binds first, keeping each combined `git diff` read safely under the hard 8 MiB
  pipe cap. Without per-file counts every file would cost the 512 KiB fallback estimate,
  collapsing batches to at most a dozen files regardless of their real size.
- **Total-count honesty.** When the changed-file index is truncated at its own caps,
  `total_files` is taken as the maximum of the API's `changedFiles` and the parsed count, so
  the header never understates the pull request.
- **Backfill targets.** Files whose counts stayed `None` are exactly the set the #55 backfill
  scans for when patches arrive, closing the loop between the API's declined records and
  ground truth computed from patch bytes.

## The compare API: merge bases as metadata

### The problem a PR diff has to solve

A pull-request diff is not `base_oid` against `head_oid`. It is the merge base of the two
against `head_oid`: the three-dot semantics of `git diff base...head`, showing what the PR
introduces rather than mixing in everything the base branch did since the fork point. (The DAG
theory, multiple merge bases, and criss-cross histories are covered in
`../git-internals/merge-bases-and-history.md`; the [git-merge-base
manpage](https://git-scm.com/docs/git-merge-base) is the primary reference.)

Locally, `git merge-base A B` walks commit ancestry, which requires the ancestry to be present.
That is precisely what the disposable PR workspace does not have: it was created empty and
fetched shallow. A shallow fetch at depth 64 holds the last 64 commits of each side, and if the
fork point lies further back, both walks hit the shallow boundary before meeting. The classic
fix is deepening: fetch more history and retry. Quinjet implements that ladder as a fallback
(depths 64, 256, 1,024, 4,096, 16,384, then a refusal), but deepening is exactly the transfer
the shallow fetch existed to avoid, and for a long-lived branch it can mean shipping tens of
thousands of commits to answer a question with a 40-character answer.

GitHub already knows the answer. The compare endpoint,
`GET repos/{owner}/{repo}/compare/{base}...{head}`, implements the same three-dot semantics
server-side and reports `merge_base_commit.sha` in its response. One metadata request replaces
the entire ladder. The doc comment on the method (`src/git/github/mod.rs:1285`) frames it: "One
metadata request replaces the deepening fetch ladder, which cannot reach a merge base thousands
of commits behind either tip."

### merge_base_from_api

`Repository::merge_base_from_api` (`src/git/github/mod.rs:1288`):

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
let output = self
    .run_gh([
        OsString::from("api"),
        OsString::from(format!(
            "repos/{}/compare/{base}...{head}",
            repository.name_with_owner
        )),
        OsString::from("--jq"),
        OsString::from(".merge_base_commit.sha"),
    ])
    .ok()?;
if !output.status.success() || output.stdout_truncated {
    return None;
}
let sha = String::from_utf8_lossy(trim_ascii(&output.stdout)).into_owned();
if !is_commit_oid(&sha) {
    return None;
}
cache_write(&key, sha.as_bytes());
Some(sha)
```

The design points:

**1. It is a hint, so every failure is `None`.** A network error, a non-success exit, a
truncated pipe, or output that does not parse as a commit OID all return `None`, and the
caller proceeds to the fetch ladder. The API path is an optimization with a complete fallback
behind it; no GitHub outage can make a PR diff impossible, only slower.

**2. The answer is validated on both sides of the cache.** The fresh response must pass
`is_commit_oid` before it is cached or used, and, unusually, the cached value is re-validated
on every read too. A corrupted or foreign entry under this key degrades to a cache miss
instead of injecting a non-OID string into a `git fetch` refspec. Nothing that has not passed
the 40-or-64-hex-digit check is ever interpolated into an argv.

**3. The key is the OID pair, so the entry is immutable.** The merge base of two fixed commits
is a mathematical fact about the DAG; `pr-merge-base-v1\n{url}\n{base}\n{head}` can never go
stale, only get evicted. A force-push changes `head`, which changes the key.

### The depth-1 point fetch

The hint's consumer is `fetch_pull_request` (`src/git/github/mod.rs:1781`). After the PR head
is fetched (the synthetic `refs/pull/{number}/head` ref at depth 64), the hint short-circuits
the base side entirely: the workspace fetches the refspec
`+{hint}:refs/quinjet/merge-base` at `--depth=1`. That is one commit object, its trees, and,
because every workspace fetch runs with `--filter=blob:none`, no blobs at all. If that fetch
succeeds and the fetched head still resolves to the advertised `head_oid`
(`preferred_fetched_commit`, `src/git/github/mod.rs:1949`, pins the exact commit the metadata
described even if the branch moved meanwhile), the function returns immediately: no base
branch history is fetched, at any depth.

The fetch flags themselves, from `fetch_ref` (`src/git/github/mod.rs:1876`):

```rust
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
```

If the server rejects the filter (no partial-clone support), the identical command is retried
without `--filter=blob:none`, keeping the depth bound either way. The transfer-level anatomy
of these fetches, and why a depth-1 blob-less fetch of one commit is nearly free, is the
subject of `../git-internals/shallow-and-partial-clone.md`; the
[git-fetch manpage](https://git-scm.com/docs/git-fetch) documents the flags.

### The ladder as the honest fallback

When the API declines (fine-grained token without the endpoint, GitHub Enterprise quirk,
network failure) or the depth-1 fetch cannot verify the head, the code falls back to fetching
the base ref and deepening both sides through `[64, 256, 1_024, 4_096, 16_384]`, probing
`git merge-base` after each step and giving up past 16,384 commits with the error "Unable to
find the PR merge base within 16,384 commits; refusing an unbounded history fetch". The
refusal is itself a rate-limit-adjacent decision, applied to Git's transfer budget rather than
the REST budget: no single pull request is allowed to cost unbounded history transfer. The
16,384 ceiling is recorded in ARCHITECTURE.md invariant 5 as "adaptive selected-PR history
fetches at 16,384 commits", and invariant 9 describes the whole arrangement: "the merge base
is resolved through the GitHub compare API and fetched as one depth-1 commit, a failed API
resolution falls back to fetching the base ref and deepening both sides adaptively."

The sequencing in `prepare_pull_request_diff` (`src/git/github/mod.rs:767`) is also
deliberate: both API hints, the merge base and the per-file counts, are requested before the
temporary repository is even created. The metadata round trips overlap with nothing and the
fetch that follows is shaped by their answers.

## Checks endpoints: exit codes as data

### gh pr checks and exit codes 1 and 8

The check list is the one read on this page where the transport's own conventions get in the
way. `gh pr checks` reports check status through its exit code as well as its output: it exits
0 when everything passed, 1 when any check failed, and 8 when checks are still pending. For a
scripting user that is a feature. For Quinjet it means a perfectly good answer arrives looking
like a failure: the standard cached wrapper, which treats a non-zero exit as an error and
would fall back to a stale cache entry, cannot be used. The doc comment on
`pull_request_checks` (`src/git/github/checks.rs:199`) states the consequence: "`gh pr
checks` exits non-zero when any run failed, so a useful response has to be recognized by its
content rather than by the exit status. That is why this reads `gh` directly instead of going
through the cached helper, and caches the accepted body itself."

The acceptance logic (`src/git/github/checks.rs:222`):

```rust
let output = self.run_gh(pull_request_checks_args(pull_request))?;
let accepted_status = output.status.success()
    || matches!(output.status.code(), Some(1 | 8)) && !output.stdout.is_empty();
if output.stdout_truncated {
    bail!("pull-request checks exceeded the metadata limit");
}
if !accepted_status {
    let error = String::from_utf8_lossy(&output.stderr);
    if error.to_ascii_lowercase().contains("no checks") {
        return Ok(PullRequestChecks::default());
    }
    bail!(
        "{}",
        bounded_command_error("unable to load pull-request checks", &output)
    );
}
```

Reading the conditions in order:

- **Exit 0 is accepted unconditionally.** All checks passed.
- **Exit 1 or 8 is accepted only with a non-empty body.** These codes mean "failed checks
  exist" and "pending checks exist": states of the pull request, not of the command. The
  non-empty-stdout guard matters because the same codes could in principle accompany a real
  failure that produced no listing; an empty body with exit 1 falls through to the error
  path rather than being parsed into an empty check list that would render as "no checks".
- **Truncation is fatal.** The check list is metadata under the 2 MiB cap; a list that
  overflows it is not partially usable the way a log is, so the read errors rather than
  presenting a silently incomplete list.
- **A rejected status with "no checks" in stderr is an empty success.** A pull request without
  CI is a normal state, not an error to render in red.

The accepted body, a TSV of `[name, workflow, state, bucket, description, link, started_at,
completed_at]` records, is cached by hand under `checks-v1\n{url}\n{number}\n{head_oid}` with
`CacheLife::Ttl(CHECK_LIST_CACHE_TTL)`, a 30-second clock. Check state is the one genuinely
time-varying answer in the GitHub module, and the doc comment on the TTL
(`src/git/github/checks.rs:12`) says exactly that: "Check state is the one thing here that
genuinely changes minute to minute, so it is the one thing kept on a clock rather than on an
identity." The key still embeds `head_oid`, so a force-push invalidates check state instantly
rather than waiting out even those 30 seconds: the new head is a different question.

### The jobs endpoint: steps as decoration

Selecting a check opens its run: step structure from the Actions jobs endpoint plus the raw
log. The steps read, `check_run_steps` (`src/git/github/checks.rs:308`), is a standard cached
`gh api repos/{owner}/{repo}/actions/jobs/{job}` call with a jq program flattening
`.steps[]?` into 6-field TSV records. Two strategy details:

- **The cache life is part of the cache key.** The key is
  `check-steps-v1\n{repository}\n{job}\n{life:?}`, where `life` is `Immutable` for a settled
  run and `Ttl(Duration::ZERO)` for a running one. Embedding the `Debug` form of the life in
  the key means the two modes never share entries: a running job's zero-TTL key exists only to
  satisfy the helper's shape (it is never fresh, so every tail read refetches), while the
  settled key is immutable and answers from disk forever.
- **Failure degrades to an empty step list.** An error from the helper returns `Ok(Vec::new())`
  rather than propagating: steps are decoration over the log, and a log without steps still
  renders as loose lines. A view that refused to show a log because a metadata endpoint
  hiccuped would have inverted the priority.

### The logs endpoint: 404 and 410 as states

The raw log read, `check_run_raw_log` (`src/git/github/checks.rs:332`), fetches
`repos/{owner}/{repo}/actions/jobs/{job}/logs` with the dedicated 8 MiB cap
(`MAX_CHECK_LOG_BYTES`), using `gh api --allow-escape-sequences` so runner ANSI bytes pass
through to Quinjet's own stripper, and retrying once without the flag when an older `gh`
rejects it ("unknown flag" sniffed in stderr).

The endpoint's status codes encode a lifecycle, and the client must read them that way
(`log_not_published`, `src/git/github/checks.rs:381`, with its doc comment: "GitHub answers
the log endpoint with 404 until a job has finished writing its archive, and with 410 once
retention expires. Neither is a failure worth showing: the run itself is still readable from
its steps."):

| Response | Job state | Quinjet's reading |
|---|---|---|
| 404 / "not found" | First seconds of a run, before the blob exists | Empty log, `log_pending` set, retry on the tail cadence |
| 200 with partial body | Running; GitHub serves what is written so far | Render it; re-reading is the tail |
| 200 with full body | Settled | Parse, render, cache immutably under `check-log-v1\n{repo}\n{job}` |
| 410 / "gone" | Retention expired | Empty log, steps still render |
| Anything else failed | Real error | Surfaced as "unable to read the check run log" |

The strategy consequence is the tail: a running job's log is read with `Ttl(Duration::ZERO)`,
never cached, and simply re-fetched on the 8-second log floor; each read returns the whole
blob written so far, so "tailing" is nothing more than repetition. A settled run's log is
written to cache once (only when non-empty and not truncated) and never re-read from the
network again, which invariant 12 states from the cache side: "A run still in progress is
never cached, because re-reading it is what tails it."

The final checks-related read is the warm-up: `prefetch_check_run_logs`
(`src/git/github/checks.rs:293`) walks the settled Actions-backed checks and reads each log
once so that selecting any of them later is a disk read:

```rust
checks
    .iter()
    .filter(|check| !check.status.is_running() && check.job_id().is_some())
    .take(MAX_PREFETCHED_CHECK_LOGS)
    .take_while(|_| wanted())
    .filter(|check| self.pull_request_check_log(pull_request, check).is_ok())
    .count()
```

Every clause is a rate-limit decision. Running jobs are skipped because "their output is not
cacheable, and re-reading it here would spend requests the live tail is about to spend
anyway" (the method's doc comment). The 32-job ceiling (`MAX_PREFETCHED_CHECK_LOGS`) bounds
what one pull request may spend in the background, and the app layer enforces it per PR
across refreshes by remembering warmed job identities. The `take_while(|_| wanted())` polls a
generation-backed closure before each job, so the moment the reader opens a different pull
request the in-flight warm-up stops mid-list instead of finishing its budget for a view
nobody is watching. The warm-up runs on its own worker lane, last in the mailbox priority
order, so it can never delay a read the reader is waiting on (`../rendering/concurrency.md`
covers the lanes; `./conversation-and-checks.md` covers log parsing and step attachment).

## Caps on every read

### The cap table

Every number below is a named constant in the source; together they are the enforcement of
ARCHITECTURE.md invariants 5 and 6. Nothing GitHub or Git produces reaches Quinjet's memory
without passing one of these bounds.

| Cap | Value | Constant, location | What it bounds |
|---|---|---|---|
| Metadata stdout | 2 MiB | `MAX_GH_METADATA_BYTES`, `src/git/github/mod.rs:33` | Default `gh` read and default cache entry bound |
| gh stderr | 256 KiB | `MAX_GH_ERROR_BYTES`, `src/git/github/mod.rs:36` | Error text kept from any `gh` child |
| PR title | 16 KiB | `MAX_PULL_REQUEST_TITLE_BYTES`, `src/git/github/mod.rs:34` | Title after TSV parse |
| PR description | 256 KiB | `MAX_PULL_REQUEST_BODY_BYTES`, `src/git/github/mod.rs:35` | Body after TSV parse |
| File listings | 8 MiB | `MAX_PR_PATH_BYTES`, `src/git/github/mod.rs:37` | name-status, numstat, and API count listings, and their cache entries |
| Changed-file index | 16,384 entries | `MAX_PR_PATHS`, `src/git/github/mod.rs:38` | Files parsed into the PR index |
| API count pages | 64 pages | `MAX_FILE_COUNT_PAGES`, `src/git/github/mod.rs:39` | Pulls files pagination (6,400 files at per_page=100) |
| Patch read | 8 MiB | `MAX_DIFF_BYTES`, `src/git/mod.rs:25` | Any single or batched `git diff` patch |
| Cached per-file patch | 1 MiB | `MAX_CACHED_PATCH_BYTES`, `src/git/github/mod.rs:42` | Patch cache admission, so one file cannot crowd out a PR |
| Check log | 8 MiB | `MAX_CHECK_LOG_BYTES`, `src/git/github/checks.rs:11` | Raw log read, cache read, cache write |
| Check log lines | 200,000 | `MAX_CHECK_LOG_LINES`, `src/git/github/checks.rs:17` | Parsed log lines |
| Conversation entries | 500 | `MAX_CONVERSATION_ENTRIES`, `src/git/github/conversation.rs:13` | Merged thread length |
| Conversation body | 64 KiB | `MAX_CONVERSATION_BODY_BYTES`, `src/git/github/conversation.rs:14` | One entry's prose |
| Conversation context | 8 KiB | `MAX_CONVERSATION_CONTEXT_BYTES`, `src/git/github/conversation.rs:15` | One review comment's diff hunk |
| Warmed logs | 32 | `MAX_PREFETCHED_CHECK_LOGS`, `src/git/github/checks.rs:16` | Background log reads per PR |
| Remotes inspected | 32 | `MAX_GIT_REMOTES`, `src/git/github/mod.rs:29` | `git remote` walk |
| Remote URL pairs | 64 | `MAX_REMOTE_URL_ENTRIES`, `src/git/github/mod.rs:30` | (remote, url) pairs collected |
| Distinct URLs | 32 | `MAX_REMOTE_URLS`, `src/git/github/mod.rs:31` | Sanitized URLs resolved |
| Repositories | 16 | `MAX_GITHUB_REPOSITORIES`, `src/git/github/mod.rs:32` | Distinct identities loaded |
| Cache total | 128 MiB / 2,048 entries | `MAX_CACHE_BYTES`, `MAX_CACHE_ENTRIES`, `src/git/github/mod.rs:46` | On-disk store, pruned oldest first |
| Fetch stdout | 128 KiB | inline, `src/git/github/mod.rs:1887` | Workspace `git fetch` chatter |
| History deepening | 16,384 commits | ladder, `src/git/github/mod.rs:1848` | Merge-base search depth |

Three cross-cutting rules give the table its coherence:

**1. A read's cap is also its cache bound.** `checked_cached_gh_bounded`'s doc comment
(`src/git/github/mod.rs:1102`) states it: "`limit` bounds both the response Quinjet will read
and the entry it will keep, so a check log can use the cache without letting metadata grow."
The cache read side self-heals when limits shrink: a file larger than its caller's limit is
deleted on sight and treated as a miss.

**2. Every cap kills rather than drains.** All of these flow through `run_bounded_command`,
so crossing a cap terminates the child. Invariant 6: "Crossing a cap kills the child rather
than first allocating all output and truncating afterward."

**3. Truncation repair is format-specific.** A cap cuts a stream at an arbitrary byte, and
each format restores a parseable suffix or prefix its own way:

| Format | Repair | Where |
|---|---|---|
| TSV pages | Pop bytes to the last newline | `api_page`, `src/git/github/mod.rs:1222` |
| NUL-separated listings | Discard everything after the last NUL | `changed_files_in_repository`, `src/git/github/mod.rs:2019` |
| Unified diff patches | Pop bytes to the last newline, flag the document | `diff_selected_paths`, `src/git/github/mod.rs:2141` |
| Batched patches | Only the final `diff --git` section may be incomplete; it is retried alone rather than cached | `diff_files`, `src/git/github/mod.rs:440` |
| Metadata records | No repair; truncation is an error | `checked_cached_gh_bounded`, `src/git/github/mod.rs:1158` |
| Raw logs | None needed; the parser is line-oriented and the document carries `truncated` | `parse_check_log`, `src/git/github/checks.rs:474` |

### Why the caps are sized the way they are

The values form deliberate ratios rather than independent guesses. The 6 MiB prefetch
estimate budget sits under the 8 MiB patch pipe cap so a batch's real output has headroom
before truncation. The 1 MiB per-file patch cache ceiling sits far under the 128 MiB store so
"one file cannot crowd out the rest of a pull request" (the constant's own doc comment,
`src/git/github/mod.rs:40`). The 2 MiB metadata cap is generous for TSV records measured in
hundreds of bytes but small enough that a misdirected read (someone pointing a metadata
reader at a log-sized response) fails fast instead of ballooning the cache. The 16,384-entry
index cap matches the local diff index cap (`MAX_DIFF_INDEX_FILES`, `src/git/mod.rs`), so PR
and local views degrade identically on pathological inputs. And the 64-page count cap trades
completeness on outlier PRs for a hard bound on API spend, backstopped by the patch-derived
count backfill for anything past it.

## The cached wrapper: fresh, network, stale

### One helper, three dispositions

Most metadata reads share one shape: try the cache, maybe call the network, decide what a
failure means. `checked_cached_gh_bounded` (`src/git/github/mod.rs:1104`) encodes that shape
once, and its return type names the three possible provenances of the bytes it hands back
(`CacheDisposition`, `src/git/github/mod.rs:213`): `Fresh` (served from cache within its
life), `Network` (a live answer, now cached), and `Stale` (the network failed and an old entry
was substituted). The decision core:

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

The rules worth naming:

**1. `refresh` cannot bypass an immutable entry.** The condition
`!refresh || life == CacheLife::Immutable` means a forced refresh re-reads TTL entries but
still serves immutable ones from disk. An immutable entry cannot be wrong (its identity is in
its key), so refreshing it would spend a request to receive identical bytes. This is why a
poll can pass `refresh: true` freely: only the genuinely time-varying reads pay.

**2. Stale beats broken, twice.** Both failure paths, a spawn error (no `gh` installed, no
network) and a non-success exit, fall back to any cached entry regardless of its age, tagged
`Stale`. `CacheLife::accepts` filters freshness for the happy path only; the entry stays on
disk after its TTL exactly so it can serve this role. The UI translates the disposition into
the warning "GitHub is unavailable; showing stale cached metadata for #N" rather than a blank
pane. Offline, a previously visited pull request still opens.

**3. Truncation with no fallback is a hard error.** Metadata that overflows 2 MiB is not
partially meaningful, and caching it would poison future reads, so with no cached entry to
fall back on the read fails with "output exceeded the metadata limit".

The `Fresh` and `Stale` dispositions surface in the UI as the `from_cache` flags on
snapshots, which drive the `cached` indicator (invariant 12b). The wrapper's callers are the
metadata reads with TTLs: `gh pr view` under `pull-request-v3` (5 minutes), `gh repo view`
under `repository` (24 hours), and the check steps read described above. The reads that do
not fit the shape bypass it deliberately: `gh pr checks` (exit codes as data), the validated
conversation reads (judged by HTTP status, not exit status), and the raw log (its own cap and
its own 404 semantics). The full cache taxonomy, disk layout, and pruning policy live in
`./caching.md`.

### CacheLife as an API-strategy concept

The enum itself (`src/git/github/mod.rs:222`) is two variants and one doc comment, and it is
the sharpest single statement of the caching philosophy:

```rust
/// How long an entry stays usable. `Immutable` is for content whose identity is
/// already in its key: a finished run's log, or a patch between two fixed
/// commits. Such an entry can never become wrong, only evicted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CacheLife {
    Immutable,
    Ttl(Duration),
}
```

Applied to the API strategy: every read on this page was designed backwards from the question
"can its key name its content?". OID pairs, job identifiers of settled runs, and
activity-stamped threads can, so those reads are aggressive (fetched once, kept forever,
refresh-proof). Repository identity, PR metadata, and check state cannot, so those reads get
clocks sized to their real rate of change (a day, five minutes, thirty seconds). The poll
below is cheap precisely because the expensive content all lives on the immutable side.

### The metadata read, end to end

The pull-request metadata read is the wrapper's flagship caller and worth tracing as one
piece, because every transport mechanism on this page participates in it.
`pull_request_metadata` (`src/git/github/mod.rs:824`) reads under the key
`pull-request-v3\n{repository url}\n{number}` with `CacheLife::Ttl(PULL_REQUEST_CACHE_TTL)`,
a 5-minute clock, and issues `gh pr view {number} --repo {selector} --json <fields> --jq
<template>` on a miss. The jq template flattens the response to one TSV record whose 18
fields arrive in this order:

| Position | Field | Notes |
|---|---|---|
| 1 | `number` | Parsed as `u64`; the record count must be exactly one |
| 2 | `title` | Bounded to 16 KiB after unescaping |
| 3 | `body` | Bounded to 256 KiB; becomes the synthetic opened entry's prose |
| 4 | `author` | Login |
| 5 | `state` | Uppercased; drives the settled gate and the primary action |
| 6 | `isDraft` | Boolean |
| 7 | `updatedAt` | The activity stamp the conversation cache keys on |
| 8 | `url` | Also the shape marker recents scavenging recognizes |
| 9 | `baseRefName` | Fetch refspec input |
| 10 | `headRefName` | Fetch refspec input |
| 11 | `headRepository` | Fork identity; empty when the fork was deleted |
| 12 | `isCrossRepository` | Boolean |
| 13 | `additions` | PR-level total, feeds header and honesty checks |
| 14 | `deletions` | PR-level total |
| 15 | `changedFiles` | Backstops `total_files` on truncated indexes |
| 16 | `baseRefOid` | Half of every immutable diff cache key |
| 17 | `headRefOid` | The other half; a force-push changes it |
| 18 | `createdAt` | The opened entry's timestamp |

The response is required to contain exactly one record ("GitHub returned N records for pull
request #M" otherwise), which catches the failure mode of a jq program silently matching
nothing or everything. Fields 16 and 17 are the payload the rest of the strategy runs on:
they gate the network-free local path (`has_commit` probes), key the merge-base and counts
hints, key every patch and listing cache entry, and their change is the one signal that
reindexes a diff.

The 5-minute TTL is a considered number. Shorter would make the poll's silent lookups
(every 20 seconds while unsettled) hit the network more often for fields that change rarely;
longer would delay noticing a force-push or a title edit beyond what a live view can justify.
The poll sidesteps the tension entirely by passing `refresh: true`, which bypasses the TTL
for its explicit reads while leaving the TTL to serve the cheap cases: reopening a recently
viewed PR, the `--pr` launch path, and CLI subcommands sharing the same cache. And when the
network fails, the wrapper's stale path plus the snapshot's `from_cache` flag turn the last
cached record into a labeled degraded view instead of an error state.

## The adaptive poll

### The cadence problem

A live pull-request view has to reconcile three facts: check state changes in seconds while a
run executes, a settled pull request changes on human timescales, and a pull request the
reader is not even looking at barely needs to change at all. One fixed poll interval either
wastes budget on the quiet cases or lags on the active one. Quinjet's answer is an adaptive
tick with per-stream floors, plus two hard stops, all in `src/app.rs`.

The tick interval, `pull_request_poll_interval` (`src/app.rs:2985`):

```rust
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

The constants (`src/app.rs:42`): `PULL_REQUEST_ACTIVE_POLL = 5 s` while any check is running,
`PULL_REQUEST_IDLE_POLL = 20 s` once checks settle, `PULL_REQUEST_BACKGROUND_POLL = 120 s`
when the reader is in another view. The doc comment above them frames the three tiers: "A run
in progress changes state in seconds and is worth watching closely; a settled pull request
only needs to notice new comments; a pull request nobody is looking at needs less again."

### Floors: the tick is a ceiling, not a schedule

If every tick refreshed every stream, the 5-second active cadence would multiply across
metadata, conversation, checks, and the log: five requests every five seconds. Instead each
stream carries its own floor, and the tick merely offers each stream the chance to fire:

- **Checks**: the tick interval itself. Check state is the only thing worth reading as often
  as the tick fires, and its 30-second disk TTL means even an aggressive tick often costs
  nothing.
- **Metadata and conversation**: `PULL_REQUEST_DETAIL_POLL = 20 s` (`src/app.rs:49`). The
  comment above it: the tick cadence is "a ceiling rather than a schedule: check state is the
  only thing worth reading as often as the tick fires. Metadata, the conversation and a
  growing log all change on human or build timescales and hold their own floor."
- **A running log**: `PULL_REQUEST_LOG_POLL = 8 s` (`src/app.rs:52`), described in its comment
  as "a tail interval rather than a staleness bound": the log grows continuously, so this is
  simply how often the tail advances.

### refresh_pull_request_live, walked

The poll body (`src/app.rs:3013`), quoted in full because its ordering is the policy:

```rust
fn refresh_pull_request_live(
    &mut self,
    now: Instant,
    force: bool,
    effects: &mut Vec<AppEffect>,
) {
    self.schedule_pull_request_poll(now);
    let Some(number) = self.pull_request_exact_number else {
        return;
    };
    if self.pull_request.is_none() {
        return;
    }
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
    let settled = self
        .pull_request
        .as_ref()
        .is_some_and(|pull_request| matches!(pull_request.state.as_str(), "MERGED" | "CLOSED"));
    if settled && !force {
        return;
    }
    if due(self.pull_request_detail_read_at, PULL_REQUEST_DETAIL_POLL) {
        let issued = effects.len();
        self.request_pull_request_lookup(number, true, true, effects);
        self.request_pull_request_conversation(true, effects);
        if effects.len() > issued {
            self.pull_request_detail_read_at = Some(now);
        }
    }
    let running = self
        .selected_pull_request_check()
        .is_some_and(|check| check.status.is_running());
    if running && due(self.pull_request_log_read_at, PULL_REQUEST_LOG_POLL) {
        let issued = effects.len();
        self.request_check_run_log(true, effects);
        if effects.len() > issued {
            self.pull_request_log_read_at = Some(now);
        }
    }
}
```

**1. The next poll is armed first.** `schedule_pull_request_poll` runs before anything can
early-return, so the clock never stalls; and it recomputes the interval each time, so the
cadence adapts the moment a run starts or the reader switches views (`switch_view` also
reschedules on its own).

**2. `due` is floor-or-force.** Each stream remembers when it last actually issued a read
(`*_read_at`); a stream fires when its floor has elapsed, or unconditionally under `force`.

**3. Floors are consumed only by issued requests.** The `issued = effects.len()` pattern
around every stream stamps the floor only when an effect was actually emitted. A read that
was suppressed by its own coalescing (a previous load still in flight) stays due and retries
on the next tick, rather than silently losing its slot for a full floor interval. Invariant
11 phrases it: "A stream that coalesces into an in-flight request stays due instead of being
skipped."

**4. A finished run's log is never polled.** The log stream fires only while the selected
check `is_running()`. A settled log is immutable content; the test suite pins the behavior
with the assertion message "a finished run's log never changes, so a poll does not re-read
it."

### The settled gate

The `settled` check is PR #55's addition ("feat: viewport-first file fill, count backfill,
and settled-PR poll stop"). A pull request whose state is `MERGED` or `CLOSED` returns from
the poll immediately after the checks stream, skipping the metadata lookup, the conversation
read, and the log tail entirely. The gate's placement is precise: the checks read sits above
it, so a settled pull request's check list still refreshes on the (now idle or background)
tick, backed by its 30-second TTL; everything below the gate stops. ARCHITECTURE.md invariant
11 records the intent: "A merged or closed pull request is not polled at all; a webhook
delivery or an explicit reload still refreshes it."

The reasoning is that a settled pull request is the common long-lived case. Readers keep
merged PRs open as reference while reviewing follow-ups; before #55, each of those idle views
kept paying the 20-second detail floor forever, for metadata and a conversation that will
essentially never change again, on a PR whose interesting content (patches, logs, counts) is
already fully immutable-cached. After #55 the steady-state cost of an open merged PR is the
occasional cheap checks read and nothing else.

### The webhook bypass

`force` is true in exactly one caller: a webhook delivery. Quinjet can bind an optional
loopback listener (`src/webhook.rs`) intended to pair with `gh webhook forward`; a delivery is
treated purely as a signal that something changed. `App::webhook_delivered`
(`src/app.rs:2953`) calls `refresh_pull_request_live(now, true, ...)`, and `force` short-
circuits every `due` closure and the settled gate: checks, metadata, conversation, and the
selected running log all read at once, and the poll clock restarts.

The bypass and the floors are two halves of one budget argument. The floors are safe to keep
high, and the settled gate is safe to keep absolute, precisely because a push-shaped escape
hatch exists for the moments when something definitely changed; and the webhook is safe to
trust because it carries no data, only timing. The delivery body is drained and discarded;
nothing from it is displayed. The worst a forged loopback request can do is trigger a refresh
that would have happened on the next poll anyway, which is what makes an unauthenticated
listener acceptable (its trust model is documented in `src/webhook_parser.rs`). Deliveries
arriving in a burst are drained to a single boolean per event-loop iteration, so a busy
repository's webhook stream coalesces into one forced refresh rather than a request per
delivery.

### Cost accounting: one minute of watching

Putting the pieces together, one minute of a reader watching a pull request with a running
check, assuming the conversation is unchanged and the metadata TTL is warm:

- **Checks**: the 5-second active tick offers twelve reads; each passes `refresh: true`, so
  the 30-second TTL is bypassed and each read costs one `gh pr checks` request. Twelve
  requests, each a 2 MiB-capped TSV read. This is the deliberate hot spot: check state is
  what the reader is actually watching.
- **Metadata + conversation**: the 20-second floor fires three times. Each lookup is one
  `gh pr view` request; each conversation refresh costs its stamp check first, then at most
  two conditional page-1 reads answering 304 for free when nothing changed. Three paid
  requests, six free ones.
- **Log tail**: the 8-second floor fires about seven times, at two requests each (steps plus
  the growing blob).

Roughly twenty-nine paid requests per minute at the most expensive point in the entire
application, dropping to about four per minute the moment the run finishes (idle tick,
no log tail), to about one every 20 seconds for a settled-but-open PR (checks only, behind
the gate), and to a few per hour from another view. The design never needed to read
`x-ratelimit-remaining` because its worst case was made affordable by construction.

## Repository identity discovery

### From remotes to a canonical identity

Every read on this page addresses a repository by its canonical GitHub identity, the
`owner/name` pair, and every explicit lookup carries that identity rather than trusting `gh`
to infer one from ambient remotes (invariant 7: "Every explicit PR lookup carries its
canonical base-repository identity after lazy remote discovery, avoiding ambient-remote
ambiguity"). Discovering that identity is itself an API-strategy problem, because the naive
approach, asking `gh repo view` about every remote, spends requests on a question Git can
mostly answer offline.

`github_repositories` (`src/git/github/mod.rs:865`) walks the pipeline under a stack of caps:

1. `git remote` lists remote names, capped at `MAX_GIT_REMOTES = 32`; each remote's fetch and
   push URLs come from `git remote get-url [--push] --all`, collected into at most
   `MAX_REMOTE_URL_ENTRIES = 64` pairs, deduplicated through a `BTreeSet`.
2. Each URL is normalized by `remote_url_for_gh` (`src/git/github/mod.rs:1584`): userinfo
   credentials and query or fragment components are stripped from scheme URLs, and scp-style
   `user@host:path` spellings are rewritten to `ssh://host/path`. The point is that secrets
   embedded in a remote URL never appear in a `gh` argv, where they would be visible in the
   process list; the test `strips_credentials_before_passing_remote_urls_to_gh`
   (`src/git/github/mod.rs:2731`) pins it.
3. Sanitized URLs are grouped (at most `MAX_REMOTE_URLS = 32` distinct groups) and each group
   is resolved to an identity one of two ways:
   - **Offline, zero requests**, when the host is exactly `github.com`:
     `repository_from_remote_url` (`src/git/github/mod.rs:1607`) parses `owner/name` straight
     out of an http, https, or ssh URL, strips a `.git` suffix, and canonicalizes ssh to
     https. Public GitHub URL shapes are stable enough to parse; the common case costs
     nothing.
   - **Through `gh`, one cached request, for everything else.** Enterprise hosts
     intentionally return `None` from the offline parser so that `gh repo view [url]`
     validates them against a host Quinjet cannot make assumptions about. The answer is
     cached under `repository\n{identity}` with a 24-hour TTL and the same stale-on-error
     fallback as other metadata.
4. At most `MAX_GITHUB_REPOSITORIES = 16` identities are kept, sorted with any repository that
   has an `origin` remote first.

The startup path is stricter still: `local_github_repository` (`src/git/github/mod.rs:950`)
is the offline-only variant, so launching Quinjet spawns no `gh` process at all. The test
`startup_does_not_fetch_any_pull_request_data` (`src/app.rs`) pins the five commands the app
issues at startup, none of which touch GitHub, and invariant 3 closes the loop: "No GitHub
command is queued at startup or merely by opening the PR tab." The first request the API ever
sees is the one the reader asked for by naming a pull request.

### Head remotes and fork identity

A cross-repository pull request has two repository identities: the base repository the PR
merges into, and the head repository the branch lives in (which may have been deleted). The
metadata record carries the head repository's `nameWithOwner`, and `matching_remotes`
(`src/git/github/mod.rs:1646`) maps it back onto the reader's configured remotes by requiring
both the same host (case-insensitively) and the same `owner/name`. The host requirement is
what keeps an enterprise `github.example.com/acme/widget` from matching a public
`github.com/acme/widget`; the test `matching_head_remotes_do_not_cross_enterprise_hosts`
(`src/git/github/mod.rs:3114`) exists because the two really can share a name. When the fork
is gone, `head_repository` is `None` and the UI labels the head "deleted fork", while the
fetch path falls back to GitHub's synthetic `refs/pull/{number}/head` on the base repository,
which survives fork deletion.

## Evolution across the merged stack

The API strategy was not designed in one sitting; it accreted through the optimization stack
merged on 2026-08-20, and the intermediate states are worth documenting because two of them
were superseded within the same day's stack. The merge order on `main`:

| PR | Commit | Subject | API-strategy relevance |
|---|---|---|---|
| #49 | `56f4154` | perf: read per-file counts from the pulls files endpoint | Counts become an API read; `api_page` generalized |
| #50 | `133e28a` | perf: prefetch smallest files first on huge pull requests | Batch ordering policy (superseded by #55) |
| #52 | `ee0b5b5` | feat: launch the terminal focused on a pull request with --pr | Lookup races startup instead of following it |
| #54 | `b753d26` | feat: pan sidebar lists with the wheel without moving the selection | Viewport state that #55's anchor reads |
| #55 | `1261472` | feat: progressive viewport-first loading for huge PR file views | Viewport-anchored prefetch, settled-poll stop, rename fix |

### What 49 established

Beyond the counts endpoint itself (covered in depth above), #49 made one structural move that
paid off immediately: the paging helper that had lived privately inside the conversation
module was generalized into `Repository::api_page` with the `ApiPage` result struct, and the
conversation's three page-read call sites were rebased onto it. When the counts pipeline
needed bounded pages a few lines later, the primitive already existed. The commit also fixed
its own first draft within the squash: "key API file counts by both endpoints" added both
OIDs to the cache key, because a key of repository and number alone would have kept serving
pre-force-push counts after the head moved. The lesson generalizes: an immutable cache key
must name everything that determines the answer, and for anything derived from PR content
that means the OID pair, not the PR number.

### The smallest-first interlude

PR #50 answered a real problem with an ordering that later proved to be the wrong axis. On a
huge pull request, background prefetch walked the file index in index order and the 6 MiB /
32-file batch budget could be consumed by a handful of giant files at the top of the index
while thousands of small files sat unloaded. #50's fix: once a pull request crossed
`HUGE_PULL_REQUEST_LINES = 100_000` total changed lines or `HUGE_PULL_REQUEST_FILES = 1_000`
files, sort the candidate list ascending by `estimated_patch_bytes` before filling batches,
so the count of files with a ready patch grew as fast as possible. The PR body promised
"most of the tree opens instantly", and by the metric of files-ready-per-request it
delivered: the stable sort packed the maximum number of files into each 6 MiB batch.

The subtle flaw is that files-ready is not what the reader experiences; files-ready *where
the reader is looking* is. Smallest-first fills the tree in size order, which correlates with
nothing about the viewport: the file the reader has scrolled to may be mid-sized and
therefore land late, behind hundreds of tiny files in parts of the tree the reader never
visits. #55 deleted both constants and the sort entirely, and the note in the stack's
engineering history is explicit that ordering by smallest-first exists only in the interval
between commits `133e28a` and `1261472`.

### Viewport-anchored wrap-around

PR #55's replacement anchors the walk to the viewport instead. `prefetch_anchor_index`
(`src/app.rs:5912`), with its doc comment:

```rust
/// Where background fill should start: the first file visible in the
/// Files tree, so patches land where the reader is looking and then wrap
/// around the rest of the index in order.
fn prefetch_anchor_index(&self) -> usize {
    if self.view != View::PullRequests || self.pull_request_section != PullRequestSection::Files
    {
        return 0;
    }
    self.pull_request_tree
        .iter()
        .skip(self.sidebar_offset)
        .find_map(|entry| match entry {
            PullRequestTreeEntry::File { index, .. } => Some(*index),
            PullRequestTreeEntry::Directory { .. } => None,
        })
        .unwrap_or(0)
}
```

The batch builder then rotates the index at the anchor rather than sorting it
(`request_pull_request_prefetch`, `src/app.rs:5930`):

```rust
let anchor = self
    .prefetch_anchor_index()
    .min(self.pull_request_files.len());
let (before, from_anchor) = self.pull_request_files.split_at(anchor);
let mut batch_bytes = 0_usize;
let mut paths: Vec<PathBuf> = Vec::new();
for file in from_anchor.iter().chain(before.iter()) {
    if paths.len() >= limit {
        break;
    }
    if !self.pull_request_file_needs_patch(&file.path)
        || self.pull_request_prefetched_paths.contains(&file.path)
    {
        continue;
    }
    let estimate = estimated_patch_bytes(file.counts);
    if !paths.is_empty()
        && batch_bytes.saturating_add(estimate) > PULL_REQUEST_PREFETCH_BYTE_BUDGET
    {
        break;
    }
    batch_bytes = batch_bytes.saturating_add(estimate);
    paths.push(file.path.clone());
}
```

The current parameters, all in `src/app.rs:33`: batches of at most
`PULL_REQUEST_PREFETCH_BATCH = 32` files, filled until adding a file's estimate would push
the batch past `PULL_REQUEST_PREFETCH_BYTE_BUDGET = 6 MiB`; estimates are
`(additions + deletions) * 80 + 4,096` bytes per file
(`PULL_REQUEST_PATCH_LINE_ESTIMATE = 80` plus the fixed overhead), falling back to
`PULL_REQUEST_PATCH_FALLBACK_ESTIMATE = 512 KiB` when a file has no counts; the walk stops
for good once `MAX_PREFETCHED_PULL_REQUEST_FILES = 4_096` paths have ever been requested for
the workspace. The `!paths.is_empty()` guard on the budget check means a single file
estimated past 6 MiB still travels, alone, so an enormous file cannot deadlock the walk. #55
also raised the total-files cap from 400 to 4,096, changing the walk's character from "warm
the first few hundred" to "fill essentially the whole index of even a huge pull request",
which is what invariant 5 now says: "Background prefetch walks the whole index up to 4,096
files, starting at the file the Files tree is showing and wrapping around the rest in order."

The anchor composes with #54's wheel panning: `sidebar_offset` is exactly the state the
detached wheel scroll moves, so panning the Files tree, without moving the selection or
requesting a single preview, silently redirects where the next background batch lands. The
test `prefetch_starts_at_the_files_viewport_and_wraps_around` (`src/app.rs`) pins the
rotation: with four files and the viewport scrolled past the first two, the batch order is
the third, the fourth, then wrapping to the first and second.

Why rotation beat both alternatives it replaced: against index order, it puts the reader's
viewport first; against smallest-first, it preserves index locality (adjacent files in the
tree arrive together, so the visible region completes coherently instead of pointillistically)
and it needed no size thresholds at all, which deleted two constants and a special case. The
per-batch byte budget already prevents the giant-file starvation #50 was aimed at: a huge
file consumes one batch slot, not the whole budget forever, and the walk continues past it.

### The rest of 55, in API terms

Three more #55 changes belong to this page's story and are covered in their own sections
above: the settled-poll gate ([The adaptive poll](#the-adaptive-poll)), the pure-rename
count fix with its v3 cache key bump
([The pulls files endpoint](#the-pulls-files-endpoint-line-counts-without-blobs)), and count
backfill from arrived patches. The sixth #55 mechanism, `borrow_local_objects`
(`src/git/github/mod.rs:1732`), is a Git-side change with an API-side consequence: writing
the opened repository's objects directory into the disposable workspace's
`objects/info/alternates` file lets lazy blob reads resolve locally, which means the
blob-less fetch strategy that the counts endpoint protects gets cheaper still for merged or
locally built pull requests, where most blobs already exist on disk under other refs. The
fewer lazy blob fetches the workspace performs, the less the network is involved in a diff at
all; `./pr-workspace.md` tells that half of the story.

PR #52 does not change any request, but it changes when the first one happens: `--pr N`
issues the lookup before the first frame renders, and because GitHub work and local reads
occupy different worker lanes, the metadata request runs concurrently with the initial
status and history reads instead of after them. The whole pipeline this page describes,
lookup, counts, compare, workspace, prefetch, begins at process start.

### The invariant text as a changelog

ARCHITECTURE.md's invariants absorbed each step, and diffing their phrasing across the stack
is the shortest summary of the evolution:

- Invariant 9 gained "per-file line counts come from the pull-request files endpoint instead
  of a blob-materializing local numstat" (#49).
- Invariant 5 briefly said "a pull request past 100,000 changed lines or 1,000 files spends
  that budget on its smallest files first" (#50), and now says "starting at the file the
  Files tree is showing and wrapping around the rest in order... and backfills a header's
  counts from its arrived patch when GitHub could not report them" (#55).
- Invariant 11 gained "A merged or closed pull request is not polled at all; a webhook
  delivery or an explicit reload still refreshes it" (#55).

## Design alternatives and why they lost

Each rejected design below was viable; the reasons they lost are specific, and several would
be the right choice in a different application.

### A linked HTTP client instead of gh

Linking an HTTP and TLS stack plus a GitHub client crate would remove process-spawn overhead
and give typed responses. It lost on authentication and surface area. Token acquisition,
storage, refresh, multi-host credentials, and enterprise host mapping are exactly the code a
terminal tool should not maintain; `gh auth login` already owns that state on the machines
Quinjet targets, and reusing it means Quinjet has no credential store at all, which is why
invariant 12a can say flatly "credentials never are" cached. The spawn overhead argument also
inverts under measurement thinking: every `gh` call on this page is on a background worker
lane with floors measured in seconds, so process startup is noise, while the caps mean the
data crossing the pipe is small. The one case where a subprocess per request could genuinely
hurt, high-frequency reads, is precisely the case the strategy eliminates by design.

### GraphQL instead of REST

GitHub's GraphQL API can fetch a pull request, its files, its timeline, and its check state
in one nested query, which sounds like fewer requests. It lost on four counts. Bounded reads:
REST's `per_page` plus `Link` pagination gives the byte-capped page loop a natural unit; a
nested GraphQL response has no equivalent of "kill the child at 2 MiB and keep whole
records". Conditional requests: the ETag-and-304 economy this page leans on is a REST
property. Cache granularity: the immutable-key cache works because each REST endpoint answers
one narrow question whose identity fits in a key; one big query would produce one big entry
invalidated by any change anywhere in it. And the logs endpoint, the largest read in the
application, is REST-only regardless. The strategy of many small, individually cached,
individually validated reads beats one clever query for a client that re-reads constantly.

### serde over JSON instead of jq to TSV

Parsing `gh`'s JSON output with serde would be idiomatic Rust. It lost on response size and
cache mechanics: the jq projection happens inside `gh`, before the capped pipe, so the 2 MiB
budget buys roughly an order of magnitude more records as TSV than as unprojected JSON, and
the cache can store response bytes verbatim and replay them through the same parser as live
output. TSV's failure mode is also better for a bounded reader: a truncated TSV stream
repairs to whole records by trimming to a newline, while a truncated JSON document is
unparsable as a whole. The cost is real, a hand-rolled unescaper and arity checks, but it is
a few dozen lines against a dependency and a second wire format.

### Local numstat instead of the files endpoint

Already covered in full above, but worth restating as the general principle it instantiates,
because it is the purest example on this page: when GitHub has already computed a derived
value (counts, a merge base), reading the value as metadata beats recomputing it from
repository content that would have to be transferred first. The principle appears twice in
invariant 9 and has a name in the techniques catalog: API-derived metadata over local
materialization (`../techniques.md`).

### Reading the rate-limit headers and adapting

An adaptive client could read `x-ratelimit-remaining` and slow itself as the budget drains.
It lost to a simpler contract: make the worst case affordable by construction and the
telemetry becomes unnecessary. Header-driven adaptation also has an unpleasant failure shape
in a shared-token world: another tool drains the budget, and the adaptive client punishes its
own reader with degraded liveness at unpredictable times. Quinjet's fixed floors give it a
predictable, small footprint instead, and the stale-on-error cache disposition handles the
day the budget runs out anyway: every metadata read degrades to its last known answer with a
warning, not to a spinner.

### Webhooks as a data source

The webhook listener could parse delivery payloads and update state directly, saving the
re-read entirely. It lost on trust and on truth. An unauthenticated loopback listener that
displayed payload content would turn any local process into a source of displayed state;
treating the delivery as a signal and re-reading through `gh` keeps the API as the single
source of truth and makes the listener safe by construction (invariant 11: "a delivery is a
signal, never a source of displayed data"). Deliveries also arrive unordered and can be
dropped by the forwarder; a signal degrades gracefully under both, a data source does not.

### Polling harder instead of validating

The floors could simply be lower: re-read the conversation every 5 seconds and skip the ETag
machinery. This is the alternative the numbers reject outright: an unchanged multi-page
conversation re-read at the tick would cost pages-times-twelve requests per minute against
the validated read's zero-cost 304s, while delivering identical bytes. The general rule the
codebase applies is that a request is only worth sending when its answer can differ, and
every mechanism on this page, TTLs sized to real change rates, immutable keys, validators,
stamps, the settled gate, is a different way of proving the answer cannot differ without
sending it.

## Reading on

Within this group, in reading order:

- `./README.md`: the group hub.
- `./pr-workspace.md`: the disposable bare workspace this page's metadata hints feed, and
  the OID-first path that skips the network entirely.
- `./prefetch.md`: the batch scheduler consuming the counts and byte estimates described
  here, and its mailbox-lane isolation.
- `./conversation-and-checks.md`: the consumers of the validated reads, page-order
  machinery, and checks endpoints, from records to rendered thread and log.
- `./caching.md`: the on-disk store behind every cache key on this page: layout, atomic
  writes, private modes, pruning.

Elsewhere in the section:

- `../git-internals/shallow-and-partial-clone.md`: the fetch protocol side of blob-less,
  depth-limited transfer.
- `../git-internals/merge-bases-and-history.md`: the DAG theory behind the compare API's
  answer.
- `../git-internals/object-model.md`: why OID-keyed cache entries can never expire.
- `../rendering/progressive-loading.md`: what the viewport-first fill looks like from the
  renderer's side on a 2,188-file pull request.
- `../rendering/concurrency.md`: the lanes, mailboxes, and generations that carry every
  request on this page.
- `../benchmarking.md`: the reproduction setup behind the huge-PR numbers referenced here.
- `../techniques.md`: the cross-cutting catalog, including adaptive polling, newest-first
  paging, and API-derived metadata as named techniques.

## Optimization review matrix

Use this matrix during performance reviews. Each row combines a cost lens, repository context, and observable signal without claiming that every combination needs a standalone benchmark.

| ID | Review condition | Evidence to capture |
| ---: | --- | --- |
