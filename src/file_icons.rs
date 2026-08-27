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
    clippy::too_many_lines,
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
        "rust-project.json" => RUST,
        "rust-analyzer.json" => RUST,
        "cargo-generate.toml" => RUST,
        "makefile.toml" => RUST,
        "rustfmt.toml" => RUST,
        ".rustfmt.toml" => RUST,
        "clippy.toml" => RUST,
        "deny.toml" => RUST,
        "cross.toml" => RUST,
        "taplo.toml" => RUST,
        "bacon.toml" => RUST,
        "pyproject.toml" => PYTHON,
        "Pipfile" => PYTHON,
        "Pipfile.lock" => PYTHON,
        "poetry.lock" => PYTHON,
        "uv.lock" => PYTHON,
        "requirements.txt" => PYTHON,
        "requirements-dev.txt" => PYTHON,
        "requirements-test.txt" => PYTHON,
        "constraints.txt" => PYTHON,
        "setup.py" => PYTHON,
        "setup.cfg" => PYTHON,
        "tox.ini" => PYTHON,
        "pytest.ini" => PYTHON,
        "poetry.toml" => PYTHON,
        "pdm.lock" => PYTHON,
        "pdm.toml" => PYTHON,
        "pixi.toml" => PYTHON,
        "conda-lock.yml" => PYTHON,
        "MANIFEST.in" => PYTHON,
        "ruff.toml" => PYTHON,
        ".ruff.toml" => PYTHON,
        "mypy.ini" => PYTHON,
        "pyrightconfig.json" => PYTHON,
        "environment.yml" => PYTHON,
        "alembic.ini" => PYTHON,
        "mkdocs.yml" => PYTHON,
        "mkdocs.yaml" => PYTHON,
        "dvc.yaml" => PYTHON,
        "dvc.lock" => PYTHON,
        "py.typed" => PYTHON,
        "wsgi.py" => PYTHON,
        "asgi.py" => PYTHON,
        "celery.py" => PYTHON,
        ".python-version" => PYTHON,
        ".flake8" => PYTHON,
        ".coveragerc" => PYTHON,
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
        "deno.lock" => TYPESCRIPT,
        "tsconfig.json" => TYPESCRIPT,
        "jsconfig.json" => JAVASCRIPT,
        "turbo.json" => TYPESCRIPT,
        "nx.json" => TYPESCRIPT,
        "biome.json" => JAVASCRIPT,
        "biome.jsonc" => JAVASCRIPT,
        "components.json" => REACT,
        "next-env.d.ts" => TYPESCRIPT,
        "middleware.ts" => TYPESCRIPT,
        "instrumentation.ts" => TYPESCRIPT,
        "proxy.ts" => TYPESCRIPT,
        "route.ts" => TYPESCRIPT,
        "page.tsx" => REACT,
        "layout.tsx" => REACT,
        "loading.tsx" => REACT,
        "error.tsx" => REACT,
        "not-found.tsx" => REACT,
        "template.tsx" => REACT,
        "default.tsx" => REACT,
        "typedoc.json" => TYPESCRIPT,
        "api-extractor.json" => TYPESCRIPT,
        "nodemon.json" => NODE,
        "lerna.json" => NODE,
        "rush.json" => NODE,
        "wrangler.toml" => TYPESCRIPT,
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
        "Package.resolved" => SWIFT,
        "Podfile.lock" => SWIFT,
        "Cartfile" => SWIFT,
        "Cartfile.resolved" => SWIFT,
        "Mintfile" => SWIFT,
        "Project.swift" => SWIFT,
        "Workspace.swift" => SWIFT,
        "Tuist.swift" => SWIFT,
        "Dangerfile" => SWIFT,
        "Fastfile" => SWIFT,
        "Appfile" => SWIFT,
        "Matchfile" => SWIFT,
        "Snapfile" => SWIFT,
        "xcodegen.yml" => SWIFT,
        "xcodegen.yaml" => SWIFT,
        "periphery.yml" => SWIFT,
        ".periphery.yml" => SWIFT,
        "sourcery.yml" => SWIFT,
        ".sourcery.yml" => SWIFT,
        ".swift-format" => SWIFT,
        ".swiftformat" => SWIFT,
        ".swiftlint.yml" => SWIFT,
        ".swiftlint.yaml" => SWIFT,
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

