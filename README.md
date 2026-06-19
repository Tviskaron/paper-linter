<p align="center">
  <img src="web/public/assets/readme-logo.svg" alt="paper-linter logo" width="420">
</p>

# paper-linter

<a href="https://tviskaron.github.io/paper-linter/"><img src="https://img.shields.io/badge/site-paper--linter-72d5c8?labelColor=475569" alt="site"></a>
<a href="https://github.com/Tviskaron/paper-linter/actions/workflows/ci.yml"><img src="https://img.shields.io/github/actions/workflow/status/Tviskaron/paper-linter/ci.yml?branch=main&amp;label=ci&amp;labelColor=475569&amp;color=72d5c8" alt="CI"></a>

PaperLinter is a static browser app for linting LaTeX paper sources. It runs the
Rust lint engine in WebAssembly, entirely in the browser: archives and folders
are processed locally and are not uploaded to a server.

## Highlights

- Static WASM app with no backend.
- Supports `.zip`, `.tar`, `.tar.gz`, `.tgz`, and local directory inputs.
- Follows common LaTeX project structure, including `\input`, `\include`, and
  `\subfile`.
- Checks citations, labels, figures, tables, package risks, section structure,
  typography, formatting, and source hygiene.
- Lets users select bundled profiles or individual rule families in the UI.

## CLI Installation

Requirements:

- Rust and Cargo. Install them from <https://rustup.rs/> if `cargo --version`
  does not work.

```console
cargo install --git https://github.com/Tviskaron/paper-linter.git --force
```

Cargo installs binaries into `~/.cargo/bin`. If `paper-linter` is not found,
add that directory to your shell path.

For zsh:

```console
echo 'export PATH="$HOME/.cargo/bin:$PATH"' >> ~/.zshrc
source ~/.zshrc
```

For bash:

```console
echo 'export PATH="$HOME/.cargo/bin:$PATH"' >> ~/.bashrc
source ~/.bashrc
```

For fish:

```console
fish_add_path "$HOME/.cargo/bin"
```

Run a check:

```console
paper-linter check paper.tex
paper-linter check . --all-tex
paper-linter rules
```

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
