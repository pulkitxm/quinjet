use unicode_width::UnicodeWidthStr;

use super::*;

#[test]
fn recognizes_exact_ecosystem_files_before_their_extensions() {
    assert_eq!(for_path(Path::new("Cargo.toml")), RUST);
    assert_eq!(for_path(Path::new("pyproject.toml")), PYTHON);
    assert_eq!(for_path(Path::new("package.json")), NPM);
    assert_eq!(for_path(Path::new("Dockerfile")), DOCKER);
    assert_eq!(for_path(Path::new("go.mod")), GO);
}

#[test]
fn recognizes_major_ecosystem_conventions() {
    for (path, icon) in [
        ("app/page.tsx", REACT),
        ("next-env.d.ts", TYPESCRIPT),
        ("biome.json", JAVASCRIPT),
        ("pyrightconfig.json", PYTHON),
        ("requirements-dev.txt", PYTHON),
        ("Package.resolved", SWIFT),
        (".swiftlint.yml", SWIFT),
        ("rust-analyzer.json", RUST),
        ("deny.toml", RUST),
    ] {
        assert_eq!(for_path(Path::new(path)), icon, "{path}");
    }
}

#[test]
fn recognizes_extensions_without_allocating_or_requiring_lowercase() {
    assert_eq!(for_path(Path::new("src/main.RS")), RUST);
    assert_eq!(for_path(Path::new("types/index.D.TS")), TYPESCRIPT);
    assert_eq!(for_path(Path::new("assets/logo.SVG")), IMAGE);
    assert_eq!(for_path(Path::new("backup.TAR.GZ")), ARCHIVE);
}

#[test]
fn uses_language_brand_glyphs_instead_of_generic_symbols() {
    assert_eq!(for_path(Path::new("main.py")).glyph, "\u{ed1b}");
    assert_eq!(for_path(Path::new("main.js")).glyph, "\u{e781}");
    assert_eq!(for_path(Path::new("main.ts")).glyph, "\u{e8ca}");
    assert_eq!(for_path(Path::new("main.rs")).glyph, "\u{e7a8}");
    assert_eq!(for_path(Path::new("main.go")).glyph, "\u{e724}");
}

#[test]
fn recognizes_apple_project_files() {
    for path in [
        "Resources/Info.plist",
        "Edith.entitlements",
        "project.pbxproj",
        "Main.storyboard",
        "PrivacyInfo.xcprivacy",
    ] {
        assert_eq!(for_path(Path::new(path)), APPLE, "{path}");
    }
    assert_eq!(for_path(Path::new("Edith.swiftmodule")), SWIFT);
}

#[test]
fn recognizes_environment_variants_and_falls_back_for_unmapped_files() {
    assert_eq!(for_path(Path::new(".env.local")), KEY);
    assert_eq!(for_path(Path::new(".ENV")), KEY);
    assert_eq!(for_path(Path::new("source.unmapped-extension")), FILE);
    assert_eq!(for_path(Path::new("LICENSE")), CERTIFICATE);
}

#[test]
fn recognizes_audited_repository_conventions() {
    for (path, icon) in [
        (".validation", CONFIG),
        (".npmignore", NPM),
        (".gitkeep", GIT),
        (".prettierignore", CONFIG),
        ("CODEOWNERS", GIT),
        (".eslintignore", CONFIG),
        ("_redirects", CONFIG),
        ("CNAME", CONFIG),
        (".clang-format", CONFIG),
        (".clangd", CONFIG),
        (".pylintrc", PYTHON),
        (".nvmrc", NODE),
        (".yarn-integrity", YARN),
        ("gradlew", JAVA),
        ("NOTICE", CERTIFICATE),
        ("Dockerfile.mysql-plain", DOCKER),
        ("binding.gyp", CONFIG),
        ("interface.idl", CODE),
        ("Info.plist", APPLE),
    ] {
        assert_eq!(for_path(Path::new(path)), icon, "{path}");
    }
}

