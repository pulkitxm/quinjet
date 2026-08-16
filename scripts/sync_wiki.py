#!/usr/bin/env python3
"""Turn `docs/` into the pages of this repository's GitHub wiki.

The wiki is generated, never edited: every page here is overwritten from
`docs/` on each push to main. One Markdown file becomes one wiki page, a
directory becomes a group whose `README.md` is the group's own page, and
relative links between files are rewritten to the wiki slugs they became.

    python3 scripts/sync_wiki.py                 write .wiki-build/
    python3 scripts/sync_wiki.py --out DIR       write somewhere else
    python3 scripts/sync_wiki.py --check         fail if a link points nowhere
    python3 scripts/sync_wiki.py --push          clone the wiki and push

Requires nothing but the standard library and, for --push, `git`.
"""

from __future__ import annotations

import argparse
import os
import re
import shutil
import subprocess
import sys
from dataclasses import dataclass, field
from pathlib import Path, PurePosixPath

REPO_ROOT = Path(__file__).resolve().parent.parent


@dataclass(frozen=True)
class Section:
    """One directory of `docs/` and the wiki slugs it produces."""

    directory: str
    prefix: str
    label: str
    files_only: bool = False


SECTIONS = (
    Section("docs/cli", "CLI", "CLI reference"),
    Section("docs/guides", "Guides", "Guides", files_only=True),
    Section("docs/practices", "Practices", "Rust practices"),
)

READING_ORDER = (
    "getting-started",
    "conventions",
    "tui",
    "repository",
    "changes",
    "remotes",
    "branch",
    "stash",
    "pull-request",
    "generated",
    "studies",
    "patterns",
    "gap-analysis",
)

SMALL_WORDS = frozenset({"a", "an", "the", "of", "to", "and", "in", "on", "for", "with", "vs"})
ACRONYMS = frozenset({"cli", "tui", "ci", "cd", "pr", "json", "api", "url", "http", "https", "id"})

LINK = re.compile(r"\]\(([^)]+)\)")
FENCE = re.compile(r"^\s*(?:```|~~~)")
HEADING = re.compile(r"^#{1,6}\s+(.*?)\s*$")
COMMANDS = re.compile(r"^##\s+Commands\s*$(.*?)(?=^##\s|\Z)", re.MULTILINE | re.DOTALL)
CHILD_LINK = re.compile(r"\]\(\./([A-Za-z0-9._-]+)\.md\)")
EXTERNAL = re.compile(r"^(?:https?:|mailto:|#)", re.IGNORECASE)
ORIGIN = re.compile(r"github\.com[:/]+([^/]+)/(.+?)(?:\.git)?$")
MARKUP = re.compile(r"[`*_]")
NOT_ANCHOR = re.compile(r"[^\w\s-]")
FALLBACK_REPOSITORY = ("pulkitxm", "quinjet")


@dataclass
class Page:
    """One Markdown file and the wiki page it becomes."""

    source: str
    slug: str
    title: str
    section: str
    order: int
    depth: int = 0
    parent: str | None = None
    child_order: int = 0
    is_index: bool = False
    is_group: bool = False


@dataclass
class Wiki:
    """Every page, plus what rewriting the links between them needs."""

    pages: list[Page] = field(default_factory=list)
    slugs: dict[str, str] = field(default_factory=dict)
    directories: dict[str, str] = field(default_factory=dict)
    root: Path = REPO_ROOT
    blob_base: str = ""
    broken: list[str] = field(default_factory=list)


def capitalize(part: str, index: int) -> str:
    """Title-case one hyphen-separated part, leaving acronyms whole."""
    if part.lower() in ACRONYMS:
        return part.upper()
    if index and part in SMALL_WORDS:
        return part
    return part[:1].upper() + part[1:]


def slug_suffix(name: str) -> str:
    """Turn a file or directory name into the tail of a wiki slug."""
    return "-".join(capitalize(part, index) for index, part in enumerate(name.split("-")))


def display_title(name: str) -> str:
    """Turn a file or directory name into a page title."""
    return slug_suffix(name).replace("-", " ")


def reading_order(name: str) -> int:
    """Rank a top-level page or group; anything unlisted sorts last."""
    return READING_ORDER.index(name) if name in READING_ORDER else len(READING_ORDER)


def is_readme(name: str) -> bool:
    """Say whether a file name is the group index."""
    return name.lower() == "readme.md"


def posix(path: Path | PurePosixPath | str) -> str:
    """Render a path the way a Markdown link writes it."""
    return str(path).replace(os.sep, "/")


