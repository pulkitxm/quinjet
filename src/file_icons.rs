use std::path::Path;

use crate::theme::SyntaxColor;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct FileIcon {
    pub glyph: &'static str,
    pub color: SyntaxColor,
}

const FILE: FileIcon = FileIcon {
    glyph: "\u{f15b}",
    color: SyntaxColor::Text,
};
const CODE: FileIcon = FileIcon {
    glyph: "\u{f1c9}",
    color: SyntaxColor::Blue,
};
const RUST: FileIcon = FileIcon {
    glyph: "\u{e7a8}",
    color: SyntaxColor::Orange,
};
const PYTHON: FileIcon = FileIcon {
    glyph: "\u{ed1b}",
    color: SyntaxColor::Yellow,
};
const JAVASCRIPT: FileIcon = FileIcon {
    glyph: "\u{f2ee}",
    color: SyntaxColor::Yellow,
};
const TYPESCRIPT: FileIcon = FileIcon {
    glyph: "\u{f2ee}",
    color: SyntaxColor::Blue,
};
const HTML: FileIcon = FileIcon {
    glyph: "\u{f13b}",
    color: SyntaxColor::Orange,
};
const CSS: FileIcon = FileIcon {
    glyph: "\u{f13c}",
    color: SyntaxColor::Blue,
};
const SASS: FileIcon = FileIcon {
    glyph: "\u{ed49}",
    color: SyntaxColor::Purple,
};
const LESS: FileIcon = FileIcon {
    glyph: "\u{ed48}",
    color: SyntaxColor::Blue,
};
const REACT: FileIcon = FileIcon {
    glyph: "\u{ed46}",
    color: SyntaxColor::Cyan,
};
const VUE: FileIcon = FileIcon {
    glyph: "\u{ed4a}",
    color: SyntaxColor::Green,
};
const ANGULAR: FileIcon = FileIcon {
    glyph: "\u{ed4b}",
    color: SyntaxColor::Red,
};
const NODE: FileIcon = FileIcon {
    glyph: "\u{ed0d}",
    color: SyntaxColor::Green,
};
const NPM: FileIcon = FileIcon {
    glyph: "\u{ed0e}",
    color: SyntaxColor::Red,
};
const YARN: FileIcon = FileIcon {
    glyph: "\u{ef75}",
    color: SyntaxColor::Blue,
};
const JAVA: FileIcon = FileIcon {
    glyph: "\u{edaf}",
    color: SyntaxColor::Red,
};
const PHP: FileIcon = FileIcon {
    glyph: "\u{ed6d}",
    color: SyntaxColor::Purple,
};
const SWIFT: FileIcon = FileIcon {
    glyph: "\u{efbe}",
    color: SyntaxColor::Orange,
};
const GO: FileIcon = FileIcon {
    glyph: "\u{e724}",
    color: SyntaxColor::Cyan,
};
const HASKELL: FileIcon = FileIcon {
    glyph: "\u{e777}",
    color: SyntaxColor::Purple,
};
const DART: FileIcon = FileIcon {
    glyph: "\u{e798}",
    color: SyntaxColor::Cyan,
};
const R_PROJECT: FileIcon = FileIcon {
    glyph: "\u{edc1}",
    color: SyntaxColor::Blue,
};
const RUBY: FileIcon = FileIcon {
    glyph: "\u{e739}",
    color: SyntaxColor::Red,
};
const LUA: FileIcon = FileIcon {
    glyph: "\u{e826}",
    color: SyntaxColor::Blue,
};
const ELIXIR: FileIcon = FileIcon {
    glyph: "\u{e7cd}",
    color: SyntaxColor::Purple,
};
const SHELL: FileIcon = FileIcon {
    glyph: "\u{f120}",
    color: SyntaxColor::Green,
};
const MARKDOWN: FileIcon = FileIcon {
    glyph: "\u{eeab}",
    color: SyntaxColor::Blue,
};
const GIT: FileIcon = FileIcon {
    glyph: "\u{efa0}",
    color: SyntaxColor::Orange,
};
const DOCKER: FileIcon = FileIcon {
    glyph: "\u{f21f}",
    color: SyntaxColor::Blue,
};
const DATABASE: FileIcon = FileIcon {
    glyph: "\u{f1c0}",
    color: SyntaxColor::Cyan,
};
const CONFIG: FileIcon = FileIcon {
    glyph: "\u{f013}",
    color: SyntaxColor::Yellow,
};
const PACKAGE: FileIcon = FileIcon {
    glyph: "\u{f1b2}",
    color: SyntaxColor::Brown,
};
const PACKAGES: FileIcon = FileIcon {
    glyph: "\u{f1b3}",
    color: SyntaxColor::Purple,
};
const LOCK: FileIcon = FileIcon {
    glyph: "\u{f023}",
    color: SyntaxColor::Yellow,
};
const KEY: FileIcon = FileIcon {
    glyph: "\u{f084}",
    color: SyntaxColor::Yellow,
};
const IMAGE: FileIcon = FileIcon {
    glyph: "\u{f1c5}",
    color: SyntaxColor::Purple,
};
const AUDIO: FileIcon = FileIcon {
    glyph: "\u{f1c7}",
    color: SyntaxColor::Cyan,
};
const VIDEO: FileIcon = FileIcon {
    glyph: "\u{f1c8}",
    color: SyntaxColor::Red,
};
const ARCHIVE: FileIcon = FileIcon {
    glyph: "\u{f1c6}",
    color: SyntaxColor::Brown,
};
const PDF: FileIcon = FileIcon {
    glyph: "\u{f1c1}",
    color: SyntaxColor::Red,
};
const WORD: FileIcon = FileIcon {
    glyph: "\u{f1c2}",
    color: SyntaxColor::Blue,
};
const SPREADSHEET: FileIcon = FileIcon {
    glyph: "\u{f1c3}",
    color: SyntaxColor::Green,
};
const PRESENTATION: FileIcon = FileIcon {
    glyph: "\u{f1c4}",
    color: SyntaxColor::Orange,
};
const FONT: FileIcon = FileIcon {
    glyph: "\u{f031}",
    color: SyntaxColor::Purple,
};
const CERTIFICATE: FileIcon = FileIcon {
    glyph: "\u{f0a3}",
    color: SyntaxColor::Yellow,
};
const BINARY: FileIcon = FileIcon {
    glyph: "\u{f2db}",
    color: SyntaxColor::Brown,
};

