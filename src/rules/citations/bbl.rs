use super::syntax::{balanced_group_end, skip_ascii_whitespace};

pub(super) fn parse_bbl_keys(content: &str) -> Vec<String> {
    let mut keys = Vec::new();
    let mut offset = 0;

    while let Some(relative) = content[offset..].find('\\') {
        let start = offset + relative;

        if content[start..].starts_with("\\bibitem") {
            let after_name = start + "\\bibitem".len();
            if command_is_continued(content, after_name) {
                offset = after_name;
                continue;
            }

            let Some((body_start, body_end)) = read_bibitem_key(content, after_name) else {
                offset = after_name;
                continue;
            };

            let key = content[body_start..body_end].trim();
            if !key.is_empty() {
                keys.push(key.to_string());
            }

            offset = body_end + 1;
            continue;
        }

        if content[start..].starts_with("\\entry") {
            let after_name = start + "\\entry".len();
            if command_is_continued(content, after_name) {
                offset = after_name;
                continue;
            }

            let Some((body_start, body_end)) = read_entry_key(content, after_name) else {
                offset = after_name;
                continue;
            };

            let key = content[body_start..body_end].trim();
            if !key.is_empty() {
                keys.push(key.to_string());
            }

            offset = body_end + 1;
            continue;
        }

        offset = start + 1;
    }

    keys
}

fn read_bibitem_key(content: &str, mut offset: usize) -> Option<(usize, usize)> {
    offset = skip_whitespace_and_comments(content, offset);

    if content[offset..].starts_with('[') {
        offset = balanced_group_end(content, offset, '[', ']')? + 1;
    }

    offset = skip_whitespace_and_comments(content, offset);
    read_required_group(content, offset)
}

fn read_entry_key(content: &str, offset: usize) -> Option<(usize, usize)> {
    let offset = skip_whitespace_and_comments(content, offset);
    read_required_group(content, offset)
}

fn read_required_group(content: &str, offset: usize) -> Option<(usize, usize)> {
    if !content[offset..].starts_with('{') {
        return None;
    }

    let end = balanced_group_end(content, offset, '{', '}')?;
    Some((offset + 1, end))
}

fn command_is_continued(content: &str, offset: usize) -> bool {
    content[offset..]
        .chars()
        .next()
        .is_some_and(|character| character.is_ascii_alphabetic())
}

fn skip_whitespace_and_comments(content: &str, mut offset: usize) -> usize {
    loop {
        offset = skip_ascii_whitespace(content, offset);
        if !content[offset..].starts_with('%') {
            return offset;
        }

        while let Some(byte) = content.as_bytes().get(offset) {
            offset += 1;
            if *byte == b'\n' {
                break;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::parse_bbl_keys;

    #[test]
    fn parses_bibitem_keys_with_optional_labels() {
        let keys = parse_bbl_keys(
            r"\begin{thebibliography}{1}
\bibitem[Li et~al.(2020)Li, Kovachki,
  Azizzadenesheli, Liu, Bhattacharya, Stuart, and Anandkumar]{li2020fourier}
Body.
\bibitem{plain-key}
\end{thebibliography}",
        );

        assert_eq!(keys, vec!["li2020fourier", "plain-key"]);
    }

    #[test]
    fn ignores_similar_command_names() {
        let keys = parse_bbl_keys(r"\bibitemextra{wrong}\bibitem{right}");

        assert_eq!(keys, vec!["right"]);
    }

    #[test]
    fn parses_bibitem_key_after_comment() {
        let keys = parse_bbl_keys(
            r"\bibitem[Krichene and Rendle(2020)]%
        {krichene2020sampled}",
        );

        assert_eq!(keys, vec!["krichene2020sampled"]);
    }

    #[test]
    fn parses_biblatex_entry_keys() {
        let keys = parse_bbl_keys(
            r"\entry{li2020fourier}{article}{}
\entry {parker2014discrete}{book}{}",
        );

        assert_eq!(keys, vec!["li2020fourier", "parker2014discrete"]);
    }
}
