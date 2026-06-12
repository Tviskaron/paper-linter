use crate::diagnostic::{Diagnostic, Severity};
use crate::rules::Rule;

const RISKY_COMMANDS: &[&str] = &["author", "title", "thanks", "footnote"];

pub struct PreambleBraceBalance;

impl Rule for PreambleBraceBalance {
    fn code(&self) -> &'static str {
        "SYN001"
    }

    fn name(&self) -> &'static str {
        "preamble brace balance"
    }

    fn check_file(&self, path: &std::path::Path, content: &str) -> Vec<Diagnostic> {
        let preamble = extract_preamble(content);
        let mut diagnostics = Vec::new();

        for (line_number, line) in preamble.lines().enumerate() {
            let line_number = line_number + 1;
            let line = uncomment_line(line);
            if !line.contains('\\') {
                continue;
            }

            for command in RISKY_COMMANDS {
                let marker = format!("\\{command}");
                let Some(relative) = line.find(&marker) else {
                    continue;
                };
                if let Some(issue) = first_argument_brace_issue(line, relative + marker.len()) {
                    diagnostics.push(
                        Diagnostic::new(
                            self.code(),
                            Severity::Error,
                            format!("unbalanced braces in \\{command}: {issue}"),
                            path,
                            line_number,
                            relative + 1,
                        )
                        .with_hint(
                            "check nested \\thanks, \\footnote, or line breaks inside the argument",
                        ),
                    );
                }
            }
        }

        diagnostics
    }
}

fn extract_preamble(content: &str) -> &str {
    content
        .split("\\begin{document}")
        .next()
        .unwrap_or(content)
}

fn uncomment_line(line: &str) -> &str {
    line.split('%').next().unwrap_or(line)
}

fn first_argument_brace_issue(line: &str, mut index: usize) -> Option<&'static str> {
    while let Some(ch) = line[index..].chars().next() {
        if ch.is_whitespace() {
            index += ch.len_utf8();
        } else {
            break;
        }
    }

    if line.as_bytes().get(index) != Some(&b'{') {
        return None;
    }

    let mut depth = 0i32;
    for ch in line[index..].chars() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth < 0 {
                    return Some("extra closing brace");
                }
                if depth == 0 {
                    return None;
                }
            }
            _ => {}
        }
    }

    Some("missing closing brace")
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::PreambleBraceBalance;
    use crate::rules::Rule;

    #[test]
    fn detects_unclosed_author_block() {
        let content = "\\documentclass{article}\n\\author{Alice \\thanks{equal}\n\\begin{document}\n";
        let diagnostics = PreambleBraceBalance.check_file(Path::new("main.tex"), content);
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, "SYN001");
    }

    #[test]
    fn accepts_balanced_author_block() {
        let content = "\\documentclass{article}\n\\author{Alice \\thanks{equal}}\n\\begin{document}\n";
        let diagnostics = PreambleBraceBalance.check_file(Path::new("main.tex"), content);
        assert!(diagnostics.is_empty());
    }
}
