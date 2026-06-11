use std::path::Path;

use crate::diagnostic::{Diagnostic, Severity};
use crate::latex::commands::commands_in_line;
use crate::rules::Rule;

pub struct PrimitiveTex;

impl Rule for PrimitiveTex {
    fn code(&self) -> &'static str {
        "LAT002"
    }

    fn name(&self) -> &'static str {
        "primitive tex"
    }

    fn strict_only(&self) -> bool {
        true
    }

    fn check_file(&self, path: &Path, content: &str) -> Vec<Diagnostic> {
        content
            .lines()
            .enumerate()
            .flat_map(|(index, line)| commands_in_line(line, index + 1))
            .filter_map(|command| {
                primitive_hint(&command.name).map(|hint| {
                    Diagnostic::new(
                        self.code(),
                        Severity::Warning,
                        format!("primitive TeX command '\\{}' is discouraged", command.name),
                        path,
                        command.line,
                        command.column,
                    )
                    .with_hint(hint)
                })
            })
            .collect()
    }
}

fn primitive_hint(command: &str) -> Option<&'static str> {
    match command {
        "def" => Some("use \\newcommand, \\renewcommand, or \\DeclareRobustCommand"),
        "let" => Some("prefer a LaTeX-level command alias when possible"),
        "above" => Some("use \\frac or an amsmath display construct"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use crate::rules::Rule;

    use super::{primitive_hint, PrimitiveTex};

    #[test]
    fn recognizes_primitive_commands() {
        assert!(primitive_hint("def").is_some());
        assert!(primitive_hint("let").is_some());
        assert!(primitive_hint("above").is_some());
        assert!(primitive_hint("newcommand").is_none());
    }

    #[test]
    fn ignores_commented_primitives() {
        let diagnostics = PrimitiveTex.check_file(Path::new("paper.tex"), "% \\def\\x{y}\n");

        assert!(diagnostics.is_empty());
    }
}