#[derive(Debug, Clone, Copy)]
struct IconMapping {
    hash: u64,
    icon: &'static FileIcon,
}

macro_rules! hashed_icon {
    ($value:expr, $default:expr, $($needle:literal => $icon:expr),+ $(,)?) => {{
        const CATALOG: &[IconMapping] = &sort_catalog([
            $(IconMapping {
                hash: ascii_hash($needle.as_bytes()),
                icon: &$icon,
            },)+
        ]);
        let value = $value;
        lookup_icon(value, CATALOG).unwrap_or($default)
    }};
}

pub(crate) fn for_path(path: &Path) -> FileIcon {
    let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
        return FILE;
    };
    let special = special_name_icon(name);
    if special != FILE {
        return special;
    }
    path.extension()
        .and_then(|extension| extension.to_str())
        .map_or(FILE, extension_icon)
}

#[expect(
    clippy::string_lit_as_bytes,
    reason = "the catalog macro keeps each visible name and its compile-time hash in one entry"
)]
fn special_name_icon(name: &str) -> FileIcon {
    if environment_name(name) {
        return KEY;
    }
    hashed_icon!(
        name,
        FILE,
        "Cargo.toml" => RUST,
        "Cargo.lock" => RUST,
        "rust-toolchain" => RUST,
        "rust-toolchain.toml" => RUST,
        "pyproject.toml" => PYTHON,
        "Pipfile" => PYTHON,
        "Pipfile.lock" => PYTHON,
        "poetry.lock" => PYTHON,
        "uv.lock" => PYTHON,
        "requirements.txt" => PYTHON,
        "setup.py" => PYTHON,
        "package.json" => NPM,
        "package-lock.json" => NPM,
        "npm-shrinkwrap.json" => NPM,
        ".npmrc" => NPM,
        "yarn.lock" => YARN,
        ".yarnrc" => YARN,
        ".yarnrc.yml" => YARN,
        "pnpm-lock.yaml" => NODE,
        "pnpm-workspace.yaml" => NODE,
        "bun.lock" => JAVASCRIPT,
        "bun.lockb" => JAVASCRIPT,
        "deno.json" => TYPESCRIPT,
        "deno.jsonc" => TYPESCRIPT,
        "tsconfig.json" => TYPESCRIPT,
        "jsconfig.json" => JAVASCRIPT,
        "angular.json" => ANGULAR,
        "Dockerfile" => DOCKER,
        "Containerfile" => DOCKER,
        ".dockerignore" => DOCKER,
        "docker-compose.yml" => DOCKER,
        "docker-compose.yaml" => DOCKER,
        "compose.yml" => DOCKER,
        "compose.yaml" => DOCKER,
        ".gitignore" => GIT,
        ".gitattributes" => GIT,
        ".gitmodules" => GIT,
        ".gitconfig" => GIT,
        ".mailmap" => GIT,
        "go.mod" => GO,
        "go.sum" => GO,
        "go.work" => GO,
        "Gemfile" => RUBY,
        "Gemfile.lock" => RUBY,
        "Rakefile" => RUBY,
        "Podfile" => SWIFT,
        "Package.swift" => SWIFT,
        "mix.exs" => ELIXIR,
        "mix.lock" => ELIXIR,
        "stack.yaml" => HASKELL,
        "cabal.project" => HASKELL,
        "pubspec.yaml" => DART,
        "pubspec.lock" => DART,
        "pom.xml" => JAVA,
        "build.gradle" => JAVA,
        "build.gradle.kts" => JAVA,
        "settings.gradle" => JAVA,
        "settings.gradle.kts" => JAVA,
        "gradle.properties" => JAVA,
        "Makefile" => CONFIG,
        "makefile" => CONFIG,
        "GNUmakefile" => CONFIG,
        "CMakeLists.txt" => CONFIG,
        "Justfile" => CONFIG,
        "Taskfile.yml" => CONFIG,
        "Taskfile.yaml" => CONFIG,
        "Vagrantfile" => CONFIG,
        "Procfile" => CONFIG,
        "flake.nix" => PACKAGE,
        "flake.lock" => LOCK,
        "default.nix" => PACKAGE,
        "shell.nix" => PACKAGE,
        "terraform.lock.hcl" => LOCK,
        ".editorconfig" => CONFIG,
        ".prettierrc" => CONFIG,
        ".eslintrc" => CONFIG,
        ".stylelintrc" => CONFIG,
        "README" => MARKDOWN,
        "CHANGELOG" => MARKDOWN,
        "CONTRIBUTING" => MARKDOWN,
        "LICENSE" => CERTIFICATE,
        "LICENCE" => CERTIFICATE,
        "COPYING" => CERTIFICATE
    )
}

