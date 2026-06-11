# paper-linter

An extremely fast linter for LaTeX papers.

`paper-linter` is designed to be 10-100x faster than existing tools while staying simple to install, easy to run, and useful in editors, CI, and pre-commit hooks.

## Highlights

- **Written in Rust**: parallel, memory-safe, no runtime.
- **Zero configuration**: sensible defaults, with opt-in strict mode.
- **Single binary, no deps**: drop into CI or pre-commit in seconds.

## What It Checks

### References & Citations

- Every `\cite{}` key exists in `.bib` or a fallback `.bbl`.
- Every `.bib` entry is cited.
- Required fields: author, year, venue.
- Duplicate bibliography keys and very similar titles with different keys.
- Roadmap: DOI/URL validation and consistent `\citet` / `\citep` style.

### Figures & Tables

- Every image is `\ref`'d in text.
- Every `\label` has a reference.
- Captions are present and assets exist.
- Roadmap: broken `\ref` targets, placement proximity to mention, resolution and format checks.

### Structure & Formatting

- Sane section hierarchy.
- Acronyms defined on first use.
- Placeholder checks: TODO, TBD, Lorem.
- Roadmap: section-title capitalization, non-breaking space before `\cite`, math-mode consistency.

### Style & Writing

- Repeated words and filler.
- Very long sentences.
- Trailing whitespace and comment percentage.
- Roadmap: passive-voice heuristic, per-venue style presets, LLM-assisted suggestions.

## Example

```console
paper-linter check paper.tex
paper.tex:42:18: warning[WS001] trailing whitespace
checked 1 file(s), 0 error(s), 1 warning(s)
```

Current implementation status: the core CLI and rule pipeline are in place, with
`CIT001`-`CIT006` citation checks and the first Structure & Formatting rules
implemented:

- `CIT001`-`CIT006`: citation and bibliography consistency checks, with `.bbl`
  fallback for arXiv sources that omit `.bib` files.
- `FMT001`: missing final newline.
- `FMT002`: repeated blank lines.
- `ENV001`: environment begin/end mismatch.
- `SEC001`: skipped section hierarchy level.
- `SEC002`: empty section.
- `TEX001`: missing non-breaking space before references or citations.
- `TXT001`: placeholder text.
- `TXT002`: repeated word.
- `WS001`: trailing whitespace.

The remaining checks listed above describe the intended MVP and v1.0 roadmap.

## Installation

Commands below are shown without shell prompt markers so they can be copied
directly.

### Requirements

- Rust and Cargo. Install them from <https://rustup.rs/> if `cargo --version`
  does not work.
- SSH access to GitHub if installing from the private Git URL.

### Repository install

```console
cargo install --git ssh://git@github.com/Tviskaron/paper-linter.git --force
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

Check the install:

```console
paper-linter --help
paper-linter check paper.tex
```

### Repository installer script

The installer runs `cargo install`, checks where the binary was installed, and
helps add Cargo's bin directory to your shell `PATH`.

```console
git clone git@github.com:Tviskaron/paper-linter.git
cd paper-linter
./install.sh --yes
```

### Public repo one-liner

If the repository is public, the installer can also be run directly from GitHub:

```console
curl -fsSL https://raw.githubusercontent.com/Tviskaron/paper-linter/main/install.sh | sh -s -- --yes
```

### From source

From a local checkout:

```console
git clone git@github.com:Tviskaron/paper-linter.git
cd paper-linter
cargo install --path .
```

Run the linter:

```console
paper-linter check paper.tex
paper-linter check paper.tex --strict
paper-linter check paper.tex --format json
paper-linter check . --select WS --ignore WS001
paper-linter check . --all-tex
paper-linter check paper.tex refs.bib --select CIT
```

For directory inputs, `paper-linter` follows likely root files such as
`main.tex` through `\input`, `\include`, and `\subfile` references. Use
`--all-tex` to scan every `.tex` file recursively, including templates, old
drafts, and samples.

For development, run without installing:

```console
cargo run -- check paper.tex
```

### Verification

Before committing changes, run:

```console
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all
```

For manual real-paper smoke tests, collect local arXiv sources into the ignored
`sample-corpus/` directory:

```console
scripts/fetch_arxiv_corpus.sh 1706.03762 1810.04805
cargo run -- check sample-corpus/1706.03762 --select CIT
```

## Output Formats

- **CLI**: colored terminal output.
- **JSON**: machine-readable diagnostics.
- **SARIF**: GitHub Actions integration.
- **LSP**: editor diagnostics, including VS Code.

Current implementation supports text and JSON output. SARIF and LSP are roadmap
items.

## Adding a Rule Module

File-local rules live in `src/rules/` and implement the `Rule` trait:

```rust
pub trait Rule: Sync {
    fn code(&self) -> &'static str;
    fn name(&self) -> &'static str;
    fn check_file(&self, path: &Path, content: &str) -> Vec<Diagnostic>;
}
```

To add a new rule:

1. Create a new file in `src/rules/`, for example `txt001.rs`.
2. Define a rule struct and implement `Rule` for it.
3. Return diagnostics with `Diagnostic::new(...)`.
4. Register the rule in `src/rules/mod.rs` by adding the module, static rule
   value, and entry in `RULES`.
5. Add unit tests beside the rule and CLI/integration tests if the rule affects
   command behavior.
6. Run the verification commands above.

Minimal pattern:

```rust
use std::path::Path;

use crate::diagnostic::{Diagnostic, Severity};
use crate::rules::Rule;

pub struct MyRule;

impl Rule for MyRule {
    fn code(&self) -> &'static str {
        "TXT001"
    }

    fn name(&self) -> &'static str {
        "short rule name"
    }

    fn check_file(&self, path: &Path, content: &str) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();

        for (index, line) in content.lines().enumerate() {
            if line.contains("TODO") {
                diagnostics.push(Diagnostic::new(
                    self.code(),
                    Severity::Warning,
                    "TODO left in paper",
                    path,
                    index + 1,
                    1,
                ));
            }
        }

        diagnostics
    }
}
```

Keep new modules fast and source-only by default: scan text, preserve line and
column positions, avoid invoking TeX, avoid network access, and leave expensive
checks for explicit future commands.

For project-level checks that need multiple files, follow the abstraction tree
in `DESIGN_NOTES.md`. The citation checker shows the intended subsystem shape:
keep a small facade/orchestrator, split scanners/parsers/resolvers/analysis into
focused private submodules, return normal `Diagnostic` values, and avoid adding
new checker special cases when a project-rule registry can own the dispatch.

## Roadmap

- **MVP**: v0.1
- **Full roadmap**: v1.0