def child_order(group: Path, names: list[str]) -> list[str]:
    """Order a group's pages the way its own `## Commands` list does.

    Only that section counts. A page mentioned earlier in the prose is being
    referred to rather than ordered, and letting a mention hoist a page put one
    group's verbs on the sidebar in the order they happened to be discussed.
    """
    readme = group / "README.md"
    if not readme.exists():
        return names
    text = readme.read_text("utf-8")
    listing = COMMANDS.search(text)
    cited: list[str] = []
    for match in CHILD_LINK.finditer(listing.group(1) if listing else text):
        if match.group(1) not in cited:
            cited.append(match.group(1))
    known = [name for name in cited if name in names]
    return known + [name for name in names if name not in known]


def file_page(entry: Path, section: Section) -> Page:
    """Build the page for a Markdown file sitting directly in a section."""
    source = f"{section.directory}/{entry.name}"
    if is_readme(entry.name):
        return Page(
            source=source,
            slug=section.prefix,
            title="Overview",
            section=section.label,
            order=-1,
            is_index=True,
        )
    return Page(
        source=source,
        slug=f"{section.prefix}-{slug_suffix(entry.stem)}",
        title=display_title(entry.stem),
        section=section.label,
        order=reading_order(entry.stem),
    )


def group_pages(entry: Path, section: Section) -> list[Page]:
    """Build the index page and the leaf pages of one group directory."""
    group_slug = f"{section.prefix}-{slug_suffix(entry.name)}"
    files = sorted(child.name for child in entry.iterdir() if child.suffix == ".md")
    pages: list[Page] = []
    if any(is_readme(name) for name in files):
        pages.append(
            Page(
                source=f"{section.directory}/{entry.name}/README.md",
                slug=group_slug,
                title=display_title(entry.name),
                section=section.label,
                order=reading_order(entry.name),
                is_group=True,
            )
        )
    leaves = [name.removesuffix(".md") for name in files if not is_readme(name)]
    pages.extend(
        Page(
            source=f"{section.directory}/{entry.name}/{leaf}.md",
            slug=f"{group_slug}-{slug_suffix(leaf)}",
            title=display_title(leaf),
            section=section.label,
            order=reading_order(entry.name),
            child_order=index,
            depth=1,
            parent=group_slug,
        )
        for index, leaf in enumerate(child_order(entry, leaves))
    )
    return pages


def section_pages(section: Section, root: Path) -> list[Page]:
    """Describe every page one section of `docs/` produces."""
    directory = root / section.directory
    if not directory.is_dir():
        return []
    pages: list[Page] = []
    for entry in sorted(directory.iterdir(), key=lambda entry: entry.name):
        if entry.is_file() and entry.suffix == ".md":
            pages.append(file_page(entry, section))
        elif entry.is_dir() and not section.files_only:
            pages.extend(group_pages(entry, section))
    return pages


def collect(root: Path = REPO_ROOT) -> Wiki:
    """Walk `docs/` and describe every page it will produce."""
    wiki = Wiki(root=root)
    for section in SECTIONS:
        wiki.pages.extend(section_pages(section, root))
    wiki.slugs = {page.source: page.slug for page in wiki.pages}
    wiki.directories = {section.directory: section.prefix for section in SECTIONS}
    for page in wiki.pages:
        if page.is_group:
            wiki.directories[posix(PurePosixPath(page.source).parent)] = page.slug
    return wiki


def owner_repo(root: Path = REPO_ROOT) -> tuple[str, str]:
    """Read the GitHub owner and repository name from the origin remote."""
    try:
        url = run_git(["remote", "get-url", "origin"], root).stdout.strip()
    except (subprocess.CalledProcessError, FileNotFoundError):
        return FALLBACK_REPOSITORY
    match = ORIGIN.search(url)
    return (match.group(1), match.group(2)) if match else FALLBACK_REPOSITORY


def heading_slugs(path: Path) -> set[str]:
    """Collect the anchors GitHub's wiki will generate for a page."""
    slugs: set[str] = set()
    inside_fence = False
    for line in path.read_text("utf-8").split("\n"):
        if FENCE.match(line):
            inside_fence = not inside_fence
            continue
        heading = HEADING.match(line)
        if inside_fence or not heading:
            continue
        text = MARKUP.sub("", heading.group(1)).lower()
        slugs.add(NOT_ANCHOR.sub("", text).strip().replace(" ", "-"))
    return slugs