#[expect(
    clippy::string_lit_as_bytes,
    clippy::too_many_lines,
    reason = "one flat catalog keeps extension contributions searchable and reviewable"
)]
fn extension_icon(extension: &str) -> FileIcon {
    hashed_icon!(
        extension,
        FILE,
        "rs" => RUST,
        "rlib" => RUST,
        "py" => PYTHON,
        "pyw" => PYTHON,
        "pyi" => PYTHON,
        "pyx" => PYTHON,
        "pxd" => PYTHON,
        "pxi" => PYTHON,
        "ipynb" => PYTHON,
        "js" => JAVASCRIPT,
        "mjs" => JAVASCRIPT,
        "cjs" => JAVASCRIPT,
        "jsx" => REACT,
        "ts" => TYPESCRIPT,
        "mts" => TYPESCRIPT,
        "cts" => TYPESCRIPT,
        "tsx" => REACT,
        "html" => HTML,
        "htm" => HTML,
        "xhtml" => HTML,
        "shtml" => HTML,
        "astro" => HTML,
        "css" => CSS,
        "scss" => SASS,
        "sass" => SASS,
        "less" => LESS,
        "styl" => CSS,
        "vue" => VUE,
        "java" => JAVA,
        "class" => JAVA,
        "jar" => JAVA,
        "war" => JAVA,
        "ear" => JAVA,
        "kt" => CODE,
        "kts" => CODE,
        "scala" => CODE,
        "sc" => CODE,
        "clj" => CODE,
        "cljs" => CODE,
        "cljc" => CODE,
        "edn" => CODE,
        "c" => CODE,
        "h" => CODE,
        "cc" => CODE,
        "cpp" => CODE,
        "cxx" => CODE,
        "hh" => CODE,
        "hpp" => CODE,
        "hxx" => CODE,
        "m" => CODE,
        "mm" => CODE,
        "cs" => CODE,
        "csx" => CODE,
        "fs" => CODE,
        "fsi" => CODE,
        "fsx" => CODE,
        "go" => GO,
        "swift" => SWIFT,
        "php" => PHP,
        "phtml" => PHP,
        "blade" => PHP,
        "rb" => RUBY,
        "erb" => RUBY,
        "rake" => RUBY,
        "gemspec" => RUBY,
        "ex" => ELIXIR,
        "exs" => ELIXIR,
        "eex" => ELIXIR,
        "heex" => ELIXIR,
        "leex" => ELIXIR,
        "erl" => CODE,
        "hrl" => CODE,
        "escript" => CODE,
        "hs" => HASKELL,
        "lhs" => HASKELL,
        "lua" => LUA,
        "pl" => CODE,
        "pm" => CODE,
        "t" => CODE,
        "r" => R_PROJECT,
        "rmd" => R_PROJECT,
        "jl" => CODE,
        "dart" => DART,
        "zig" => CODE,
        "nim" => CODE,
        "nims" => CODE,
        "v" => CODE,
        "d" => CODE,
        "adb" => CODE,
        "ads" => CODE,
        "f" => CODE,
        "for" => CODE,
        "f77" => CODE,
        "f90" => CODE,
        "f95" => CODE,
        "f03" => CODE,
        "f08" => CODE,
        "cob" => CODE,
        "cbl" => CODE,
        "pas" => CODE,
        "pp" => CODE,
        "ml" => CODE,
        "mli" => CODE,
        "re" => CODE,
        "rei" => CODE,
        "lisp" => CODE,
        "lsp" => CODE,
        "cl" => CODE,
        "scm" => CODE,
        "ss" => CODE,
        "rkt" => CODE,
        "pro" => CODE,
        "prolog" => CODE,
        "sol" => CODE,
        "vy" => CODE,
        "move" => CODE,
        "cairo" => CODE,
        "graphql" => CODE,
        "gql" => CODE,
        "proto" => CODE,
        "thrift" => CODE,
        "wasm" => BINARY,
        "wat" => CODE,
        "sh" => SHELL,
        "bash" => SHELL,
        "zsh" => SHELL,
        "fish" => SHELL,
        "nu" => SHELL,
        "ps1" => SHELL,
        "psm1" => SHELL,
        "psd1" => SHELL,
        "bat" => SHELL,
        "cmd" => SHELL,
        "sql" => DATABASE,
        "db" => DATABASE,
        "sqlite" => DATABASE,
        "sqlite3" => DATABASE,
        "mdb" => DATABASE,
        "json" => CODE,
        "jsonc" => CODE,
        "json5" => CODE,
        "jsonl" => CODE,
        "ndjson" => CODE,
        "xml" => CODE,
        "xsd" => CODE,
        "xsl" => CODE,
        "xslt" => CODE,
        "dtd" => CODE,
        "yaml" => CONFIG,
        "yml" => CONFIG,
        "toml" => CONFIG,
        "ini" => CONFIG,
        "cfg" => CONFIG,
        "conf" => CONFIG,
        "config" => CONFIG,
        "properties" => CONFIG,
        "hcl" => PACKAGES,
        "tf" => PACKAGES,
        "tfvars" => PACKAGES,
        "nix" => PACKAGE,
        "lock" => LOCK,
        "md" => MARKDOWN,
        "markdown" => MARKDOWN,
        "mdown" => MARKDOWN,
        "mkdn" => MARKDOWN,
        "mdx" => MARKDOWN,
        "rst" => MARKDOWN,
        "adoc" => MARKDOWN,
        "asciidoc" => MARKDOWN,
        "org" => MARKDOWN,
        "txt" => FILE,
        "rtf" => WORD,
        "tex" => FILE,
        "latex" => FILE,
        "bib" => FILE,
        "csv" => SPREADSHEET,
        "tsv" => SPREADSHEET,
        "pdf" => PDF,
        "doc" => WORD,
        "docx" => WORD,
        "odt" => WORD,
        "xls" => SPREADSHEET,
        "xlsx" => SPREADSHEET,
        "ods" => SPREADSHEET,
        "ppt" => PRESENTATION,
        "pptx" => PRESENTATION,
        "odp" => PRESENTATION,
        "svg" => IMAGE,
        "png" => IMAGE,
        "jpg" => IMAGE,
        "jpeg" => IMAGE,
        "gif" => IMAGE,
        "webp" => IMAGE,
        "avif" => IMAGE,
        "bmp" => IMAGE,
        "ico" => IMAGE,
        "tif" => IMAGE,
        "tiff" => IMAGE,
        "psd" => IMAGE,
        "ai" => IMAGE,
        "sketch" => IMAGE,
        "mp3" => AUDIO,
        "wav" => AUDIO,
        "flac" => AUDIO,
        "ogg" => AUDIO,
        "m4a" => AUDIO,
        "aac" => AUDIO,
        "mp4" => VIDEO,
        "mkv" => VIDEO,
        "mov" => VIDEO,
        "avi" => VIDEO,
        "webm" => VIDEO,
        "zip" => ARCHIVE,
        "tar" => ARCHIVE,
        "gz" => ARCHIVE,
        "bz2" => ARCHIVE,
        "xz" => ARCHIVE,
        "7z" => ARCHIVE,
        "rar" => ARCHIVE,
        "zst" => ARCHIVE,
        "tgz" => ARCHIVE,
        "tbz2" => ARCHIVE,
        "txz" => ARCHIVE,
        "ttf" => FONT,
        "otf" => FONT,
        "woff" => FONT,
        "woff2" => FONT,
        "eot" => FONT,
        "pem" => KEY,
        "key" => KEY,
        "pub" => KEY,
        "crt" => CERTIFICATE,
        "cer" => CERTIFICATE,
        "p12" => CERTIFICATE,
        "pfx" => CERTIFICATE,
        "exe" => BINARY,
        "dll" => BINARY,
        "so" => BINARY,
        "dylib" => BINARY,
        "a" => BINARY,
        "lib" => BINARY,
        "elf" => BINARY,
        "bin" => BINARY,
        "appimage" => BINARY,
        "deb" => PACKAGE,
        "rpm" => PACKAGE,
        "apk" => PACKAGE,
        "msi" => PACKAGE,
        "hbs" => CODE,
        "mustache" => CODE,
        "jinja" => CODE,
        "jinja2" => CODE,
        "twig" => CODE,
        "liquid" => CODE,
        "patch" => GIT,
        "diff" => GIT
    )
}

