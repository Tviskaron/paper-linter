use std::path::Path;

use super::syntax::{balanced_group_end, skip_ascii_whitespace};
use super::{BibliographyDecl, CitationKind, CitationUse};

#[derive(Debug, Clone, Copy)]
struct ParsedCommand<'a> {
    name: &'a str,
    after_name: usize,
    is_starred: bool,
}

#[derive(Debug, Clone)]
struct CommandArguments {
    required: Vec<(usize, usize)>,
    has_optional: bool,
    end: usize,
}

pub(super) fn find_citations(path: &Path, content: &str) -> Vec<CitationUse> {
    let mut citations = Vec::new();
    let mut verbatim_depth = 0usize;
    let mut line_start_offset = 0usize;

    for (line_index, raw_line) in content.lines().enumerate() {
        let line = uncommented_line(raw_line);
        let scan_line = verbatim_depth == 0;
        let next_verbatim_depth = update_verbatim_depth(line, verbatim_depth);
        if !scan_line {
            verbatim_depth = next_verbatim_depth;
            line_start_offset += raw_line.len() + 1;
            continue;
        }

        let mut offset = 0;

        while let Some(relative) = line[offset..].find('\\') {
            let start = offset + relative;
            let Some(command) = read_command_name(line, start) else {
                offset = start + 1;
                continue;
            };

            let Some(kind) = citation_kind(command.name) else {
                offset = command.after_name;
                continue;
            };
            let is_nocite = kind == CitationKind::NoCite;

            let Some(arguments) = read_command_arguments(line, command.after_name) else {
                offset = command.after_name;
                continue;
            };
            let key_groups = citation_key_groups(command.name, &arguments);
            let command_end = citation_command_end(command.name, &arguments);

            for (body_start, body_end) in key_groups {
                for (key, key_column_offset) in split_keys(&line[body_start..body_end]) {
                    if contains_macro_parameter(&key) {
                        continue;
                    }

                    citations.push(CitationUse {
                        key,
                        command: command.name.to_string(),
                        kind,
                        file: path.to_path_buf(),
                        line: line_index + 1,
                        column: char_column(line, body_start + key_column_offset),
                        is_nocite,
                        is_starred: command.is_starred,
                        has_optional_arg: arguments.has_optional,
                        command_start: line_start_offset + start,
                        command_end: line_start_offset + command_end,
                        argument_start: line_start_offset + body_start,
                        argument_end: line_start_offset + body_end,
                    });
                }
            }

            offset = command_end;
        }

        verbatim_depth = next_verbatim_depth;
        line_start_offset += raw_line.len() + 1;
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

        let Some(command) = read_command_name(content, start) else {
            offset = start + 1;
            continue;
        };

        if command.name != "bibliography" && command.name != "addbibresource" {
            offset = command.after_name;
            continue;
        }

        let Some(arguments) = read_command_arguments(content, command.after_name) else {
            offset = command.after_name;
            continue;
        };
        let Some((body_start, body_end)) = arguments.required.first().copied() else {
            offset = arguments.end;
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

        offset = arguments.end;
    }

    declarations
}

fn read_command_name(line: &str, slash_index: usize) -> Option<ParsedCommand<'_>> {
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
    let is_starred = line[command_end..].starts_with('*');
    let after_name = command_end + usize::from(is_starred);
    Some(ParsedCommand {
        name,
        after_name,
        is_starred,
    })
}

fn read_command_arguments(line: &str, mut offset: usize) -> Option<CommandArguments> {
    let mut required = Vec::new();
    let mut has_optional = false;

    loop {
        offset = skip_ascii_whitespace(line, offset);
        if line[offset..].starts_with('[') {
            has_optional = true;
            offset = balanced_group_end(line, offset, '[', ']')? + 1;
            continue;
        }

        if line[offset..].starts_with('{') {
            let end = balanced_group_end(line, offset, '{', '}')?;
            required.push((offset + 1, end));
            offset = end + 1;
            continue;
        }

        break;
    }

    if required.is_empty() {
        None
    } else {
        Some(CommandArguments {
            required,
            has_optional,
            end: offset,
        })
    }
}

fn citation_key_groups(name: &str, arguments: &CommandArguments) -> Vec<(usize, usize)> {
    if is_volume_citation_command(name) {
        arguments.required.last().copied().into_iter().collect()
    } else if is_multi_citation_command(name) {
        arguments.required.clone()
    } else {
        arguments.required.first().copied().into_iter().collect()
    }
}

fn citation_command_end(name: &str, arguments: &CommandArguments) -> usize {
    if is_volume_citation_command(name) || is_multi_citation_command(name) {
        arguments.end
    } else {
        arguments
            .required
            .first()
            .map(|(_, end)| end + 1)
            .unwrap_or(arguments.end)
    }
}

fn citation_kind(name: &str) -> Option<CitationKind> {
    if name == "nocite" {
        return Some(CitationKind::NoCite);
    }
    if is_textual_citation_command(name) {
        return Some(CitationKind::Textual);
    }
    if is_author_year_citation_command(name) {
        return Some(CitationKind::AuthorYearOnly);
    }
    if is_parenthetical_citation_command(name) {
        return Some(CitationKind::Parenthetical);
    }
    if is_neutral_citation_command(name) {
        return Some(CitationKind::Neutral);
    }
    None
}

