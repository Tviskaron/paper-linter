use std::path::Path;

use crate::diagnostic::{Diagnostic, Severity};
use crate::latex::prose::prose_spans;
use crate::rules::Rule;

const PASSIVE_PATTERNS: &[&str] = &[
    " am ",
    " is ",
    " are ",
    " was ",
    " were ",
    " be ",
    " been ",
    " being ",
];

const PARTICIPLES: &[&str] = &[
    " shown ",
    " given ",
    " found ",
    " observed ",
    " demonstrated ",
    " presented ",
    " performed ",
    " obtained ",
    " proposed ",
    " used ",
];

pub struct PassiveVoice;

impl Rule for PassiveVoice {
    fn code(&self) -> &'static str {
        "TXT005"
    }

    fn name(&self) -> &'static str {
        "passive voice"
    }

    fn strict_only(&self) -> bool {
        true
    }

    fn check_file(&self, path: &Path, content: &str) -> Vec<Diagnostic> {
        prose_spans(content)
            .into_iter()
            .filter_map(|span| {
                let padded = format!(" {} ", span.text.to_ascii_lowercase());
                let column = passive_match_column(&span.text, &padded)?;
                Some(
                    Diagnostic::new(
                        self.code(),
                        Severity::Warning,
                        "possible passive-voice phrasing".to_string(),
                        path,
                        span.line,
                        span.start_column + column,
                    )
                    .with_hint("consider rewriting in active voice if appropriate"),
                )
            })
            .collect()
    }
}

fn passive_match_column(original: &str, padded: &str) -> Option<usize> {
    for pattern in PASSIVE_PATTERNS {
        let Some(index) = padded.find(pattern) else {
            continue;
        };
        let after = &padded[index + pattern.len()..];
        if PARTICIPLES.iter().any(|participle| after.contains(participle))
            || after.contains(" by ")
            || after.split_whitespace().next().is_some_and(|word| word.ends_with("ed"))
        {
            return Some(index.max(1) - 1);
        }
    }

    let _ = original;
    None
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use crate::rules::Rule;

    use super::PassiveVoice;

    #[test]
    fn detects_likely_passive_voice() {
        let diagnostics = PassiveVoice.check_file(
            Path::new("paper.tex"),
            "The results were shown by the model.\n",
        );
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, "TXT005");
    }
}
