# paper-linter

An extremely fast linter for LaTeX papers.

`paper-linter` is designed to be 10-100x faster than existing tools while staying simple to install, easy to run, and useful in editors, CI, and pre-commit hooks.

## Highlights

- **Written in Rust**: parallel, memory-safe, no runtime.
- **Zero configuration**: sensible defaults, with opt-in strict mode.
- **Single binary, no deps**: drop into CI or pre-commit in seconds.

## What It Checks

### References & Citations

- Every `\cite{}` key exists in `.bib`.
- Every `.bib` entry is cited.
- Required fields: author, year, venue.
- Roadmap: DOI/URL validation, duplicate bibliography detection, consistent `\citet` / `\citep` style.

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
$ paper-linter check paper.tex
paper.tex:42:18: warning[WS001] trailing whitespace
checked 1 file(s), 0 error(s), 1 warning(s)
```

Current implementation status: the core CLI and rule pipeline are in place, with
`WS001` trailing whitespace as the first proving rule. The checks listed above
describe the intended MVP and v1.0 roadmap.

## Installation

### From source

Requirements:

- Rust toolchain with Cargo.

Build and install from this repository:

```console
$ cargo install --path .
```

Run the linter:

```console
$ paper-linter check paper.tex
$ paper-linter check paper.tex --strict
$ paper-linter check paper.tex --format json
$ paper-linter check . --select WS --ignore WS001
```

For development, run without installing:

```console
$ cargo run -- check paper.tex
```

### Verification

Before committing changes, run:

```console
$ cargo fmt --check
$ cargo clippy --all-targets --all-features -- -D warnings
$ cargo test --all
```

## Output Formats

- **CLI**: colored terminal output.
- **JSON**: machine-readable diagnostics.
- **SARIF**: GitHub Actions integration.
- **LSP**: editor diagnostics, including VS Code.

Current implementation supports text and JSON output. SARIF and LSP are roadmap
items.

## Adding a Rule Module

Rules live in `src/rules/` and implement the `Rule` trait:

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

## Roadmap

- **MVP**: v0.1
- **Full roadmap**: v1.0
