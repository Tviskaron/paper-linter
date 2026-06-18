# PaperLinter

[![CI](https://github.com/Tviskaron/paper-linter/actions/workflows/ci.yml/badge.svg)](https://github.com/Tviskaron/paper-linter/actions/workflows/ci.yml)

PaperLinter is a static browser app for linting LaTeX paper sources. It runs the
Rust lint engine in WebAssembly, entirely in the browser: archives and folders
are processed locally and are not uploaded to a server.

The public deployment target is GitHub Pages:

- App: `https://tviskaron.github.io/paper-linter/`
- Source: `https://github.com/Tviskaron/paper-linter`

## Highlights

- Static WASM app with no backend.
- Supports `.zip`, `.tar`, `.tar.gz`, `.tgz`, and local directory inputs.
- Follows common LaTeX project structure, including `\input`, `\include`, and
  `\subfile`.
- Checks citations, labels, figures, tables, package risks, section structure,
  typography, formatting, and source hygiene.
- Lets users select bundled profiles or individual rule families in the UI.

## Local Development

```console
npm ci
npm run dev
```

The development command builds the Rust crate with `wasm-pack`, writes generated
bindings to `web/pkg`, then starts Vite for the `web/` app.

## Build

```console
npm run build
```

The production build writes static files to `web/dist`. The Vite configuration
uses a relative asset base so the generated JavaScript, CSS, logo, and WASM file
work under GitHub Pages project paths.

## Rust Checks

```console
cargo fmt --all --check
cargo check --lib --target wasm32-unknown-unknown --no-default-features --features web --locked
cargo test --lib --no-default-features --features web --locked
cargo clippy --lib --no-default-features --features web --locked -- -D warnings
```

## WebAssembly API

The browser app imports the generated `PaperLinter` binding from `web/pkg`:

- `new PaperLinter()`
- `add_file(path, bytes)`
- `check(options_json)`
- `rules_json()`

`check` accepts JSON options with `preset`, `select`, `ignore`, `strict`,
`all_rules`, and `all_tex`. It returns JSON diagnostics and summary data for the
uploaded virtual project.

## Deployment

GitHub Actions deploys on pushes to `main`:

1. Install Node dependencies with `npm ci`.
2. Install Rust with the `wasm32-unknown-unknown` target.
3. Run `npm run build`.
4. Upload `web/dist` to GitHub Pages.

Generated directories such as `target/`, `web/pkg/`, and `web/dist/` are not
tracked.
