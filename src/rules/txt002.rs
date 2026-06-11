use std::path::Path;

use crate::diagnostic::{Diagnostic, Severity};
use crate::rules::Rule;

pub struct RepeatedWords;

impl Rule for RepeatedWords {
    fn code(&self) -> &'static str {
        "TXT002"
    }

    fn name(&self) -> &'static str {
        "repeated word"
    }

    fn check_file(&self, path: &Path, content: &str) -> Vec<Diagnostic> {
        content
            .lines()
            .enumerate()
            .filter_map(|(index, line)| {
                repeated_word_column(line).map(|column| {
                    Diagnostic::new(
                        self.code(),
                        Severity::Warning,
                        self.name(),
                        path,
                        index + 1,
                        column,
                    )
                })
            })
            .collect()
    }
}

fn repeated_word_column(line: &str) -> Option<usize> {
    if line.contains('&') {
        return None;
    }

    if !line
        .trim_start()
        .starts_with(|character: char| character.is_ascii_alphabetic())
    {
        return None;
    }

    let mut previous: Option<WordToken> = None;

    for token in words_outside_math_and_comments(line) {
        let Some(word) = token else {
            previous = None;
            continue;
        };

        let normalized = word.text.to_ascii_lowercase();
        if previous
            .as_ref()
            .is_some_and(|previous| repeated_words_are_adjacent(line, previous, &word, &normalized))
        {
            return Some(word.column);
        }

        previous = Some(word);
    }

    None
}

#[derive(Debug, Clone)]
struct WordToken {
    text: String,
    start: usize,
    end: usize,
    column: usize,
}

fn repeated_words_are_adjacent(
    line: &str,
    previous: &WordToken,
    current: &WordToken,
    normalized_current: &str,
) -> bool {
    previous.text.eq_ignore_ascii_case(normalized_current)
        && line[previous.end..current.start]
            .chars()
            .all(char::is_whitespace)
        && !is_compound_word_boundary(line, previous.start, previous.end)
        && !is_compound_word_boundary(line, current.start, current.end)
}

fn is_compound_word_boundary(line: &str, start: usize, end: usize) -> bool {
    previous_char(line, start).is_some_and(is_compound_char)
        || next_char(line, end).is_some_and(is_compound_char)
}

fn previous_char(line: &str, index: usize) -> Option<char> {
    line[..index].chars().next_back()
}

fn next_char(line: &str, index: usize) -> Option<char> {
    line[index..].chars().next()
}

fn is_compound_char(character: char) -> bool {
    matches!(character, '-' | '–' | '/')
        || character.is_ascii_digit()
        || character.is_ascii_alphanumeric()
}

fn words_outside_math_and_comments(line: &str) -> Vec<Option<WordToken>> {
    let mut tokens = Vec::new();
    let mut start = None;
    let mut in_math = false;
    let mut previous = None;
    let mut index = 0;

    while index < line.len() {
        let character = line[index..]
            .chars()
            .next()
            .expect("index should be at a character boundary");
        let next_index = index + character.len_utf8();
        if character == '%' && previous != Some('\\') {
            break;
        }

        if character == '$' && previous != Some('\\') {
            if let Some(start_index) = start.take() {
                push_word_token(&mut tokens, line, start_index, index);
            }
            in_math = !in_math;
            tokens.push(None);
            previous = Some(character);
            index = next_index;
            continue;
        }

        if in_math {
            if let Some(start_index) = start.take() {
                push_word_token(&mut tokens, line, start_index, index);
            }
            previous = Some(character);
            index = next_index;
            continue;
        }

        if character == '\\' {
            if let Some(start_index) = start.take() {
                push_word_token(&mut tokens, line, start_index, index);
            }
            tokens.push(None);
            let (command, after_command) = read_command_name(line, next_index);
            index = if is_text_command(command) {
                after_command
            } else {
                skip_command_arguments(line, after_command)
            };
            previous = Some(character);
            continue;
        }

        if character.is_ascii_alphabetic() {
            start.get_or_insert(index);
        } else if let Some(start_index) = start.take() {
            push_word_token(&mut tokens, line, start_index, index);
        }

        previous = Some(character);
        index = next_index;
    }

    if let Some(start_index) = start {
        push_word_token(&mut tokens, line, start_index, line.len());
    }

    tokens
}

fn push_word_token(tokens: &mut Vec<Option<WordToken>>, line: &str, start: usize, end: usize) {
    let word = &line[start..end];
    if word.len() == 1 {
        tokens.push(None);
    } else {
        tokens.push(Some(WordToken {
            text: word.to_string(),
            start,
            end,
            column: byte_to_column(line, start),
        }));
    }
}

fn read_command_name(line: &str, mut index: usize) -> (&str, usize) {
    let start = index;
    while line
        .as_bytes()
        .get(index)
        .is_some_and(u8::is_ascii_alphabetic)
    {
        index += 1;
    }
    (&line[start..index], index)
}

