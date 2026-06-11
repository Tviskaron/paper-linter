use std::path::Path;

use crate::diagnostic::{Diagnostic, Severity};
use crate::latex::prose::prose_spans;
use crate::rules::Rule;

pub struct HardCodedReferenceNumber;

impl Rule for HardCodedReferenceNumber {
    fn code(&self) -> &'static str {
        "TEX002"
    }

    fn name(&self) -> &'static str {
        "hard-coded reference number"
    }

    fn strict_only(&self) -> bool {
        true
    }

    fn check_file(&self, path: &Path, content: &str) -> Vec<Diagnostic> {
        prose_spans(content)
            .into_iter()
            .flat_map(|span| {
                hard_coded_references(&span.text)
                    .into_iter()
                    .map(|reference| {
                        Diagnostic::new(
                            self.code(),
                            Severity::Warning,
                            format!(
                                "{} {} looks like a hard-coded reference",
                                reference.kind, reference.number
                            ),
                            path,
                            span.line,
                            span.start_column + reference.column - 1,
                        )
                        .with_hint(format!(
                            "use \\ref{{...}} or \\cref{{...}} for the {} number",
                            reference.kind.to_ascii_lowercase()
                        ))
                    })
                    .collect::<Vec<_>>()
            })
            .collect()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct HardCodedReference<'a> {
    kind: &'a str,
    number: &'a str,
    column: usize,
}

fn hard_coded_references(line: &str) -> Vec<HardCodedReference<'_>> {
    let words = indexed_words(line);
    let mut references = Vec::new();

    for pair in words.windows(2) {
        let kind = canonical_reference_kind(pair[0].text);
        if kind.is_none() || !is_reference_number(pair[1].text) {
            continue;
        }
        if is_external_reference_context(line, &words, pair[0].column) {
            continue;
        }

        references.push(HardCodedReference {
            kind: kind.expect("checked above"),
            number: pair[1].text,
            column: pair[0].column,
        });
    }

    references
}

fn is_external_reference_context(line: &str, words: &[IndexedWord<'_>], column: usize) -> bool {
    let Some(index) = words.iter().position(|word| word.column == column) else {
        return false;
    };

    is_inside_citation_optional_arg(line, column)
        || followed_by_citation_locator(line, words, index)
        || followed_by_external_phrase(words, index)
}

fn is_inside_citation_optional_arg(line: &str, column: usize) -> bool {
    let byte_index = column_to_byte(line, column);
    let prefix = &line[..byte_index.min(line.len())];
    let Some(bracket_start) = prefix.rfind('[') else {
        return false;
    };
    let after_bracket = &line[bracket_start + 1..byte_index.min(line.len())];
    if after_bracket.contains(']') {
        return false;
    }

    let command_prefix = &line[..bracket_start];
    if let Some(command_start) = command_prefix.rfind('\\') {
        let command = command_prefix[command_start + 1..].trim_end();
        return is_citation_command(command);
    }

    command_prefix
        .split(|character: char| !character.is_ascii_alphabetic())
        .rev()
        .find(|word| !word.is_empty())
        .is_some_and(is_citation_command)
}

fn followed_by_citation_locator(line: &str, words: &[IndexedWord<'_>], index: usize) -> bool {
    let Some(preposition) = words
        .get(index + 2)
        .map(|word| word.text.to_ascii_lowercase())
    else {
        return false;
    };
    if !matches!(preposition.as_str(), "of" | "in" | "from") {
        return false;
    }

    let Some(next) = words.get(index + 3).map(|word| word.text) else {
        return false;
    };
    if is_citation_command(next) {
        return true;
    }

    let start_column = words[index + 2].column;
    let byte_start = column_to_byte(line, start_column);
    line[byte_start..].contains("\\cite")
}

fn followed_by_external_phrase(words: &[IndexedWord<'_>], index: usize) -> bool {
    let Some(preposition) = words
        .get(index + 2)
        .map(|word| word.text.to_ascii_lowercase())
    else {
        return false;
    };
    if !matches!(preposition.as_str(), "of" | "in" | "from") {
        return false;
    }

    let Some(next) = words.get(index + 3).map(|word| word.text) else {
        return false;
    };
    if next.eq_ignore_ascii_case("their") {
        return true;
    }
    next.chars().next().is_some_and(char::is_uppercase)
}

fn is_citation_command(command: &str) -> bool {
    matches!(
        command,
        "cite"
            | "citep"
            | "citet"
            | "citealp"
            | "parencite"
            | "textcite"
            | "Cite"
            | "Citep"
            | "Citet"
    )
}

fn column_to_byte(line: &str, column: usize) -> usize {
    line.char_indices()
        .nth(column.saturating_sub(1))
        .map(|(index, _)| index)
        .unwrap_or(line.len())
}

#[derive(Debug, Clone, Copy)]
struct IndexedWord<'a> {
    text: &'a str,
    column: usize,
}

fn indexed_words(line: &str) -> Vec<IndexedWord<'_>> {
    let mut words = Vec::new();
    let mut start = None;

    for (index, character) in line.char_indices() {
        if character.is_ascii_alphanumeric() || matches!(character, '.' | '-') {
            start.get_or_insert(index);
        } else if let Some(start_index) = start.take() {
            words.push(IndexedWord {
                text: trim_word(&line[start_index..index]),
                column: byte_to_column(line, start_index),
            });
        }
    }

    if let Some(start_index) = start {
        words.push(IndexedWord {
            text: trim_word(&line[start_index..]),
            column: byte_to_column(line, start_index),
        });
    }

    words
        .into_iter()
        .filter(|word| !word.text.is_empty())
        .collect()
}

fn trim_word(word: &str) -> &str {
    word.trim_matches(|character: char| matches!(character, '.' | ',' | ';' | ':' | ')' | ']'))
}

fn canonical_reference_kind(word: &str) -> Option<&'static str> {
    match word.trim_matches(|character: char| matches!(character, '(' | '[')) {
        "Figure" | "Fig." | "Fig" => Some("Figure"),
        "Table" | "Tab." | "Tab" => Some("Table"),
        "Section" | "Sec." | "Sec" => Some("Section"),
        _ => None,
    }
}

