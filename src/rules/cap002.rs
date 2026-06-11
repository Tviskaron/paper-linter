use crate::diagnostic::{Diagnostic, Severity};
use crate::project::ProjectIndex;
use crate::rules::ProjectRule;

pub struct CaptionPunctuation;

impl ProjectRule for CaptionPunctuation {
    fn code(&self) -> &'static str {
        "CAP002"
    }

    fn name(&self) -> &'static str {
        "caption-punctuation"
    }

    fn strict_only(&self) -> bool {
        true
    }

    fn check_project(&self, project: &ProjectIndex) -> Vec<Diagnostic> {
        project
            .floats
            .iter()
            .flat_map(|float| {
                float
                    .captions
                    .iter()
                    .filter(|caption| !caption_has_terminal_punctuation(&caption.text))
                    .map(move |caption| {
                        Diagnostic::new(
                            self.code(),
                            Severity::Warning,
                            format!(
                                "{} caption should end with punctuation",
                                float.kind.as_str()
                            ),
                            &caption.location.file,
                            caption.location.line,
                            caption.location.column,
                        )
                        .with_hint("end the caption with '.', '?', or '!'")
                    })
            })
            .collect()
    }
}

fn caption_has_terminal_punctuation(text: &str) -> bool {
    text.trim_end()
        .chars()
        .rev()
        .find(|character| !matches!(character, '}' | ']' | ')' | '\'' | '"'))
        .is_some_and(|character| matches!(character, '.' | '?' | '!'))
}

#[cfg(test)]
mod tests {
    use super::caption_has_terminal_punctuation;

    #[test]
    fn detects_terminal_punctuation() {
        assert!(caption_has_terminal_punctuation("Result."));
        assert!(caption_has_terminal_punctuation("Result?"));
        assert!(caption_has_terminal_punctuation("Result!"));
        assert!(caption_has_terminal_punctuation("Result.\""));
        assert!(!caption_has_terminal_punctuation("Result"));
    }
}
