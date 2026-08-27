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
