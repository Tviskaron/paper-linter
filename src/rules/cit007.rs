use crate::diagnostic::{Diagnostic, Severity};
use crate::project::ProjectIndex;
use crate::rules::ProjectRule;

pub struct AdjacentCitations;

impl ProjectRule for AdjacentCitations {
    fn code(&self) -> &'static str {
        "CIT007"
    }

    fn name(&self) -> &'static str {
        "adjacent citations"
    }

    fn strict_only(&self) -> bool {
        true
    }

    fn check_project(&self, project: &ProjectIndex) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();

        for file in &project.files {
            let mut verbatim_depth = 0usize;
            for (index, raw_line) in file.content.lines().enumerate() {
                let line_number = index + 1;
                let line = uncommented_line(raw_line);
                let scan_line = verbatim_depth == 0;
                let next_verbatim_depth = update_verbatim_depth(line, verbatim_depth);

                if scan_line && next_verbatim_depth == 0 {
                    for citation in adjacent_citations(line, line_number) {
                        diagnostics.push(
                            Diagnostic::new(
                                self.code(),
                                Severity::Warning,
                                "adjacent citation commands should be merged",
                                &file.path,
                                citation.line,
                                citation.column,
                            )
                            .with_hint("combine citation keys in one citation command"),
                        );
                    }
                }

                verbatim_depth = next_verbatim_depth;
            }
        }

        diagnostics
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CitationCommand {
    line: usize,
    column: usize,
    start: usize,
    end: usize,
}

fn adjacent_citations(line: &str, line_number: usize) -> Vec<CitationCommand> {
    let citations = citation_commands(line, line_number);
    citations
        .windows(2)
        .filter_map(|window| {
            let previous = window[0];
            let current = window[1];
            separator_is_only_space_or_tie(&line[previous.end..current.start]).then_some(current)
        })
        .collect()
}

fn citation_commands(line: &str, line_number: usize) -> Vec<CitationCommand> {
    let mut citations = Vec::new();
    let mut offset = 0;

    while let Some(relative) = line[offset..].find('\\') {
        let start = offset + relative;
        let Some((name, after_name)) = read_command_name(line, start) else {
            offset = start + 1;
            continue;
        };

        if !is_citation_command(name) {
            offset = after_name;
            continue;
        }

        let Some(end) = citation_command_end(line, after_name) else {
            offset = after_name;
            continue;
        };

        citations.push(CitationCommand {
            line: line_number,
            column: byte_to_column(line, start),
            start,
            end,
        });
        offset = end;
    }

    citations
}

fn citation_command_end(line: &str, mut offset: usize) -> Option<usize> {
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

    Some(balanced_group_end(line, offset, '{', '}')? + 1)
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

    if line[command_end..].starts_with('*') {
        Some((&line[command_start..command_end], command_end + 1))
    } else {
        Some((&line[command_start..command_end], command_end))
    }
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
            | "parencite"
            | "Parencite"
            | "textcite"
            | "Textcite"
            | "autocite"
            | "Autocite"
    )
}

fn separator_is_only_space_or_tie(separator: &str) -> bool {
    !separator.is_empty()
        && separator
            .chars()
            .all(|character| character.is_ascii_whitespace() || character == '~')
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

        let Some(end) = citation_command_end(line, after_name) else {
            offset = after_name;
            continue;
        };

        let Some(body_start) = line[after_name..end]
            .find('{')
            .map(|index| after_name + index + 1)
        else {
            offset = end;
            continue;
        };
        let body_end = end - 1;

        if is_verbatim_environment(line[body_start..body_end].trim()) {
            if name == "begin" {
                depth += 1;
            } else {
                depth = depth.saturating_sub(1);
            }
        }

        offset = end;
    }

    depth
}

fn is_verbatim_environment(name: &str) -> bool {
    matches!(
        name,
        "verbatim" | "Verbatim" | "lstlisting" | "minted" | "alltt"
    )
}

fn skip_ascii_whitespace(line: &str, mut offset: usize) -> usize {
    while line[offset..]
        .chars()
        .next()
        .is_some_and(|character| character.is_ascii_whitespace())
    {
        offset += 1;
    }
    offset
}

fn balanced_group_end(line: &str, opening: usize, open: char, close: char) -> Option<usize> {
    let mut depth = 0usize;
    let mut escaped = false;

    for (relative, character) in line[opening..].char_indices() {
        if escaped {
            escaped = false;
            continue;
        }

        if character == '\\' {
            escaped = true;
            continue;
        }

        if character == open {
            depth += 1;
        } else if character == close {
            depth = depth.checked_sub(1)?;
            if depth == 0 {
                return Some(opening + relative);
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
    use super::adjacent_citations;

    #[test]
    fn detects_adjacent_citation_commands() {
        let citations = adjacent_citations(r"\cite{a} \citep[see]{b}", 1);

        assert_eq!(citations.len(), 1);
        assert_eq!(citations[0].column, 10);
    }

    #[test]
    fn allows_punctuation_between_citations() {
        let citations = adjacent_citations(r"\cite{a}; \cite{b}", 1);

        assert!(citations.is_empty());
    }

    #[test]
    fn allows_single_multikey_citation() {
        let citations = adjacent_citations(r"\cite{a,b}", 1);

        assert!(citations.is_empty());
    }
}
