use std::path::Path;

use crate::diagnostic::{Diagnostic, Severity};
use crate::latex::prose::{prose_spans, sentences};
use crate::rules::Rule;

const MAX_WORDS: usize = 40;

pub struct LongSentence;

impl Rule for LongSentence {
    fn code(&self) -> &'static str {
        "TXT003"
    }

    fn name(&self) -> &'static str {
        "long sentence"
    }

    fn strict_only(&self) -> bool {
        true
    }

    fn check_file(&self, path: &Path, content: &str) -> Vec<Diagnostic> {
        if path
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("bbl"))
        {
            return Vec::new();
        }

        prose_spans(content)
            .into_iter()
            .flat_map(|span| sentences(&span))
            .filter(|sentence| sentence.word_count > MAX_WORDS)
            .map(|sentence| {
                Diagnostic::new(
                    self.code(),
                    Severity::Warning,
                    format!(
                        "sentence has {} words (threshold {})",
                        sentence.word_count, MAX_WORDS
                    ),
                    path,
                    sentence.line,
                    sentence.start_column,
                )
                .with_hint("consider splitting the sentence for readability")
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use crate::rules::Rule;

    use super::LongSentence;

    #[test]
    fn flags_long_sentence() {
        let text = "This sentence repeats words many times and keeps going with extra detail about experiments datasets metrics baselines ablations conclusions methods results analysis findings observations comparisons evaluations benchmarks protocols implementations architectures components modules pipelines workflows systems models datasets again and again today.\n";
        let diagnostics = LongSentence.check_file(Path::new("paper.tex"), text);
        assert!(!diagnostics.is_empty());
        assert_eq!(diagnostics[0].code, "TXT003");
    }

    #[test]
    fn ignores_short_sentence() {
        let diagnostics = LongSentence.check_file(Path::new("paper.tex"), "This is short.\n");
        assert!(diagnostics.is_empty());
    }
}
