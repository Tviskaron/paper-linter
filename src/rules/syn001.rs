use std::path::Path;

use crate::diagnostic::{Diagnostic, Severity};
use crate::rules::Rule;

const RISKY_COMMANDS: &[&str] = &["author", "title", "thanks"];

pub struct PreambleBraceBalance;

impl Rule for PreambleBraceBalance {
    fn code(&self) -> &'static str {
        "SYN001"
    }

    fn name(&self) -> &'static str {
        "preamble brace balance"
    }

    fn check_file(&self, path: &Path, content: &str) -> Vec<Diagnostic> {
        let preamble = extract_preamble(content);
        let mut diagnostics = Vec::new();

        for command in RISKY_COMMANDS {
            for occurrence in command_occurrences(preamble, command) {
                if let Some(issue) = first_argument_brace_issue(preamble, occurrence.after_command)
                {
                    let (line, column) = line_column(preamble, occurrence.command_start);
                    diagnostics.push(
                        Diagnostic::new(
                            self.code(),
                            Severity::Error,
                            format!("unbalanced braces in \\{command}: {issue}"),
                            path,
                            line,
                            column,
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
    content.split("\\begin{document}").next().unwrap_or(content)
}

#[derive(Debug, Clone, Copy)]
struct CommandOccurrence {
    command_start: usize,
    after_command: usize,
}

fn command_occurrences(content: &str, command: &str) -> Vec<CommandOccurrence> {
    let marker = format!("\\{command}");
    let mut occurrences = Vec::new();
    let mut offset = 0;

    while let Some(relative) = content[offset..].find(&marker) {
        let command_start = offset + relative;
        let after_command = command_start + marker.len();
        if content[after_command..]
            .chars()
            .next()
            .is_none_or(|ch| !ch.is_ascii_alphabetic())
            && !is_commented_position(content, command_start)
        {
            occurrences.push(CommandOccurrence {
                command_start,
                after_command,
            });
        }
        offset = after_command;
    }

    occurrences
}

fn first_argument_brace_issue(content: &str, mut index: usize) -> Option<&'static str> {
    while let Some(ch) = content[index..].chars().next() {
        if ch.is_whitespace() {
            index += ch.len_utf8();
        } else {
            break;
        }
    }

    if content.as_bytes().get(index) != Some(&b'{') {
        return None;
    }

    let mut depth = 0i32;
    let mut escaped = false;
    while index < content.len() {
        let ch = content[index..]
            .chars()
            .next()
            .expect("valid char boundary");

        if escaped {
            escaped = false;
            index += ch.len_utf8();
            continue;
        }

        if ch == '%' && !is_escaped(content, index) {
            index = content[index..]
                .find('\n')
                .map(|relative| index + relative + 1)
                .unwrap_or(content.len());
            continue;
        }

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

        escaped = ch == '\\';
        index += ch.len_utf8();
    }

    Some("missing closing brace")
}

fn is_commented_position(content: &str, byte_index: usize) -> bool {
    let line_start = content[..byte_index]
        .rfind('\n')
        .map(|index| index + 1)
        .unwrap_or(0);

    content[line_start..byte_index]
        .char_indices()
        .any(|(relative, ch)| ch == '%' && !is_escaped(content, line_start + relative))
}

fn is_escaped(content: &str, byte_index: usize) -> bool {
    let mut count = 0usize;
    let mut index = byte_index;
    while index > 0 && content.as_bytes().get(index - 1) == Some(&b'\\') {
        count += 1;
        index -= 1;
    }
    count % 2 == 1
}

fn line_column(content: &str, byte_index: usize) -> (usize, usize) {
    let line_start = content[..byte_index]
        .rfind('\n')
        .map(|index| index + 1)
        .unwrap_or(0);
    (
        content[..byte_index].matches('\n').count() + 1,
        content[line_start..byte_index].chars().count() + 1,
    )
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::PreambleBraceBalance;
    use crate::rules::Rule;

    #[test]
    fn detects_unclosed_author_block() {
        let content =
            "\\documentclass{article}\n\\author{Alice \\thanks{equal}\n\\begin{document}\n";
        let diagnostics = PreambleBraceBalance.check_file(Path::new("main.tex"), content);
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, "SYN001");
    }

    #[test]
    fn accepts_balanced_author_block() {
        let content =
            "\\documentclass{article}\n\\author{\nAlice \\thanks{equal}\n}\n\\begin{document}\n";
        let diagnostics = PreambleBraceBalance.check_file(Path::new("main.tex"), content);
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn ignores_body_footnotes_in_included_files() {
        let content = "Text\\footnote{\nA multiline body footnote.\n} continues.\n";
        let diagnostics = PreambleBraceBalance.check_file(Path::new("section.tex"), content);
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn ignores_commented_braces_inside_preamble_argument() {
        let content =
            "\\documentclass{article}\n\\author{Alice\n% \\thanks{template\n}\n\\begin{document}\n";
        let diagnostics = PreambleBraceBalance.check_file(Path::new("main.tex"), content);
        assert!(diagnostics.is_empty());
    }
}
