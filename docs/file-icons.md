# File icons

Quinjet displays a themed icon beside filenames in Changes, pull request file
trees, diff headers, pull request details, and conflict resolution. The icon
registry lives in `src/file_icons.rs` and is part of Quinjet itself, with no
runtime package, asset directory, or network request.

The registry uses real Font Awesome Brands and Devicons logo outlines through
the Nerd Fonts code point namespace. It does not use emoji. Font Awesome is
preferred whenever it provides the language or tool brand. Devicons fills
common gaps such as Rust, JavaScript, TypeScript, Go, Haskell, Dart, Ruby, Lua,
and Elixir. Generic Font Awesome symbols are reserved for formats that do not
have a brand, such as archives and certificates.

Install and select a current [Nerd Font](https://www.nerdfonts.com/) in the
terminal to render every icon. An unpatched font may show replacement
characters, but filenames and every interaction remain available.

SVGs and font files are deliberately not embedded. Ratatui and crossterm render
terminal cells, and terminal cells cannot portably display an inline SVG.
Embedding those assets would increase the binary while still requiring a
terminal-specific image protocol. Nerd Font glyphs retain the vector brand
outlines while using the terminal's native text renderer. Each lookup hashes the
filename or extension once, searches a compile-time sorted table without
allocating, and returns a static glyph plus a semantic syntax color from the
active theme.

## Coverage

Exact ecosystem filenames take precedence over extensions. The catalog
recognizes files such as `Cargo.toml`, `pyproject.toml`, `package.json`,
`Dockerfile`, `go.mod`, `Gemfile`, `Package.swift`, `mix.exs`, `pom.xml`, and
common lock, build, Git, and environment files.

Extension coverage includes:

- Major compiled, interpreted, functional, shell, web, and smart contract
  languages
- Framework formats such as React, Vue, Angular, Sass, Less, and templates
- JSON, YAML, TOML, XML, SQL, GraphQL, Protocol Buffers, and Terraform
- Markdown, office documents, images, audio, video, archives, fonts,
  certificates, packages, databases, and binaries

Unknown paths receive the generic Font Awesome file icon.

## Contributing an icon

Add an exact filename to `special_name_icon` or an extension to
`extension_icon` in `src/file_icons.rs`. Reuse an existing `FileIcon` constant
when its glyph and color fit. For a new glyph, add one constant near the other
icon definitions and use a code point present in the current Nerd Fonts
[`glyphnames.json`](https://github.com/ryanoasis/nerd-fonts/blob/master/glyphnames.json).
Prefer the `fa-` entry when Font Awesome has the icon, then use `dev-` only for
a missing language brand.

Add the path to the table in lowercase or conventional casing. Lookup is ASCII
case-insensitive. Add a focused assertion to the module tests for the new path.

Font Awesome Free icons are available under the project's documented free
licenses. Nerd Fonts relocates the glyphs into non-conflicting terminal code
point ranges and documents the included sets and their licenses.
