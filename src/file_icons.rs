use std::path::Path;

use crate::theme::SyntaxColor;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct FileIcon {
    pub glyph: &'static str,
    pub color: SyntaxColor,
}

mod catalog;

#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use catalog::*;

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
        "flake.nix" => NIXOS,
        "flake.lock" => LOCK,
        "default.nix" => NIXOS,
        "shell.nix" => NIXOS,
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
        "kt" => KOTLIN,
        "kts" => KOTLIN,
        "scala" => SCALA,
        "sc" => SCALA,
        "clj" => CLOJURE,
        "cljs" => CLOJURE,
        "cljc" => CLOJURE,
        "edn" => CLOJURE,
        "c" => C_LANGUAGE,
        "h" => C_LANGUAGE,
        "cc" => CPP,
        "cpp" => CPP,
        "cxx" => CPP,
        "hh" => CPP,
        "hpp" => CPP,
        "hxx" => CPP,
        "m" => CODE,
        "mm" => CODE,
        "cs" => C_SHARP,
        "csx" => C_SHARP,
        "fs" => F_SHARP,
        "fsi" => F_SHARP,
        "fsx" => F_SHARP,
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
        "erl" => ERLANG,
        "hrl" => ERLANG,
        "escript" => ERLANG,
        "hs" => HASKELL,
        "lhs" => HASKELL,
        "lua" => LUA,
        "pl" => PERL,
        "pm" => PERL,
        "t" => PERL,
        "r" => R_PROJECT,
        "rmd" => R_PROJECT,
        "jl" => JULIA,
        "dart" => DART,
        "zig" => ZIG,
        "nim" => NIM,
        "nims" => NIM,
        "v" => CODE,
        "d" => CODE,
        "adb" => CODE,
        "ads" => CODE,
        "f" => FORTRAN,
        "for" => FORTRAN,
        "f77" => FORTRAN,
        "f90" => FORTRAN,
        "f95" => FORTRAN,
        "f03" => FORTRAN,
        "f08" => FORTRAN,
        "cob" => COBOL,
        "cbl" => COBOL,
        "pas" => CODE,
        "pp" => CODE,
        "ml" => OCAML,
        "mli" => OCAML,
        "re" => CODE,
        "rei" => CODE,
        "lisp" => CODE,
        "lsp" => CODE,
        "cl" => CODE,
        "scm" => CODE,
        "ss" => CODE,
        "rkt" => RACKET,
        "pro" => PROLOG,
        "prolog" => PROLOG,
        "sol" => SOLIDITY,
        "vy" => CODE,
        "move" => CODE,
        "cairo" => CODE,
        "graphql" => GRAPHQL,
        "gql" => GRAPHQL,
        "proto" => CODE,
        "thrift" => CODE,
        "wasm" => BINARY,
        "wat" => CODE,
        "sh" => BASH,
        "bash" => BASH,
        "zsh" => SHELL,
        "fish" => SHELL,
        "nu" => SHELL,
        "ps1" => POWERSHELL,
        "psm1" => POWERSHELL,
        "psd1" => POWERSHELL,
        "bat" => SHELL,
        "cmd" => SHELL,
        "sql" => DATABASE,
        "db" => DATABASE,
        "sqlite" => DATABASE,
        "sqlite3" => DATABASE,
        "mdb" => DATABASE,
        "json" => JSON,
        "jsonc" => JSON,
        "json5" => JSON,
        "jsonl" => JSON,
        "ndjson" => JSON,
        "xml" => XML,
        "xsd" => XML,
        "xsl" => XML,
        "xslt" => XML,
        "dtd" => XML,
        "plist" => APPLE,
        "entitlements" => APPLE,
        "xcconfig" => APPLE,
        "pbxproj" => APPLE,
        "xcscheme" => APPLE,
        "xcworkspacedata" => APPLE,
        "xcprivacy" => APPLE,
        "xcsettings" => APPLE,
        "xctestplan" => APPLE,
        "storyboard" => APPLE,
        "xib" => APPLE,
        "nib" => APPLE,
        "storekit" => APPLE,
        "mobileconfig" => APPLE,
        "provisionprofile" => APPLE,
        "swiftdoc" => SWIFT,
        "swiftinterface" => SWIFT,
        "swiftmodule" => SWIFT,
        "swiftoverlay" => SWIFT,
        "swiftcrossimport" => SWIFT,
        "tbd" => APPLE,
        "metallib" => APPLE,
        "mlmodel" => APPLE,
        "mlpackage" => APPLE,
        "yaml" => YAML,
        "yml" => YAML,
        "toml" => CONFIG,
        "ini" => CONFIG,
        "cfg" => CONFIG,
        "conf" => CONFIG,
        "config" => CONFIG,
        "properties" => CONFIG,
        "hcl" => TERRAFORM,
        "tf" => TERRAFORM,
        "tfvars" => TERRAFORM,
        "nix" => NIXOS,
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
        "tex" => LATEX,
        "latex" => LATEX,
        "bib" => LATEX,
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
mod tests;
