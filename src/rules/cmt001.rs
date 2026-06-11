use std::path::Path;

use crate::diagnostic::{Diagnostic, Severity};
use crate::rules::Rule;

const EDITORIAL_TOKENS: &[&str] = &["todo", "fixme", "xxx", "tbd", "hack"];
const EDITORIAL_PHRASES: &[&str] = &[
    "check this",
    "fix this",
    "rewrite this",
    "add this",
    "move this",
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

    for token in EDITORIAL_TOKENS {
        if let Some(index) = find_token(&lower, token) {
            let column = comment[..index].chars().count() + 2;
            return Some((token, column));
        }
    }

    for phrase in EDITORIAL_PHRASES {
        if let Some(index) = lower.find(phrase) {
            let column = comment[..index].chars().count() + 2;
            return Some((phrase, column));
        }
    }
    None
}

fn find_token(haystack: &str, token: &str) -> Option<usize> {
    let mut search_start = 0;

    while let Some(relative) = haystack[search_start..].find(token) {
        let index = search_start + relative;
        let before = haystack[..index].chars().next_back();
        let after = haystack[index + token.len()..].chars().next();

        if !is_word_character(before) && !is_word_character(after) {
            return Some(index);
        }

        search_start = index + token.len();
    }

    None
}

fn is_word_character(character: Option<char>) -> bool {
    character.is_some_and(|character| character.is_ascii_alphanumeric() || character == '_')
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

    #[test]
    fn detects_common_editorial_markers_as_tokens() {
        let content = "% FIXME: update\n% XXX remove\n% TBD\n% HACK: temporary\n";
        let diagnostics = EditorialComment.check_file(Path::new("paper.tex"), content);

        assert_eq!(diagnostics.len(), 4);
    }

    #[test]
    fn ignores_markers_inside_words() {
        let content =
            "% todorov2012mujoco\n% \\usepackage{todonotes}\n% \\iftodonotes\n% NetHack\n% reward hacking\n";
        let diagnostics = EditorialComment.check_file(Path::new("paper.tex"), content);

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn ignores_bare_review_and_rewrite_words() {
        let content =
            "% review mode enabled by template\n% reviewers should see checklist\n% rewrite systems are discussed\n";
        let diagnostics = EditorialComment.check_file(Path::new("paper.tex"), content);

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn detects_strong_editorial_phrases() {
        let content = "% check this claim\n% rewrite this paragraph\n";
        let diagnostics = EditorialComment.check_file(Path::new("paper.tex"), content);

        assert_eq!(diagnostics.len(), 2);
    }
}