def map_target(target: str, page_dir: str, wiki: Wiki) -> tuple[str | None, str, str]:
    """Rewrite one link target, and report the path and anchor it resolved to."""
    parts = target.split()
    url = parts[0]
    trailing = f" {' '.join(parts[1:])}" if len(parts) > 1 else ""
    if not url or EXTERNAL.match(url):
        return None, "", ""
    anchor = ""
    if "#" in url:
        url, _, fragment = url.partition("#")
        anchor = f"#{fragment}"
    if not url:
        return None, "", ""
    resolved = posix(PurePosixPath(os.path.normpath(f"{page_dir}/{url}"))).rstrip("/")
    if resolved in wiki.directories:
        return f"{wiki.directories[resolved]}{anchor}{trailing}", resolved, anchor
    if resolved in wiki.slugs:
        return f"{wiki.slugs[resolved]}{anchor}{trailing}", resolved, anchor
    if (wiki.root / resolved).exists():
        return f"{wiki.blob_base}/{resolved}{anchor}{trailing}", resolved, anchor
    return None, resolved, anchor


def rewrite_line(line: str, source: str, number: int, wiki: Wiki) -> str:
    """Rewrite every link on one line, recording the ones that go nowhere."""
    page_dir = posix(PurePosixPath(source).parent)

    def replace(match: re.Match[str]) -> str:
        target = match.group(1)
        mapped, resolved, anchor = map_target(target, page_dir, wiki)
        relative = target.split()[0] if target.split() else ""
        if mapped is None:
            if relative and not EXTERNAL.match(relative):
                wiki.broken.append(f"{source}:{number}: {relative} resolves to no file")
            return match.group(0)
        if anchor and resolved in wiki.slugs:
            fragment = anchor.removeprefix("#")
            if fragment not in heading_slugs(wiki.root / resolved):
                wiki.broken.append(f"{source}:{number}: {relative} has no such heading")
        return f"]({mapped})"

    return LINK.sub(replace, line)


def rewrite_links(content: str, source: str, wiki: Wiki) -> str:
    """Rewrite a page's relative links to the wiki slugs they became."""
    inside_fence = False
    lines = []
    for number, line in enumerate(content.split("\n"), start=1):
        if FENCE.match(line):
            inside_fence = not inside_fence
            lines.append(line)
        elif inside_fence:
            lines.append(line)
        else:
            lines.append(rewrite_line(line, source, number, wiki))
    return "\n".join(lines)


def section_index(wiki: Wiki, label: str) -> Page | None:
    """Find a section's overview page, which is not part of its tree."""
    return next((page for page in wiki.pages if page.section == label and page.is_index), None)


def section_tree(wiki: Wiki, label: str) -> list[tuple[Page, list[Page]]]:
    """Order a section's top-level pages, each with its own children."""
    in_section = [page for page in wiki.pages if page.section == label]
    tops = sorted(
        (page for page in in_section if not page.is_index and page.depth == 0),
        key=lambda page: (page.order, page.title),
    )
    return [
        (
            top,
            sorted(
                (page for page in in_section if page.parent == top.slug),
                key=lambda page: page.child_order,
            ),
        )
        for top in tops
    ]


def end_with_newline(text: str) -> str:
    """Trim trailing blank lines and finish with exactly one newline."""
    return f"{text.rstrip()}\n"


def build_home(wiki: Wiki) -> str:
    """Build the wiki's landing page."""
    lines = [
        (
            "Documentation for **Quinjet**: the command line reference and the "
            "longer guides, generated from the `docs/` directory of the main "
            "repository. Edit the docs in the repo, these pages are overwritten "
            "on every push to `main`."
        ),
        "",
        (
            "`quinjet` with no verb opens the terminal interface. Every operation "
            "that interface performs is also a verb, and `quinjet --help` lists "
            "them all."
        ),
        "",
    ]
    for section in SECTIONS:
        tree = section_tree(wiki, section.label)
        if not tree:
            continue
        lines.extend([f"## {section.label}", ""])
        index = section_index(wiki, section.label)
        if index:
            lines.append(f"- [{index.title}]({index.slug})")
        for page, children in tree:
            verbs = ", ".join(f"[{child.title}]({child.slug})" for child in children)
            lines.append(f"- [{page.title}]({page.slug})" + (f": {verbs}" if verbs else ""))
        lines.append("")
    return end_with_newline("\n".join(lines))


def sidebar_group(page: Page, children: list[Page]) -> list[str]:
    """Render one collapsible group of the sidebar."""
    if not children:
        return [f"- [{page.title}]({page.slug})"]
    return [
        "",
        "<details>",
        f'<summary><a href="{page.slug}">{page.title}</a></summary>',
        "",
        *[f"- [{child.title}]({child.slug})" for child in children],
        "",
        "</details>",
        "",
    ]


def build_sidebar(wiki: Wiki) -> str:
    """Build the sidebar every wiki page shows."""
    lines = ["- [Home](Home)", ""]
    for section in SECTIONS:
        tree = section_tree(wiki, section.label)
        if not tree:
            continue
        lines.extend([f"**{section.label}**", ""])
        index = section_index(wiki, section.label)
        if index:
            lines.append(f"- [{index.title}]({index.slug})")
        for page, children in tree:
            lines.extend(sidebar_group(page, children))
        lines.append("")
    return end_with_newline("\n".join(lines))


