use std::path::Path;

use crate::diagnostic::{Diagnostic, Severity};
use crate::rules::Rule;

const EDITORIAL_PATTERNS: &[&str] = &[
    "todo",
    "fixme",
    "xxx",
    "hack",
    "review",
    "rewrite",
    "check this",
    "@author",
];

pub struct EditorialComment;

impl Rule for EditorialComment {
    fn code(&self) -> &'static str {
        "CMT001"
    }

    fn name(&self) -> &'static str {
        "editorial comment"
    }

    fn strict_only(&self) -> bool {
        true
    }

    fn check_file(&self, path: &Path, content: &str) -> Vec<Diagnostic> {
        content
            .lines()
            .enumerate()
            .filter_map(|(index, line)| {
                let comment = comment_text(line)?;
                if is_layout_comment(line, comment) {
                    return None;
                }
                editorial_match(comment).map(|(pattern, column)| {
                    Diagnostic::new(
                        self.code(),
                        Severity::Warning,
                        format!("editorial comment contains '{pattern}'"),
                        path,
                        index + 1,
                        column,
                    )
                    .with_hint("resolve or remove the editorial comment before submission")
                })
            })
            .collect()
    }
}

fn comment_text(line: &str) -> Option<&str> {
    let index = line.find('%')?;
    if index > 0 && line.as_bytes()[index - 1] == b'\\' {
        return None;
    }
    Some(line[index + 1..].trim())
}

fn is_layout_comment(line: &str, comment: &str) -> bool {
    comment.is_empty()
        || (line.trim_end().ends_with('%') && comment.is_empty())
        || line.trim_start().starts_with('%') && comment.is_empty()
}

fn editorial_match(comment: &str) -> Option<(&'static str, usize)> {
    let lower = comment.to_ascii_lowercase();
    for pattern in EDITORIAL_PATTERNS {
        if let Some(index) = lower.find(pattern) {
            let column = comment[..index].chars().count() + 2;
            return Some((pattern, column));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use crate::rules::Rule;

    use super::EditorialComment;

    #[test]
    fn detects_todo_in_comment() {
        let diagnostics =
            EditorialComment.check_file(Path::new("paper.tex"), "% TODO: fix figure\n");
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, "CMT001");
    }
}