fn is_reference_number(word: &str) -> bool {
    let word = word.trim_matches(|character: char| matches!(character, '(' | '[' | ')' | ']'));
    let mut has_digit = false;
    let mut previous_was_separator = false;

    for character in word.chars() {
        if character.is_ascii_digit() {
            has_digit = true;
            previous_was_separator = false;
        } else if character == '.' {
            if previous_was_separator {
                return false;
            }
            previous_was_separator = true;
        } else {
            return false;
        }
    }

    has_digit && !previous_was_separator
}

fn byte_to_column(line: &str, byte_index: usize) -> usize {
    line[..byte_index].chars().count() + 1
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use crate::rules::Rule;

    use super::{hard_coded_references, HardCodedReferenceNumber};

    #[test]
    fn detects_hard_coded_reference_numbers() {
        let diagnostics = HardCodedReferenceNumber.check_file(
            Path::new("paper.tex"),
            "Figure 3 shows the result. Section 4.1 explains it.\n",
        );

        assert_eq!(diagnostics.len(), 2);
        assert_eq!(diagnostics[0].code, "TEX002");
        assert!(diagnostics[0].message.contains("Figure 3"));
        assert!(diagnostics[1].message.contains("Section 4.1"));
    }

    #[test]
    fn accepts_dynamic_references() {
        let diagnostics = HardCodedReferenceNumber.check_file(
            Path::new("paper.tex"),
            "Figure~\\ref{fig:result} and Section~\\cref{sec:method} show it.\n",
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn ignores_comments_math_and_command_lines() {
        let content =
            "% Figure 3\nThe value $Figure 3$ is literal.\n\\caption{Figure 3 shows it}\n";
        let diagnostics = HardCodedReferenceNumber.check_file(Path::new("paper.tex"), content);

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn ignores_external_citation_locators() {
        let content = "\\citep[Fig. 2]{paper}\nFigure 3 of \\citep{paper} is related.\nTable 1 in their paper is similar.\nTable 7 of IBMD is external.\n";
        let diagnostics = HardCodedReferenceNumber.check_file(Path::new("paper.tex"), content);

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn recognizes_common_prefixes() {
        let references = hard_coded_references("Fig. 2 and Tab. 1 summarize Sec. 3.2.");

        assert_eq!(references.len(), 3);
        assert_eq!(references[0].kind, "Figure");
        assert_eq!(references[1].kind, "Table");
        assert_eq!(references[2].kind, "Section");
    }
}