fn is_parenthetical_citation_command(name: &str) -> bool {
    matches!(
        name,
        "citep"
            | "Citep"
            | "parencite"
            | "Parencite"
            | "parencites"
            | "Parencites"
            | "autocite"
            | "Autocite"
            | "autocites"
            | "Autocites"
            | "smartcite"
            | "Smartcite"
            | "smartcites"
            | "Smartcites"
            | "footcite"
            | "Footcite"
            | "footcites"
            | "supercite"
            | "Supercite"
            | "supercites"
            | "Pnotecite"
            | "pnotecite"
    )
}

fn is_textual_citation_command(name: &str) -> bool {
    matches!(
        name,
        "citet"
            | "Citet"
            | "textcite"
            | "Textcite"
            | "textcites"
            | "Textcites"
            | "citealt"
            | "Citealt"
            | "citealp"
            | "Citealp"
    )
}

fn is_author_year_citation_command(name: &str) -> bool {
    matches!(
        name,
        "citeauthor"
            | "Citeauthor"
            | "citeyear"
            | "Citeyear"
            | "citeyearpar"
            | "Citeyearpar"
            | "citenum"
            | "citetitle"
            | "Citetitle"
            | "citedate"
            | "Citedate"
            | "citeurl"
    )
}

fn is_neutral_citation_command(name: &str) -> bool {
    matches!(
        name,
        "cite"
            | "Cite"
            | "cites"
            | "Cites"
            | "notecite"
            | "Notecite"
            | "fnotecite"
            | "Fnotecite"
            | "fullcite"
            | "Fullcite"
            | "footfullcite"
            | "footcitetext"
            | "footcitetexts"
            | "Avolcite"
            | "Avolcites"
            | "avolcite"
            | "avolcites"
            | "Ftvolcite"
            | "Ftvolcites"
            | "ftvolcite"
            | "ftvolcites"
            | "Fvolcite"
            | "Fvolcites"
            | "fvolcite"
            | "fvolcites"
            | "Pvolcite"
            | "Pvolcites"
            | "pvolcite"
            | "pvolcites"
            | "Svolcite"
            | "Svolcites"
            | "svolcite"
            | "svolcites"
            | "Tvolcite"
            | "Tvolcites"
            | "tvolcite"
            | "tvolcites"
            | "Volcite"
            | "Volcites"
            | "volcite"
            | "volcites"
    )
}

fn is_volume_citation_command(name: &str) -> bool {
    matches!(
        name,
        "Avolcite"
            | "Avolcites"
            | "avolcite"
            | "avolcites"
            | "Ftvolcite"
            | "Ftvolcites"
            | "ftvolcite"
            | "ftvolcites"
            | "Fvolcite"
            | "Fvolcites"
            | "fvolcite"
            | "fvolcites"
            | "Pvolcite"
            | "Pvolcites"
            | "pvolcite"
            | "pvolcites"
            | "Svolcite"
            | "Svolcites"
            | "svolcite"
            | "svolcites"
            | "Tvolcite"
            | "Tvolcites"
            | "tvolcite"
            | "tvolcites"
            | "Volcite"
            | "Volcites"
            | "volcite"
            | "volcites"
    )
}

fn is_multi_citation_command(name: &str) -> bool {
    matches!(
        name,
        "cites"
            | "Cites"
            | "parencites"
            | "Parencites"
            | "textcites"
            | "Textcites"
            | "autocites"
            | "Autocites"
            | "smartcites"
            | "Smartcites"
            | "footcites"
            | "supercites"
    ) || name.ends_with("cites")
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
        let Some(command) = read_command_name(line, start) else {
            offset = start + 1;
            continue;
        };

        if command.name != "begin" && command.name != "end" {
            offset = command.after_name;
            continue;
        }

        let Some(arguments) = read_command_arguments(line, command.after_name) else {
            offset = command.after_name;
            continue;
        };
        let Some((body_start, body_end)) = arguments.required.first().copied() else {
            offset = arguments.end;
            continue;
        };

        if is_verbatim_environment(line[body_start..body_end].trim()) {
            if command.name == "begin" {
                depth += 1;
            } else {
                depth = depth.saturating_sub(1);
            }
        }

        offset = arguments.end;
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
        assert_eq!(citations[0].command, "citep");
        assert!(citations[0].has_optional_arg);
        assert!(!citations[0].is_starred);
        assert_eq!(citations[2].command, "citet");
        assert!(citations[2].is_starred);
    }

    #[test]
    fn finds_biblatex_plural_and_volume_citations() {
        let citations = find_citations(
            Path::new("paper.tex"),
            r"\Cites{alpha}{beta} \parencites{gamma}{delta} \volcite{2}[45]{epsilon} \citeurl{zeta}",
        );

        let keys: Vec<_> = citations
            .iter()
            .map(|citation| citation.key.as_str())
            .collect();
        assert_eq!(
            keys,
            vec!["alpha", "beta", "gamma", "delta", "epsilon", "zeta"]
        );
        assert_eq!(citations[0].command, "Cites");
        assert_eq!(citations[4].command, "volcite");
        assert!(citations[4].has_optional_arg);
        assert_eq!(citations[4].argument_start, 62);
        assert_eq!(citations[4].argument_end, 69);
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
    fn does_not_treat_following_braced_text_as_citation_keys() {
        let citations = find_citations(
            Path::new("paper.tex"),
            r"\citep{wagner2021improving} {propose DIPOLE, an approach.}",
        );

        assert_eq!(citations.len(), 1);
        assert_eq!(citations[0].key, "wagner2021improving");
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