fn is_text_command(command: &str) -> bool {
    matches!(
        command,
        "emph"
            | "textbf"
            | "textit"
            | "textmd"
            | "textrm"
            | "textsc"
            | "textsf"
            | "textsl"
            | "texttt"
            | "underline"
    )
}

fn skip_command_arguments(line: &str, mut index: usize) -> usize {
    loop {
        index = skip_ascii_whitespace(line, index);
        let Some(byte) = line.as_bytes().get(index) else {
            return index;
        };

        let end = match byte {
            b'[' => balanced_group_end(line, index, b'[', b']'),
            b'{' => balanced_group_end(line, index, b'{', b'}'),
            _ => return index,
        };

        let Some(end) = end else {
            return index;
        };
        index = end + 1;
    }
}

fn skip_ascii_whitespace(line: &str, mut index: usize) -> usize {
    while line
        .as_bytes()
        .get(index)
        .is_some_and(u8::is_ascii_whitespace)
    {
        index += 1;
    }
    index
}

fn balanced_group_end(line: &str, start: usize, open: u8, close: u8) -> Option<usize> {
    let mut depth = 0usize;
    let mut index = start;

    while index < line.len() {
        match line.as_bytes()[index] {
            b'\\' => {
                index = (index + 2).min(line.len());
            }
            byte if byte == open => {
                depth += 1;
                index += 1;
            }
            byte if byte == close => {
                depth -= 1;
                if depth == 0 {
                    return Some(index);
                }
                index += 1;
            }
            _ => {
                index += 1;
            }
        }
    }

    None
}

fn byte_to_column(line: &str, byte_index: usize) -> usize {
    line[..byte_index].chars().count() + 1
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use crate::rules::Rule;

    use super::RepeatedWords;

    #[test]
    fn detects_repeated_words() {
        let diagnostics =
            RepeatedWords.check_file(Path::new("paper.tex"), "This result result is useful.\n");

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, "TXT002");
        assert_eq!(diagnostics[0].line, 1);
        assert_eq!(diagnostics[0].column, 13);
    }

    #[test]
    fn ignores_case_difference_when_repeated() {
        let diagnostics = RepeatedWords.check_file(Path::new("paper.tex"), "The the result\n");

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].column, 5);
    }

    #[test]
    fn ignores_clean_text() {
        let diagnostics = RepeatedWords.check_file(Path::new("paper.tex"), "This is useful.\n");

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn ignores_words_inside_math_spans() {
        let diagnostics =
            RepeatedWords.check_file(Path::new("paper.tex"), "$x_i = x_i + 1$ is useful.\n");

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn math_spans_break_repeated_word_chain() {
        let diagnostics =
            RepeatedWords.check_file(Path::new("paper.tex"), "and $I$, $J$, and $D$ are goals.\n");

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn latex_commands_are_not_words() {
        let diagnostics = RepeatedWords.check_file(
            Path::new("paper.tex"),
            "Anna Kuzina~\\footnotemark[1] \\footnotemark[2]\\\\\n",
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn ignores_non_text_command_arguments() {
        let diagnostics = RepeatedWords.check_file(
            Path::new("paper.tex"),
            "The results are reported according to \\cite{end2end} and \\cite{hofmann}.\n",
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn detects_repeated_words_inside_text_command_arguments() {
        let diagnostics = RepeatedWords.check_file(
            Path::new("paper.tex"),
            "This is \\emph{very very} useful.\n",
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].column, 20);
    }

    #[test]
    fn punctuation_breaks_repeated_word_chain() {
        let diagnostics =
            RepeatedWords.check_file(Path::new("paper.tex"), "This is useful, useful result.\n");

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn ignores_hyphenated_compounds() {
        let diagnostics = RepeatedWords.check_file(
            Path::new("paper.tex"),
            "This is a log-log plot with item-item interactions.\n",
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn ignores_alphanumeric_identifiers() {
        let diagnostics = RepeatedWords.check_file(
            Path::new("paper.tex"),
            "We use seq2seq models and CIFAR10 CIFAR100 baselines.\n",
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn ignores_words_adjacent_to_compounds() {
        let diagnostics = RepeatedWords.check_file(
            Path::new("paper.tex"),
            "We report in in-domain and plug-in in our setup.\n",
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn ignores_table_rows() {
        let diagnostics = RepeatedWords.check_file(
            Path::new("paper.tex"),
            "Agents & Success & Success & Collisions \\\\\n",
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn ignores_comments() {
        let diagnostics = RepeatedWords.check_file(Path::new("paper.tex"), "Text % TODO the the\n");

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn ignores_command_lines() {
        let diagnostics =
            RepeatedWords.check_file(Path::new("paper.tex"), "\\begin{tabular}{c cc cc}\n");

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn ignores_single_letter_repeats() {
        let diagnostics =
            RepeatedWords.check_file(Path::new("paper.tex"), "Models include 2M, 6M, and 85M.\n");

        assert!(diagnostics.is_empty());
    }
}
