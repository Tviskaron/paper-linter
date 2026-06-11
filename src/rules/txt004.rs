use std::path::Path;

use crate::diagnostic::{Diagnostic, Severity};
use crate::latex::prose::{prose_spans, prose_words};
use crate::rules::Rule;

const FILLER_WORDS: &[&str] = &[
    "actually",
    "basically",
    "clearly",
    "easily",
    "extremely",
    "fairly",
    "highly",
    "interestingly",
    "just",
    "literally",
    "mostly",
    "naturally",
    "obviously",
    "quite",
    "really",
    "relatively",
    "remarkably",
    "severely",
    "significantly",
    "simply",
    "somewhat",
    "strongly",
    "surprisingly",
    "truly",
    "undoubtedly",
    "undue",
    "very",
    "virtually",
];

pub struct FillerWords;

impl Rule for FillerWords {
    fn code(&self) -> &'static str {
        "TXT004"
    }

    fn name(&self) -> &'static str {
        "filler word"
    }

    fn strict_only(&self) -> bool {
        true
    }

    fn check_file(&self, path: &Path, content: &str) -> Vec<Diagnostic> {
        prose_spans(content)
            .into_iter()
            .flat_map(|span| {
                prose_words(&span)
                    .into_iter()
                    .filter_map(move |(word, column)| {
                        if is_filler(&word) {
                            Some(
                                Diagnostic::new(
                                    self.code(),
                                    Severity::Warning,
                                    format!("filler word '{word}'"),
                                    path,
                                    span.line,
                                    column,
                                )
                                .with_hint(
                                    "consider removing the filler or using stronger wording",
                                ),
                            )
                        } else {
                            None
                        }
                    })
            })
            .collect()
    }
}

fn is_filler(word: &str) -> bool {
    let normalized = word.to_ascii_lowercase();
    FILLER_WORDS
        .iter()
        .any(|candidate| normalized == *candidate)
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use crate::rules::Rule;

    use super::FillerWords;

    #[test]
    fn detects_filler_word() {
        let diagnostics =
            FillerWords.check_file(Path::new("paper.tex"), "This is clearly useful.\n");
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, "TXT004");
    }
}