fn environment_name(name: &str) -> bool {
    let Some(prefix) = name.get(..4) else {
        return false;
    };
    prefix.eq_ignore_ascii_case(".env")
        && name
            .as_bytes()
            .get(4)
            .is_none_or(|separator| *separator == b'.')
}

#[expect(
    clippy::indexing_slicing,
    reason = "the loop condition proves the byte index is in bounds in this const function"
)]
const fn ascii_hash(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    let mut index = 0;
    while index < bytes.len() {
        hash ^= bytes[index].to_ascii_lowercase() as u64;
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        index += 1;
    }
    hash
}

#[expect(
    clippy::indexing_slicing,
    reason = "the insertion-sort loop proves both catalog indices are in bounds"
)]
const fn sort_catalog<const N: usize>(mut catalog: [IconMapping; N]) -> [IconMapping; N] {
    let mut index = 1;
    while index < N {
        let mut cursor = index;
        while cursor > 0 && catalog[cursor - 1].hash > catalog[cursor].hash {
            let previous = catalog[cursor - 1];
            catalog[cursor - 1] = catalog[cursor];
            catalog[cursor] = previous;
            cursor -= 1;
        }
        index += 1;
    }
    catalog
}

#[expect(
    clippy::integer_division,
    reason = "binary search intentionally rounds the midpoint down"
)]
fn lookup_icon(value: &str, catalog: &[IconMapping]) -> Option<FileIcon> {
    let hash = ascii_hash(value.as_bytes());
    let mut low = 0;
    let mut high = catalog.len();
    while low < high {
        let middle = low + (high - low) / 2;
        let mapping = catalog.get(middle)?;
        if mapping.hash < hash {
            low = middle + 1;
        } else {
            high = middle;
        }
    }
    catalog
        .get(low)
        .filter(|mapping| mapping.hash == hash)
        .map(|mapping| *mapping.icon)
}

