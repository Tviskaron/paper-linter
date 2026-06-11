#[derive(Debug, Clone)]
pub struct ProseSpan {
    pub text: String,
    pub line: usize,
    pub start_column: usize,
}

#[derive(Debug, Clone)]
pub struct Sentence {
    pub text: String,
    pub line: usize,
    pub start_column: usize,
    pub word_count: usize,
}

pub fn prose_spans(content: &str) -> Vec<ProseSpan> {
    content
        .lines()
        .enumerate()
        .filter_map(|(index, line)| {
            if line.trim_start().starts_with('\\') || line.contains('&') {
                return None;
            }

            let prose = extract_prose_line(line);
            if prose.trim().is_empty() {
                return None;
            }

            Some(ProseSpan {
                text: prose,
                line: index + 1,
                start_column: prose_start_column(line),
            })
        })
        .collect()
}

pub fn prose_words(span: &ProseSpan) -> Vec<(String, usize)> {
    words_outside_math_and_comments(&span.text)
        .into_iter()
        .map(|(word, column)| (word.to_string(), span.start_column + column - 1))
        .collect()
}

pub fn sentences(span: &ProseSpan) -> Vec<Sentence> {
    split_sentences(&span.text)
        .into_iter()
        .map(|(text, start_column, word_count)| Sentence {
            text,
            line: span.line,
            start_column: span.start_column + start_column,
            word_count,
        })
        .collect()
}

pub fn word_count(text: &str) -> usize {
    words_outside_math_and_comments(text)
        .into_iter()
        .filter(|(word, _)| !word.is_empty())
        .count()
}

fn extract_prose_line(line: &str) -> String {
    let mut prose = String::new();
    let mut in_math = false;
    let mut previous = None;

    for character in line.chars() {
        if character == '%' && previous != Some('\\') {
            break;
        }

        if character == '$' && previous != Some('\\') {
            in_math = !in_math;
            prose.push(' ');
            previous = Some(character);
            continue;
        }

        if in_math || character == '\\' {
            if !character.is_whitespace() {
                prose.push(' ');
            }
            previous = Some(character);
            continue;
        }

        prose.push(character);
        previous = Some(character);
    }

    prose
}

fn prose_start_column(line: &str) -> usize {
    let prose = extract_prose_line(line);
    let trimmed = prose.trim_start();
    if trimmed.is_empty() {
        return 1;
    }
    let leading_spaces = prose.len() - prose.trim_start().len();
    line[..leading_spaces.min(line.len())]
        .chars()
        .count()
        + 1
}

fn split_sentences(text: &str) -> Vec<(String, usize, usize)> {
    let mut sentences = Vec::new();
    let chars: Vec<char> = text.chars().collect();
    let mut start = 0usize;

    for (index, character) in chars.iter().enumerate() {
        if matches!(character, '.' | '!' | '?') && is_sentence_boundary_chars(&chars, index) {
            let end = index + 1;
            let sentence_text: String = chars[start..end].iter().collect();
            let sentence_text = sentence_text.trim().to_string();
            if !sentence_text.is_empty() {
                let start_column = chars[..start].len() + 1;
                sentences.push((sentence_text.clone(), start_column, word_count(&sentence_text)));
            }
            start = end;
            while start < chars.len() && chars[start].is_whitespace() {
                start += 1;
            }
        }
    }

    let tail: String = chars[start..].iter().collect();
    let tail = tail.trim();
    if !tail.is_empty() {
        let start_column = chars[..start].len() + 1;
        sentences.push((tail.to_string(), start_column, word_count(tail)));
    }

    sentences
}

fn is_sentence_boundary_chars(chars: &[char], index: usize) -> bool {
    if index + 1 >= chars.len() {
        return true;
    }

    let next = chars[index + 1];
    next.is_whitespace() || next == '"' || next == '\''
}

fn words_outside_math_and_comments(line: &str) -> Vec<(&str, usize)> {
    let mut words = Vec::new();
    let mut start = None;
    let mut in_math = false;
    let mut previous = None;

    for (index, character) in line.char_indices() {
        if character == '%' && previous != Some('\\') {
            break;
        }

        if character == '$' && previous != Some('\\') {
            if let Some(start_index) = start.take() {
                words.push((&line[start_index..index], byte_to_column(line, start_index)));
            }
            in_math = !in_math;
            previous = Some(character);
            continue;
        }

        if in_math || character == '\\' {
            if let Some(start_index) = start.take() {
                words.push((&line[start_index..index], byte_to_column(line, start_index)));
            }
            previous = Some(character);
            continue;
        }

        if character.is_ascii_alphabetic() || character == '\'' {
            start.get_or_insert(index);
        } else if let Some(start_index) = start.take() {
            words.push((&line[start_index..index], byte_to_column(line, start_index)));
        }

        previous = Some(character);
    }

    if let Some(start_index) = start {
        words.push((&line[start_index..], byte_to_column(line, start_index)));
    }

    words
}

fn byte_to_column(line: &str, byte_index: usize) -> usize {
    line[..byte_index].chars().count() + 1
}

#[cfg(test)]
mod tests {
    use super::{prose_spans, sentences, word_count};

    #[test]
    fn counts_words_outside_math() {
        assert_eq!(word_count("The value $x_i$ is useful."), 4);
    }

    #[test]
    fn splits_long_sentence() {
        let spans = prose_spans("This is a long sentence with many words in it for testing.\n");
        let found = sentences(&spans[0]);
        assert_eq!(found.len(), 1);
        assert!(found[0].word_count >= 10);
    }
}
