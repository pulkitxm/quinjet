use super::*;

#[test]
fn audited_source_formats_have_syntax_previews() {
    let syntaxes = two_face::syntax::extra_newlines();
    let paths = [
        "script.applescript",
        "page.astro",
        "script.bash",
        "source.c",
        "schema.capnp",
        "source.cc",
        "module.cjs",
        "settings.cnf",
        "settings.conf",
        "source.cpp",
        "source.cs",
        "project.csproj",
        "style.css",
        "module.cts",
        "exports.def",
        "Dockerfile",
        "Dockerfile.autobahn",
        "App.entitlements",
        "module.ex",
        "module.exs",
        "script.fish",
        "fetch.test.ts.gzip",
        "package.gemspec",
        "main.go",
        "headers.gperf",
        "binding.gyp",
        "header.h",
        "template.hbs",
        "main.tf",
        "page.html",
        "requests.http",
        "frames.http2",
        "interface.idl",
        "settings.ini",
        "Main.java",
        "app.js",
        "data.json",
        "data.json5",
        "data.jsonc",
        "component.jsx",
        "build.gradle.kts",
        "linker.lds",
        "app.manifest",
        "README.markdown",
        "README.md",
        "component.mdx",
        "module.mjs",
        "go.mod",
        "flake.nix",
        "change.patch",
        "project.pbxproj",
        "style.pcss",
        "page.php",
        "script.pl",
        "Info.plist",
        "script.ps1",
        "module.psm1",
        "schema.prisma",
        "gradle.properties",
        "service.proto",
        "main.py",
        "script.rb",
        "resource.rc",
        "settings.reg",
        "main.rs",
        "script.sh",
        "query.sql",
        "Localizable.strings",
        "component.svelte",
        "main.swift",
        "settings.toml",
        "module.ts",
        "component.tsx",
        "component.vue",
        "site.webmanifest",
        "YamlCreate.winget",
        "scheme.xcscheme",
        "workspace.xcworkspacedata",
        "document.xml",
        "workflow.yaml",
        "workflow.yml",
        "script.zsh",
    ];
    let plain = syntaxes.find_syntax_plain_text();
    let missing = paths
        .into_iter()
        .filter(|path| std::ptr::eq(syntax_for_path(&syntaxes, Some(Path::new(path))), plain))
        .collect::<Vec<_>>();
    assert!(missing.is_empty(), "{missing:?}");
}

#[test]
fn audited_plain_text_formats_remain_previewable() {
    let raw = b"diff --git a/file b/file\n--- a/file\n+++ b/file\n@@ -0,0 +1 @@\n+preview value\n";
    for path in [
        "template.in",
        "init.lldb",
        "candidate.patterns",
        "leaksan.supp",
        "page.template",
        "page.tmpl",
        "page.tpl",
    ] {
        let document = parse_diff(raw, path, Some(Path::new(path)), false);
        assert!(
            document
                .lines
                .iter()
                .any(|line| { line.kind == DiffLineKind::Added && line.text() == "preview value" })
        );
    }
}
