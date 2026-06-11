use std::path::Path;

use super::syntax::{balanced_group_end, skip_ascii_whitespace};
use super::{BibliographyDecl, CitationUse};

pub(super) fn find_citations(path: &Path, content: &str) -> Vec<CitationUse> {
    let mut citations = Vec::new();
    let mut verbatim_depth = 0usize;

    for (line_index, raw_line) in content.lines().enumerate() {
        let line = uncommented_line(raw_line);
        let scan_line = verbatim_depth == 0;
        let next_verbatim_depth = update_verbatim_depth(line, verbatim_depth);
        if !scan_line {
            verbatim_depth = next_verbatim_depth;
            continue;
        }

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
                if contains_macro_parameter(&key) {
                    continue;
                }

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

        verbatim_depth = next_verbatim_depth;
    }

    citations
}

pub(super) fn find_bibliographies(path: &Path, content: &str) -> Vec<BibliographyDecl> {
    let mut declarations = Vec::new();
    let line_starts = line_starts(content);
    let mut offset = 0;

    while let Some(relative) = content[offset..].find('\\') {
        let start = offset + relative;
        if is_commented_position(content, start) {
            offset = start + 1;
            continue;
        }

        let Some((name, after_name)) = read_command_name(content, start) else {
            offset = start + 1;
            continue;
        };

        if name != "bibliography" && name != "addbibresource" {
            offset = after_name;
            continue;
        }

        let Some((body_start, body_end)) = read_command_argument(content, after_name) else {
            offset = after_name;
            continue;
        };

        for (bib_path, column_offset) in split_keys(&content[body_start..body_end]) {
            let (line, column) = line_column(&line_starts, content, body_start + column_offset);
            declarations.push(BibliographyDecl {
                path: bib_path,
                file: path.to_path_buf(),
                line,
                column,
            });
        }

        offset = body_end + 1;
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

fn contains_macro_parameter(key: &str) -> bool {
    let mut chars = key.chars();
    while let Some(character) = chars.next() {
        if character == '#' && chars.next().is_some_and(|next| next.is_ascii_digit()) {
            return true;
        }
    }
    false
}

fn update_verbatim_depth(line: &str, mut depth: usize) -> usize {
    let mut offset = 0;
    while let Some(relative) = line[offset..].find('\\') {
        let start = offset + relative;
        let Some((name, after_name)) = read_command_name(line, start) else {
            offset = start + 1;
            continue;
        };

        if name != "begin" && name != "end" {
            offset = after_name;
            continue;
        }

        let Some((body_start, body_end)) = read_command_argument(line, after_name) else {
            offset = after_name;
            continue;
        };

        if is_verbatim_environment(line[body_start..body_end].trim()) {
            if name == "begin" {
                depth += 1;
            } else {
                depth = depth.saturating_sub(1);
            }
        }

        offset = body_end + 1;
    }

    depth
}

fn is_verbatim_environment(name: &str) -> bool {
    matches!(
        name,
        "verbatim" | "Verbatim" | "lstlisting" | "minted" | "alltt"
    )
}

fn line_starts(content: &str) -> Vec<usize> {
    let mut starts = vec![0];
    for (index, character) in content.char_indices() {
        if character == '\n' {
            starts.push(index + 1);
        }
    }
    starts
}

fn line_column(line_starts: &[usize], content: &str, byte_index: usize) -> (usize, usize) {
    let line_index = line_starts.partition_point(|start| *start <= byte_index) - 1;
    let line_start = line_starts[line_index];
    let column = content[line_start..byte_index].chars().count() + 1;
    (line_index + 1, column)
}

fn is_commented_position(content: &str, byte_index: usize) -> bool {
    let line_start = content[..byte_index]
        .rfind('\n')
        .map(|index| index + 1)
        .unwrap_or(0);
    uncommented_line(&content[line_start..byte_index]).len() < byte_index - line_start
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
    fn ignores_macro_parameter_citation_keys() {
        let citations = find_citations(
            Path::new("paper.tex"),
            r"\newcommand{\mycite}[1]{\cite{#1}} \cite{real}",
        );

        assert_eq!(citations.len(), 1);
        assert_eq!(citations[0].key, "real");
    }

    #[test]
    fn ignores_citations_inside_verbatim_environments() {
        let citations = find_citations(
            Path::new("paper.tex"),
            "\\begin{verbatim}\n\\cite{example}\n\\end{verbatim}\n\\cite{real}",
        );

        assert_eq!(citations.len(), 1);
        assert_eq!(citations[0].key, "real");
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

    #[test]
    fn finds_multiline_bibliography_declarations() {
        let declarations = find_bibliographies(
            Path::new("paper.tex"),
            "\\addbibresource{refs.bib\n}\n% \\bibliography{commented}\n",
        );

        assert_eq!(declarations.len(), 1);
        assert_eq!(declarations[0].path, "refs.bib");
        assert_eq!(declarations[0].line, 1);
        assert_eq!(declarations[0].column, 17);
    }
}