#[cfg(test)]
mod tests {
    use super::*;
    use unicode_width::UnicodeWidthStr;

    #[test]
    fn recognizes_exact_ecosystem_files_before_their_extensions() {
        assert_eq!(for_path(Path::new("Cargo.toml")), RUST);
        assert_eq!(for_path(Path::new("pyproject.toml")), PYTHON);
        assert_eq!(for_path(Path::new("package.json")), NPM);
        assert_eq!(for_path(Path::new("Dockerfile")), DOCKER);
        assert_eq!(for_path(Path::new("go.mod")), GO);
    }

    #[test]
    fn recognizes_extensions_without_allocating_or_requiring_lowercase() {
        assert_eq!(for_path(Path::new("src/main.RS")), RUST);
        assert_eq!(for_path(Path::new("types/index.D.TS")), TYPESCRIPT);
        assert_eq!(for_path(Path::new("assets/logo.SVG")), IMAGE);
        assert_eq!(for_path(Path::new("backup.TAR.GZ")), ARCHIVE);
    }

    #[test]
    fn recognizes_environment_variants_and_falls_back_for_unknown_files() {
        assert_eq!(for_path(Path::new(".env.local")), KEY);
        assert_eq!(for_path(Path::new(".ENV")), KEY);
        assert_eq!(for_path(Path::new("source.unknown")), FILE);
        assert_eq!(for_path(Path::new("LICENSE")), CERTIFICATE);
    }

    #[test]
    fn every_icon_occupies_one_terminal_cell() {
        let icons = [
            FILE,
            CODE,
            RUST,
            PYTHON,
            JAVASCRIPT,
            TYPESCRIPT,
            HTML,
            CSS,
            SASS,
            LESS,
            REACT,
            VUE,
            ANGULAR,
            NODE,
            NPM,
            YARN,
            JAVA,
            PHP,
            SWIFT,
            GO,
            HASKELL,
            DART,
            R_PROJECT,
            RUBY,
            LUA,
            ELIXIR,
            SHELL,
            MARKDOWN,
            GIT,
            DOCKER,
            DATABASE,
            CONFIG,
            PACKAGE,
            PACKAGES,
            LOCK,
            KEY,
            IMAGE,
            AUDIO,
            VIDEO,
            ARCHIVE,
            PDF,
            WORD,
            SPREADSHEET,
            PRESENTATION,
            FONT,
            CERTIFICATE,
            BINARY,
        ];
        assert!(icons.iter().all(|icon| icon.glyph.width() == 1));
    }
}