const AUDITED_EXTENSIONS: &[&str] = &[
    "acl",
    "applescript",
    "asc",
    "astro",
    "attr",
    "autobahn",
    "avif",
    "bash",
    "bat",
    "bin",
    "bin-linking",
    "br",
    "c",
    "capnp",
    "cc",
    "cer",
    "cjs",
    "cnf",
    "conf",
    "cpp",
    "crt",
    "cs",
    "csproj",
    "csr",
    "css",
    "csv",
    "cts",
    "custom",
    "def",
    "default",
    "der",
    "dist",
    "dockerfile",
    "docx",
    "dyn",
    "entitlements",
    "env",
    "eot",
    "ex",
    "example",
    "exponent",
    "exs",
    "ext",
    "file",
    "fish",
    "gemspec",
    "gif",
    "go",
    "gperf",
    "gyp",
    "gz",
    "gzip",
    "h",
    "hbs",
    "hcl",
    "html",
    "http",
    "http2",
    "icns",
    "ico",
    "idl",
    "in",
    "ini",
    "invalid-rules-of-hooks-f6f37b63b2d4",
    "ipynb",
    "jar",
    "java",
    "jfif",
    "jpeg",
    "jpg",
    "js",
    "json",
    "json5",
    "jsonc",
    "jsx",
    "key",
    "kts",
    "lds",
    "lldb",
    "local",
    "lock",
    "lockb",
    "log",
    "m4a",
    "manifest",
    "map",
    "mariadb-plain",
    "markdown",
    "md",
    "mdc",
    "mdx",
    "mjs",
    "mod",
    "modulus",
    "mov",
    "mp3",
    "mp4",
    "mts",
    "mysql-native-password",
    "mysql-plain",
    "nix",
    "node",
    "odt",
    "old",
    "otf",
    "patch",
    "patterns",
    "pbxproj",
    "pcss",
    "pdf",
    "pem",
    "pfx",
    "php",
    "pl",
    "plist",
    "png",
    "postgres-auth",
    "postgres-plain",
    "prisma",
    "properties",
    "proto",
    "ps1",
    "psd1",
    "psm1",
    "py",
    "pyc",
    "rb",
    "rc",
    "reg",
    "reg2",
    "resolved",
    "rs",
    "rtf",
    "samples",
    "sh",
    "sha1",
    "sha256",
    "sln",
    "snap",
    "snapshot",
    "spkac",
    "splinecode",
    "sql",
    "squid",
    "srl",
    "status",
    "strings",
    "sum",
    "supp",
    "svelte",
    "svg",
    "swift",
    "tar",
    "template",
    "test",
    "testdb",
    "tex",
    "tgz",
    "tmpl",
    "toml",
    "tpl",
    "ts",
    "tsx",
    "ttf",
    "txt",
    "unknown",
    "v2",
    "v2-most-features",
    "vue",
    "wasm",
    "webm",
    "webmanifest",
    "webp",
    "weird",
    "winget",
    "woff",
    "woff2",
    "world",
    "xcscheme",
    "xcworkspacedata",
    "xlsx",
    "xml",
    "yaml",
    "yml",
    "z",
    "zsh",
    "zst",
];

#[test]
fn covers_every_extension_observed_in_contributed_repositories() {
    for extension in AUDITED_EXTENSIONS {
        assert_ne!(extension_icon(extension), FILE, "{extension}");
    }
}

#[test]
fn every_curated_extension_has_a_non_generic_icon() {
    let mut extensions = CURATED_EXTENSIONS.to_vec();
    extensions.sort_unstable();
    extensions.dedup();
    assert_eq!(extensions.len(), CURATED_EXTENSIONS.len());
    for extension in CURATED_EXTENSIONS {
        let path = format!("file.{extension}");
        assert_ne!(for_path(Path::new(&path)), FILE, "{extension}");
    }
}

#[test]
fn every_icon_occupies_one_terminal_cell() {
    let icons = [
        FILE,
        TEXT,
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
        APPLE,
        GO,
        HASKELL,
        DART,
        R_PROJECT,
        RUBY,
        LUA,
        ELIXIR,
        C_LANGUAGE,
        CPP,
        C_SHARP,
        F_SHARP,
        KOTLIN,
        SCALA,
        CLOJURE,
        ERLANG,
        PERL,
        JULIA,
        ZIG,
        NIM,
        FORTRAN,
        COBOL,
        OCAML,
        PROLOG,
        RACKET,
        SOLIDITY,
        GRAPHQL,
        NIXOS,
        TERRAFORM,
        JSON,
        YAML,
        XML,
        POWERSHELL,
        LATEX,
        BASH,
        SHELL,
        MARKDOWN,
        GIT,
        DOCKER,
        DATABASE,
        CONFIG,
        PACKAGE,
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
