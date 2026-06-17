# PaperLinter

[![CI](https://github.com/Tviskaron/paper-linter/actions/workflows/ci.yml/badge.svg)](https://github.com/Tviskaron/paper-linter/actions/workflows/ci.yml)

PaperLinter is a fast, deterministic Rust linter for LaTeX papers. It is built
for local editing, CI, pre-commit hooks, and batch corpus checks without
compiling LaTeX by default.

<p align="center">
  <img src="logo/horizontal_logo.svg" alt="PaperLinter" width="560">
</p>


## Highlights

- **Single binary**: no Python runtime, TeX installation, database, or network
  access required for normal linting.
- **Zero-config defaults**: conservative checks run by default; noisy style and
  strict checks are opt-in.
- **Project-aware LaTeX scanning**: follows `\input`, `\include`, and
  `\subfile`, indexes labels, references, citations, figures, tables, packages,
  and common build artifacts.
- **CI-friendly output**: text, JSON, SARIF, and LSP-shaped diagnostics.
- **Fast by design**: source-only static analysis, stable rule IDs, small
  deterministic diagnostics.

## Quick Start

```console
cargo install --git https://github.com/Tviskaron/paper-linter.git --force
paper-linter check paper.tex
```

Common commands:

```console
paper-linter check paper.tex
paper-linter check . --all-tex
paper-linter check . --all
paper-linter check paper.tex --strict
paper-linter check paper.tex --select FIG,CAP,REF
paper-linter check paper.tex --ignore TXT
paper-linter check paper.tex --format json
paper-linter check paper.tex --format sarif
paper-linter ready .
paper-linter doctor .
paper-linter pack . --dry-run
paper-linter format . --check
paper-linter rules
paper-linter explain FIG001
```

Example output:

```console
paper.tex:42:18: warning[TXT001] placeholder text
checked 1 file(s), 0 error(s), 1 warning(s)
```

## Rule Selection

PaperLinter keeps default mode conservative.

- `--select FIG,CAP`: run only matching rule codes or families.
- `--ignore TXT`: suppress matching rule codes or families after selection.
- `--strict`: enable strict-only rules and promote most warnings to errors.
- `--all`: enable every known rule without promoting warnings to errors.
- `--all-tex`: scan every `.tex` file under directory inputs instead of only
  files reachable from the detected root.
- `--preset essential|standard|strict|polish`: apply a bundled rule profile.

Use `paper-linter rules` to list known rules and `paper-linter explain CODE` for
the rationale and suggested fix for one rule.

Bundled profiles:

- `essential`: low-noise checks for critical source/package issues.
- `standard`: broader structure, package, caption, syntax, and typography checks.
- `strict`: stricter style and bibliography checks for tighter review before submission.
- `polish`: prose and formatting cleanup checks.

## What It Checks

### Project Structure

- Missing `\input`, `\include`, and `\subfile` targets.
- Ambiguous or missing project roots.
- Orphan `.tex` files that are not reachable from the root document.
- Package and preamble risks, including option clashes, missing dependencies,
  and unbalanced preamble braces.
- Build artifact issues from `.log`, `.blg`, `.aux`, and compile comparison
  reports when those files are present.

### References, Labels, and Citations

- Missing `\ref`-like targets.
- Unused labels, with noisy label checks kept opt-in.
- Missing bibliography keys, unused bibliography entries, duplicate keys, and
  duplicate bibliography declarations.
- Citation style issues such as consecutive collapsible citations, mixed
  citation command families, punctuation before citations, and non-canonical
  bibliography keys.
- `.bbl` fallback support for arXiv source bundles that omit `.bib` files.

### Figures, Tables, and Algorithms

- Missing figure/table captions.
- Missing, unsafe, unsupported, corrupt, or case-mismatched image assets.
- Orphan figure, table, and algorithm labels.
- Figure labels missing on real top-level figures.
- Caption punctuation, image format, image resolution, and image header checks
  in strict or explicitly selected modes.

### LaTeX, Math, and Source Hygiene

- Environment begin/end mismatches.
- Legacy LaTeX packages/environments and primitive TeX commands.
- Double-dollar display math, unbraced multi-character scripts, and raw text
  operators inside math.
- Missing non-breaking spaces before references and citations.
- Missing final newline, repeated blank lines, and trailing whitespace.

### Writing and Paper Polish

- Placeholder text, TODO markers, Lorem Ipsum, and editorial comments.
- Repeated words.
- Long sentences, filler words, and passive-voice heuristics in opt-in modes.
- Section hierarchy problems, empty sections, singleton subdivisions, stacked
  headings, short sections, and heading style issues.

## Commands

### `check`

Runs lint rules and returns a non-zero exit code only when errors remain after
selection, suppressions, and baselines.

```console
paper-linter check path/to/paper.tex
paper-linter check path/to/paper.tex --format terminal
paper-linter check path/to/project --format sarif
paper-linter check path/to/project --baseline paper-linter-baseline.json
paper-linter check path/to/project --update-baseline paper-linter-baseline.json
```

Use `--format terminal` for compact colored output in editor hooks and local
agent loops. Keep `--format text`, `json`, `sarif`, or `lsp` for reports and
machine-readable integrations.

### `ready`

Prints a compact readiness summary for local review or CI gates.

```console
paper-linter ready .
paper-linter ready . --format json
```

### `doctor`

Explains how PaperLinter sees the project: discovered files, root selection,
packages, labels, refs, graphics, floats, and include graph facts.

```console
paper-linter doctor .
paper-linter doctor . --format json
```

### `pack`

Performs a read-only submission bundle audit. It lists local assets, missing
graphics, case mismatches, bibliography/style/class files, and missing includes.

```console
paper-linter pack . --dry-run
paper-linter pack . --dry-run --format json
```

### `format`

Applies or checks safe mechanical formatting only.

```console
paper-linter format . --check
paper-linter format . --diff
paper-linter format . --write
```

### `index`

Writes the project index to JSON so another run can reuse it.

```console
paper-linter index paper.tex --output project-index.json
paper-linter check paper.tex --project-index project-index.json
```

### `suggest`

Prints targeted suggestions for supported rules.

```console
paper-linter suggest paper.tex --rule TXT003
```

## Configuration

PaperLinter works without configuration. Optional configuration files and
presets can enable aliases, rule families, thresholds, and bibliography policy.

```console
paper-linter check . --config paper-linter.toml
paper-linter check . --preset standard
```

Example:

```toml
enable = ["FIG", "REF"]
disable = ["TXT005"]

[aliases]
ref = ["figref", "secref"]
cite = ["mycite"]
input = ["subimport"]
graphic = ["plotfile"]

[bibliography]
forbidden_fields = ["file", "abstract"]
```

Inline suppressions:

```tex
% paper-linter-ignore-next-line FIG001
\includegraphics{generated-at-build-time}

\caption{Draft caption} % paper-linter-ignore-line CAP002

% paper-linter-ignore-file TXT
```

## Output Formats

- `text`: compact terminal output.
- `json`: machine-readable diagnostics and summary.
- `sarif`: GitHub code scanning and CI integrations.
- `lsp`: editor-friendly diagnostic payloads.

## Installation

### Requirements

- Rust and Cargo. Install from <https://rustup.rs/> if `cargo --version` does
  not work.

### Cargo

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

### Installer Script

```console
curl -fsSL https://raw.githubusercontent.com/Tviskaron/paper-linter/main/install.sh | sh -s -- --yes
```

Or clone the public repository first:

```console
git clone https://github.com/Tviskaron/paper-linter.git
cd paper-linter
./install.sh --yes
```

### From Source

```console
git clone https://github.com/Tviskaron/paper-linter.git
cd paper-linter
cargo install --path .
```

For development, run without installing:

```console
cargo run -- check paper.tex
```

## Optional: Suggestion and Fixture Scripts

Optional Python tooling lives under `scripts/` and does not affect default
`check` behavior:

- [`scripts/README.md`](scripts/README.md) — arXiv compile/compare helpers and env vars
- [`scripts/ml/README.md`](scripts/ml/README.md) — LoRA training and `suggest --ml-model`
- [`scripts/llm_validation/README.md`](scripts/llm_validation/README.md) — Ollama benchmark harness

```console
paper-linter suggest paper.tex --rule TXT001
paper-linter suggest paper.tex --ml-model path/to/adapter
```

## Development

Before committing changes:

```console
cargo fmt --all --check
cargo check --all-targets --all-features --locked
cargo clippy --all-targets --all-features --locked -- -D warnings
cargo test --all --locked
```

CI runs the same fast gate on pushes and pull requests. Large real-paper corpus
checks remain manual so required CI stays deterministic and quick.

Manual arXiv smoke test:

```console
scripts/fetch_arxiv_corpus.sh 1706.03762 1810.04805
cargo run -- check sample-corpus/1706.03762 --select CIT
```

## Adding Rules

File-local rules live in `src/rules/` and implement `Rule`. Project-aware rules
implement `ProjectRule` or `GraphProjectRule` and should use the scanner,
project index, and include graph instead of ad hoc whole-file regexes.

Checklist:

- Keep the rule source-only, deterministic, and non-panicking on malformed TeX.
- Preserve exact file, line, and column positions when possible.
- Prefer conservative defaults; put noisy style checks behind `--strict`,
  `--all`, presets, or explicit `--select`.
- Add focused unit tests and at least one CLI/integration regression for user
  visible behavior.
- Add minimized fixtures for real-paper false-positive fixes.
- Register rule metadata in `src/rules/mod.rs` so `rules`, `explain`, JSON,
  SARIF, and LSP outputs stay complete.

Minimal file-local pattern:

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
        "short-rule-name"
    }

    fn check_file(&self, path: &Path, content: &str) -> Vec<Diagnostic> {
        content
            .lines()
            .enumerate()
            .filter(|(_, line)| line.contains("TODO"))
            .map(|(index, _)| {
                Diagnostic::new(
                    self.code(),
                    Severity::Warning,
                    "TODO left in paper",
                    path,
                    index + 1,
                    1,
                )
            })
            .collect()
    }
}
```

## Design Principles

- Do not compile LaTeX for normal linting.
- Do not implement a full TeX interpreter.
- Prefer reusable scanners, events, indexes, and rule modules over one-off
  regular expressions.
- Default mode should be low-noise and suitable for CI.
- Strict, venue-specific, or subjective checks must be opt-in.
