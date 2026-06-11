use std::path::Path;

use super::syntax::{balanced_group_end, skip_ascii_whitespace};
use super::{BibliographyDecl, CitationUse};

pub(super) fn find_citations(path: &Path, content: &str) -> Vec<CitationUse> {
    let mut citations = Vec::new();

    for (line_index, raw_line) in content.lines().enumerate() {
        let line = uncommented_line(raw_line);
        let mut offset = 0;

        while let Some(relative) = line[offset..].find('\\') {
            let start = offset + relative;
            let Some((name, after_name)) = read_command_name(line, start) else {
                offset = start + 1;
                continue;
            };

            let is_nocite = name.eq_ignore_ascii_case("nocite");
            if !is_citation_command(name) && !is_nocite {
                offset = after_name;
                continue;
            }

            let Some((body_start, body_end)) = read_command_argument(line, after_name) else {
                offset = after_name;
                continue;
            };

            for (key, key_column_offset) in split_keys(&line[body_start..body_end]) {
                citations.push(CitationUse {
                    key,
                    file: path.to_path_buf(),
                    line: line_index + 1,
                    column: char_column(line, body_start + key_column_offset),
                    is_nocite,
                });
            }

            offset = body_end + 1;
        }
    }

    citations
}

pub(super) fn find_bibliographies(path: &Path, content: &str) -> Vec<BibliographyDecl> {
    let mut declarations = Vec::new();

    for (line_index, raw_line) in content.lines().enumerate() {
        let line = uncommented_line(raw_line);
        let mut offset = 0;

        while let Some(relative) = line[offset..].find('\\') {
            let start = offset + relative;
            let Some((name, after_name)) = read_command_name(line, start) else {
                offset = start + 1;
                continue;
            };

            if name != "bibliography" && name != "addbibresource" {
                offset = after_name;
                continue;
            }

            let Some((body_start, body_end)) = read_command_argument(line, after_name) else {
                offset = after_name;
                continue;
            };

            for (bib_path, column_offset) in split_keys(&line[body_start..body_end]) {
                declarations.push(BibliographyDecl {
                    path: bib_path,
                    file: path.to_path_buf(),
                    line: line_index + 1,
                    column: char_column(line, body_start + column_offset),
                });
            }

            offset = body_end + 1;
        }
    }

    declarations
}

fn read_command_name(line: &str, slash_index: usize) -> Option<(&str, usize)> {
    let command_start = slash_index + 1;
    let mut command_end = command_start;

    for (index, character) in line[command_start..].char_indices() {
        if character.is_ascii_alphabetic() {
            command_end = command_start + index + character.len_utf8();
        } else {
            break;
        }
    }

    if command_end == command_start {
        return None;
    }

    let name = &line[command_start..command_end];
    if line[command_end..].starts_with('*') {
        Some((name, command_end + 1))
    } else {
        Some((name, command_end))
    }
}

fn read_command_argument(line: &str, mut offset: usize) -> Option<(usize, usize)> {
    loop {
        offset = skip_ascii_whitespace(line, offset);
        if !line[offset..].starts_with('[') {
            break;
        }

        offset = balanced_group_end(line, offset, '[', ']')? + 1;
    }

    offset = skip_ascii_whitespace(line, offset);
    if !line[offset..].starts_with('{') {
        return None;
    }

    let end = balanced_group_end(line, offset, '{', '}')?;
    Some((offset + 1, end))
}

fn is_citation_command(name: &str) -> bool {
    matches!(
        name,
        "cite"
            | "Cite"
            | "citep"
            | "Citep"
            | "citet"
            | "Citet"
            | "citealp"
            | "Citealp"
            | "citealt"
            | "Citealt"
            | "citeauthor"
            | "Citeauthor"
            | "citeyear"
            | "Citeyear"
            | "citeyearpar"
            | "Citeyearpar"
            | "parencite"
            | "Parencite"
            | "textcite"
            | "Textcite"
            | "autocite"
            | "Autocite"
            | "footcite"
            | "Footcite"
            | "supercite"
            | "Supercite"
    )
}

fn split_keys(body: &str) -> Vec<(String, usize)> {
    let mut keys = Vec::new();
    let mut start = 0;

    for (index, character) in body.char_indices() {
        if character == ',' {
            push_key(body, start, index, &mut keys);
            start = index + character.len_utf8();
        }
    }

    push_key(body, start, body.len(), &mut keys);
    keys
}

fn push_key(body: &str, start: usize, end: usize, keys: &mut Vec<(String, usize)>) {
    let raw = &body[start..end];
    let left_trimmed = raw.trim_start();
    let key = left_trimmed.trim_end();
    if key.is_empty() {
        return;
    }

    let column_offset = start + raw.len() - left_trimmed.len();
    keys.push((key.to_string(), column_offset));
}

fn uncommented_line(line: &str) -> &str {
    let mut escaped = false;

    for (index, character) in line.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }

        if character == '\\' {
            escaped = true;
            continue;
        }

        if character == '%' {
            return &line[..index];
        }
    }

    line
}

fn char_column(line: &str, byte_index: usize) -> usize {
    line[..byte_index].chars().count() + 1
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{find_bibliographies, find_citations};
    use crate::rules::citations::BibliographyDecl;

    #[test]
    fn finds_citations_with_optional_args_and_multiple_keys() {
        let citations = find_citations(
            Path::new("paper.tex"),
            r"\citep[see][p. 3]{alpha, beta} and \citet*{gamma}",
        );

        let keys: Vec<_> = citations
            .iter()
            .map(|citation| citation.key.as_str())
            .collect();
        assert_eq!(keys, vec!["alpha", "beta", "gamma"]);
        assert_eq!(citations[0].column, 19);
    }

    #[test]
    fn ignores_commented_citations_but_not_escaped_percent() {
        let citations = find_citations(
            Path::new("paper.tex"),
            r"shown as 5\% in \cite{real} % \cite{commented}",
        );

        assert_eq!(citations.len(), 1);
        assert_eq!(citations[0].key, "real");
    }

    #[test]
    fn marks_nocite_keys() {
        let citations = find_citations(Path::new("paper.tex"), r"\nocite{*}");

        assert_eq!(citations.len(), 1);
        assert!(citations[0].is_nocite);
        assert_eq!(citations[0].key, "*");
    }

    #[test]
    fn finds_bibliography_declarations() {
        let declarations = find_bibliographies(
            Path::new("paper.tex"),
            r"\bibliography{refs, more} \addbibresource{extra.bib}",
        );

        let paths: Vec<_> = declarations
            .iter()
            .map(|declaration: &BibliographyDecl| declaration.path.as_str())
            .collect();
        assert_eq!(paths, vec!["refs", "more", "extra.bib"]);
    }
}