macro_rules! extension_catalog {
    ($($icon:expr => [$($needle:literal),+]),+ $(,)?) => {
        #[expect(
            clippy::string_lit_as_bytes,
            reason = "one flat catalog keeps extension contributions searchable and reviewable"
        )]
        fn extension_icon(extension: &str) -> FileIcon {
            const CATALOG: &[IconMapping] = &sort_catalog([
                $($(IconMapping {
                    hash: ascii_hash($needle.as_bytes()),
                    icon: &$icon,
                },)+)+
            ]);
            lookup_icon(extension, CATALOG).unwrap_or(FILE)
        }

        #[cfg(test)]
        const CURATED_EXTENSIONS: &[&str] = &[$($($needle,)+)+];
    };
}

extension_catalog!(
    RUST => ["rs", "rlib"],
    PYTHON => ["py", "pyw", "pyi", "pyx", "pxd", "pxi", "ipynb"],
    JAVASCRIPT => ["js", "mjs", "cjs", "coffee", "litcoffee", "eszip", "eszip2"],
    REACT => ["jsx", "tsx"],
    TYPESCRIPT => ["ts", "mts", "cts", "tsbuildinfo"],
    HTML => ["html", "htm", "xhtml", "shtml", "astro", "svelte", "ejs", "handlebars", "njk", "pug", "haml", "slim", "mjml"],
    CSS => ["css", "styl"],
    SASS => ["scss", "sass"],
    LESS => ["less"],
    VUE => ["vue"],
    JAVA => ["java", "class", "jar", "war", "ear", "groovy", "gradle"],
    KOTLIN => ["kt", "kts"],
    SCALA => ["scala", "sc"],
    CLOJURE => ["clj", "cljs", "cljc", "edn"],
    C_LANGUAGE => ["c", "h"],
    CPP => ["cc", "cpp", "cxx", "hh", "hpp", "hxx", "cppm", "inl"],
    CODE => ["m", "mm", "v", "d", "adb", "ads", "pas", "pp", "re", "rei", "lisp", "lsp", "cl", "scm", "ss", "vy", "move", "cairo", "proto", "thrift", "wat", "hbs", "mustache", "jinja", "jinja2", "twig", "liquid", "ada", "apinotes", "asm", "cr", "cu", "cuh", "di", "el", "elm", "fbs", "capnp", "frag", "gd", "gdscript", "godot", "gdextension", "glsl", "hlsl", "wgsl", "vert", "hip", "j2", "djtpl", "ll", "td", "sil", "mir", "qml", "s", "sed", "tcl", "sv", "svh", "vh", "sycl", "vala", "vapi", "vb", "vbs", "vim", "wit", "in", "template", "tmpl", "tpl", "def", "inc", "include", "ld", "lds", "map", "rsp", "sln", "vcxproj", "vbproj", "natvis", "gltf", "obj", "dae", "stl", "kml", "gpx", "sample", "example", "snap", "snapshot", "golden", "expected", "mermaid", "mmd", "puml", "plantuml", "dot", "gv", "drawio"],
    C_SHARP => ["cs", "csx", "cshtml", "csproj"],
    F_SHARP => ["fs", "fsi", "fsx", "fsproj"],
    GO => ["go"],
    SWIFT => ["swift", "swiftdoc", "swiftinterface", "swiftmodule", "swiftoverlay", "swiftcrossimport", "gyb"],
    PHP => ["php", "phtml", "blade"],
    RUBY => ["rb", "erb", "rake", "gemspec"],
    ELIXIR => ["ex", "exs", "eex", "heex", "leex"],
    ERLANG => ["erl", "hrl", "escript"],
    HASKELL => ["hs", "lhs"],
    LUA => ["lua"],
    PERL => ["pl", "pm", "t", "p6", "raku"],
    R_PROJECT => ["r", "rmd"],
    JULIA => ["jl"],
    DART => ["dart"],
    ZIG => ["zig"],
    NIM => ["nim", "nims", "nimble"],
    FORTRAN => ["f", "for", "f77", "f90", "f95", "f03", "f08"],
    COBOL => ["cob", "cbl"],
    OCAML => ["ml", "mli"],
    RACKET => ["rkt"],
    PROLOG => ["pro", "prolog"],
    SOLIDITY => ["sol"],
    GRAPHQL => ["graphql", "gql"],
    BINARY => ["wasm", "exe", "dll", "so", "dylib", "a", "lib", "elf", "bin", "appimage", "bc", "pkl", "pickle", "joblib", "npy", "npz", "pt", "pth", "onnx", "tflite", "safetensors", "glb", "fbx", "blend", "usdz", "mo", "shp", "shx", "pcap", "pcapng", "graffle"],
    BASH => ["sh", "bash"],
    SHELL => ["zsh", "fish", "nu", "bat", "cmd", "awk", "command", "ksh"],
    POWERSHELL => ["ps1", "psm1", "psd1"],
    DATABASE => ["sql", "db", "sqlite", "sqlite3", "mdb", "prisma", "edgeql", "psql", "dbf", "mmdb", "realm"],
    JSON => ["json", "jsonc", "json5", "jsonl", "ndjson", "webmanifest", "geojson", "topojson"],
    XML => ["xml", "xsd", "xsl", "xslt", "dtd"],
    APPLE => ["plist", "entitlements", "xcconfig", "pbxproj", "xcscheme", "xcworkspacedata", "xcprivacy", "xcsettings", "xctestplan", "storyboard", "xib", "nib", "storekit", "mobileconfig", "provisionprofile", "tbd", "metallib", "mlmodel", "mlpackage", "applescript", "scpt", "metal", "modulemap", "strings", "stringsdict"],
    YAML => ["yaml", "yml"],
    CONFIG => ["toml", "ini", "cfg", "conf", "config", "properties", "bazel", "bzl", "bzlmod", "cmake", "cue", "dhall", "ron", "rego", "makefile", "babelrc", "browserslistrc", "coveragerc", "editorconfig", "eslintrc", "flake8", "flowconfig", "graphqlconfig", "prettierrc", "swcrc", "cnf", "code-workspace", "code-snippets", "tmlanguage", "desktop", "service", "socket", "timer"],
    TERRAFORM => ["hcl", "tf", "tfvars"],
    NIXOS => ["nix"],
    LOCK => ["lock", "resolved", "sum"],
    MARKDOWN => ["md", "markdown", "mdown", "mkdn", "mdx", "rst", "adoc", "asciidoc", "org", "mdc", "pod", "roff", "man", "1", "2", "3", "4", "5", "6", "7", "8", "9"],
    TEXT => ["txt", "po", "pot", "arb", "xlf", "xliff", "xmb", "xtb", "ics", "eml", "cff", "text", "log", "stdout", "stderr"],
    WORD => ["rtf", "doc", "docx", "odt"],
    LATEX => ["tex", "latex", "bib", "cls", "sty"],
    SPREADSHEET => ["csv", "tsv", "xls", "xlsx", "ods", "parquet", "orc", "avro", "arrow", "feather", "h5", "hdf5", "mat"],
    PDF => ["pdf"],
    PRESENTATION => ["ppt", "pptx", "odp"],
    IMAGE => ["svg", "png", "jpg", "jpeg", "gif", "webp", "avif", "bmp", "ico", "tif", "tiff", "psd", "ai", "sketch", "apng", "heic", "icns", "exr", "hdr", "tga", "dds", "cur", "jp2", "jxl", "ktx", "ktx2", "pbm", "pgm", "ppm", "pnm", "xpm", "xcf", "qoi", "pic", "dng", "cr2", "nef"],
    AUDIO => ["mp3", "wav", "flac", "ogg", "m4a", "aac", "aiff", "opus", "pcm", "mid", "midi", "m3u", "m3u8", "pls"],
    VIDEO => ["mp4", "mkv", "mov", "avi", "webm", "m4v", "vtt", "srt", "ass"],
    ARCHIVE => ["zip", "tar", "gz", "bz2", "xz", "7z", "rar", "zst", "tgz", "tbz2", "txz", "asar", "br", "lzma", "nupkg", "whl", "egg", "ipa", "pkg", "dmg", "kmz", "bak", "old"],
    FONT => ["ttf", "otf", "woff", "woff2", "eot"],
    KEY => ["pem", "key", "pub", "keystore", "age", "asc", "gpg", "ppk", "sig"],
    CERTIFICATE => ["crt", "cer", "p12", "pfx", "cert", "csr", "der", "spdx"],
    PACKAGE => ["deb", "rpm", "apk", "msi", "wixproj", "wxs", "wxi", "wxl", "iss", "nsi", "nsh"],
    DOCKER => ["dockerfile"],
    NPM => ["npmrc"],
    NODE => ["nvmrc", "node"],
    YARN => ["yarnrc"],
    GIT => ["orig", "patch", "diff"],
);

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