def build_footer(tree_base: str) -> str:
    """Build the footer every wiki page shows."""
    return end_with_newline(
        f"_Generated from [`docs/`]({tree_base}/docs), edit the docs in the repo, not the wiki._"
    )


def build_pages(root: Path = REPO_ROOT) -> tuple[dict[str, str], list[str]]:
    """Render every wiki page, and report the links that go nowhere."""
    owner, repo = owner_repo(root)
    wiki = collect(root)
    wiki.blob_base = f"https://github.com/{owner}/{repo}/blob/main"
    pages: dict[str, str] = {}
    for page in wiki.pages:
        raw = (root / page.source).read_text("utf-8")
        pages[f"{page.slug}.md"] = end_with_newline(rewrite_links(raw, page.source, wiki))
    if pages:
        pages["Home.md"] = build_home(wiki)
        pages["_Sidebar.md"] = build_sidebar(wiki)
        pages["_Footer.md"] = build_footer(f"https://github.com/{owner}/{repo}/tree/main")
    return pages, wiki.broken


def write_pages(directory: Path, pages: dict[str, str]) -> None:
    """Replace every Markdown file in a directory with the rendered pages."""
    directory.mkdir(parents=True, exist_ok=True)
    for existing in directory.iterdir():
        if existing.name != ".git" and existing.suffix == ".md":
            existing.unlink()
    for name, content in pages.items():
        (directory / name).write_text(content, "utf-8")


def run_git(arguments: list[str], cwd: Path) -> subprocess.CompletedProcess[str]:
    """Run one Git command in a directory and insist that it succeeds."""
    return subprocess.run(
        ["git", *arguments],
        cwd=cwd,
        capture_output=True,
        text=True,
        check=True,
    )


def clone_wiki(url: str, directory: Path) -> None:
    """Clone the wiki, or start a fresh repository when it has none yet."""
    shutil.rmtree(directory, ignore_errors=True)
    cloned = subprocess.run(
        ["git", "clone", "--depth", "1", url, str(directory)],
        capture_output=True,
        text=True,
        check=False,
    )
    if cloned.returncode == 0:
        return
    sys.stdout.write("Wiki not initialized yet; starting one.\n")
    directory.mkdir(parents=True, exist_ok=True)
    run_git(["init", "-b", "master"], directory)
    run_git(["remote", "add", "origin", url], directory)


def push_pages(pages: dict[str, str], root: Path = REPO_ROOT) -> None:
    """Write the rendered pages into the wiki repository and push them."""
    owner, repo = owner_repo(root)
    token = os.environ.get("GITHUB_TOKEN") or os.environ.get("GH_TOKEN") or ""
    auth = f"x-access-token:{token}@" if token else ""
    url = f"https://{auth}github.com/{owner}/{repo}.wiki.git"
    directory = root / ".wiki-clone"
    clone_wiki(url, directory)
    run_git(["config", "user.name", os.environ.get("GIT_AUTHOR_NAME", "wiki-sync")], directory)
    run_git(
        [
            "config",
            "user.email",
            os.environ.get("GIT_AUTHOR_EMAIL", "wiki-sync@users.noreply.github.com"),
        ],
        directory,
    )
    write_pages(directory, pages)
    run_git(["add", "-A"], directory)
    if not run_git(["status", "--porcelain"], directory).stdout.strip():
        sys.stdout.write("Wiki already up to date.\n")
        return
    run_git(["commit", "-m", "docs: sync wiki from docs/"], directory)
    run_git(["push", "origin", "HEAD"], directory)
    sys.stdout.write(f"Pushed {len(pages)} pages to {owner}/{repo}.wiki.git\n")


def main() -> int:
    """Build the wiki, then check it, push it, or write it to a directory."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--out", default=str(REPO_ROOT / ".wiki-build"))
    parser.add_argument("--check", action="store_true")
    parser.add_argument("--push", action="store_true")
    arguments = parser.parse_args()

    pages, broken = build_pages()
    if not pages:
        sys.stderr.write("No documentation found under docs/\n")
        return 1
    if arguments.check:
        for entry in broken:
            sys.stderr.write(f"broken link {entry}\n")
        if broken:
            return 1
        sys.stdout.write(f"{len(pages)} pages build cleanly.\n")
        return 0
    if arguments.push:
        push_pages(pages)
        return 0
    out = Path(arguments.out).resolve()
    write_pages(out, pages)
    sys.stdout.write(f"Wrote {len(pages)} pages to {out}\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
