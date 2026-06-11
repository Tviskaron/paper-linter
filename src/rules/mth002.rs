use std::path::Path;

use crate::diagnostic::{Diagnostic, Severity};
use crate::rules::Rule;

pub struct UnbracedMathScript;

impl Rule for UnbracedMathScript {
    fn code(&self) -> &'static str {
        "MTH002"
    }

    fn name(&self) -> &'static str {
        "unbraced math script"
    }

    fn strict_only(&self) -> bool {
        true
    }

    fn check_file(&self, path: &Path, content: &str) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        let mut verbatim_depth = 0usize;
        let mut math_mode: Option<MathMode> = None;

        for (index, raw_line) in content.lines().enumerate() {
            let line_number = index + 1;
            let line = uncommented_line(raw_line);
            let scan_line = verbatim_depth == 0;
            let next_verbatim_depth = update_verbatim_depth(line, verbatim_depth);

            if scan_line {
                let ranges = math_ranges(line, &mut math_mode);
                for range in ranges {
                    diagnostics.extend(
                        unbraced_script_columns(line, range.start, range.end).map(|column| {
                            Diagnostic::new(
                                self.code(),
                                Severity::Warning,
                                "math subscript or superscript has multiple characters without braces",
                                path,
                                line_number,
                                column,
                            )
                            .with_hint("wrap the script in braces, for example x^{10} or a_{ij}")
                        }),
                    );
                }
            }

            verbatim_depth = next_verbatim_depth;
        }

        diagnostics
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum MathMode {
    SingleDollar,
    DoubleDollar,
    Paren,
    Bracket,
    Environment(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ByteRange {
    start: usize,
    end: usize,
}

fn math_ranges(line: &str, math_mode: &mut Option<MathMode>) -> Vec<ByteRange> {
    let mut ranges = Vec::new();
    let mut index = 0;

    while index < line.len() {
        if let Some(mode) = math_mode.clone() {
            let end = find_math_end(line, index, &mode);
            let range_end = end.map_or(line.len(), |(end_index, _)| end_index);
            if index < range_end {
                ranges.push(ByteRange {
                    start: index,
                    end: range_end,
                });
            }

            if let Some((_, next_index)) = end {
                *math_mode = None;
                index = next_index;
            } else {
                break;
            }
            continue;
        }

        if let Some((mode, content_start)) = find_math_start(line, index) {
            *math_mode = Some(mode);
            index = content_start;
        } else {
            break;
        }
    }

    ranges
}

fn find_math_start(line: &str, start: usize) -> Option<(MathMode, usize)> {
    let mut index = start;
    let bytes = line.as_bytes();

    while index < bytes.len() {
        match bytes[index] {
            b'$' if !is_escaped(line, index) => {
                if index + 1 < bytes.len() && bytes[index + 1] == b'$' {
                    return Some((MathMode::DoubleDollar, index + 2));
                }
                return Some((MathMode::SingleDollar, index + 1));
            }
            b'\\' => {
                if line[index..].starts_with("\\(") {
                    return Some((MathMode::Paren, index + 2));
                }
                if line[index..].starts_with("\\[") {
                    return Some((MathMode::Bracket, index + 2));
                }
                if let Some((begin, name, end)) = environment_event(line, index) {
                    if begin && is_math_environment(name) {
                        return Some((MathMode::Environment(name.to_string()), end));
                    }
                    index = end;
                    continue;
                }
            }
            _ => {}
        }

        index += 1;
    }

    None
}

fn find_math_end(line: &str, start: usize, mode: &MathMode) -> Option<(usize, usize)> {
    match mode {
        MathMode::SingleDollar => find_unescaped(line, start, "$", 1),
        MathMode::DoubleDollar => find_unescaped(line, start, "$$", 2),
        MathMode::Paren => find_unescaped(line, start, "\\)", 2),
        MathMode::Bracket => find_unescaped(line, start, "\\]", 2),
        MathMode::Environment(name) => find_environment_end(line, start, name),
    }
}

fn find_unescaped(
    line: &str,
    start: usize,
    delimiter: &str,
    delimiter_len: usize,
) -> Option<(usize, usize)> {
    let mut search_start = start;

    while let Some(relative_index) = line[search_start..].find(delimiter) {
        let index = search_start + relative_index;
        if !is_escaped(line, index) {
            return Some((index, index + delimiter_len));
        }
        search_start = index + delimiter_len;
    }

    None
}

fn find_environment_end(line: &str, start: usize, expected_name: &str) -> Option<(usize, usize)> {
    let mut search_start = start;

    while let Some(relative_index) = line[search_start..].find('\\') {
        let index = search_start + relative_index;
        if let Some((begin, name, end)) = environment_event(line, index) {
            if !begin && name == expected_name {
                return Some((index, end));
            }
            search_start = end;
        } else {
            search_start = index + 1;
        }
    }

    None
}

fn unbraced_script_columns(
    line: &str,
    start: usize,
    end: usize,
) -> impl Iterator<Item = usize> + '_ {
    let mut columns = Vec::new();
    let bytes = line.as_bytes();
    let ignored_ranges = ignored_command_argument_ranges(line, start, end);
    let mut index = start;

    while index < end {
        if let Some(range) = ignored_ranges.iter().find(|range| range.contains(index)) {
            index = range.end;
            continue;
        }

        if matches!(bytes[index], b'^' | b'_') && !is_escaped(line, index) {
            let mut token_start = index + 1;
            while token_start < end && bytes[token_start].is_ascii_whitespace() {
                token_start += 1;
            }

            if token_start < end && bytes[token_start] == b'{' {
                index += 1;
                continue;
            }

            let token_len = bytes[token_start..end]
                .iter()
                .take_while(|byte| byte.is_ascii_alphanumeric())
                .count();

            if token_len > 1 {
                columns.push(byte_to_column(line, index));
                index = token_start + token_len;
                continue;
            }
        }

        index += 1;
    }

    columns.into_iter()
}

fn ignored_command_argument_ranges(line: &str, start: usize, end: usize) -> Vec<ByteRange> {
    let mut ranges = Vec::new();
    let mut index = start;

    while index < end {
        if line.as_bytes()[index] != b'\\' {
            index += 1;
            continue;
        }

        let Some((command, after_command)) = read_command_name(line, index) else {
            index += 1;
            continue;
        };
        if !is_ignored_argument_command(command) {
            index = after_command;
            continue;
        }

        let Some((arg_start, arg_end)) = required_group_range(line, after_command, end) else {
            index = after_command;
            continue;
        };
        ranges.push(ByteRange {
            start: arg_start,
            end: arg_end,
        });
        index = arg_end;
    }

    ranges
}

impl ByteRange {
    fn contains(self, index: usize) -> bool {
        self.start <= index && index < self.end
    }
}

fn read_command_name(line: &str, slash_index: usize) -> Option<(&str, usize)> {
    if line.as_bytes().get(slash_index) != Some(&b'\\') {
        return None;
    }

    let command_start = slash_index + 1;
    let mut command_end = command_start;
    while command_end < line.len() && line.as_bytes()[command_end].is_ascii_alphabetic() {
        command_end += 1;
    }
    (command_end > command_start).then(|| (&line[command_start..command_end], command_end))
}

fn required_group_range(line: &str, start: usize, limit: usize) -> Option<(usize, usize)> {
    let bytes = line.as_bytes();
    let mut index = start;
    while index < limit && bytes[index].is_ascii_whitespace() {
        index += 1;
    }
    if bytes.get(index) != Some(&b'{') {
        return None;
    }

    let mut depth = 1usize;
    let mut cursor = index + 1;
    while cursor < limit {
        match bytes[cursor] {
            b'\\' => cursor = (cursor + 2).min(limit),
            b'{' => {
                depth += 1;
                cursor += 1;
            }
            b'}' => {
                depth -= 1;
                cursor += 1;
                if depth == 0 {
                    return Some((index, cursor));
                }
            }
            _ => cursor += 1,
        }
    }

    None
}

fn is_ignored_argument_command(command: &str) -> bool {
    matches!(
        command,
        "label"
            | "ref"
            | "eqref"
            | "autoref"
            | "cref"
            | "Cref"
            | "pageref"
            | "nameref"
            | "includegraphics"
            | "input"
            | "include"
            | "url"
            | "href"
            | "cite"
            | "citep"
            | "citet"
            | "citealp"
            | "parencite"
            | "textcite"
    )
}

fn uncommented_line(line: &str) -> &str {
    let mut escaped = false;

    for (index, character) in line.char_indices() {
        if character == '%' && !escaped {
            return &line[..index];
        }

        escaped = character == '\\' && !escaped;
        if character != '\\' {
            escaped = false;
        }
    }

    line
}

fn update_verbatim_depth(line: &str, mut depth: usize) -> usize {
    let mut search_start = 0;

    while let Some(relative_index) = line[search_start..].find('\\') {
        let index = search_start + relative_index;
        if let Some((begin, name, end)) = environment_event(line, index) {
            if is_verbatim_environment(name) {
                if begin {
                    depth += 1;
                } else {
                    depth = depth.saturating_sub(1);
                }
            }
            search_start = end;
        } else {
            search_start = index + 1;
        }
    }

    depth
}

fn environment_event(line: &str, index: usize) -> Option<(bool, &str, usize)> {
    let (begin, rest) = if let Some(rest) = line[index..].strip_prefix("\\begin{") {
        (true, rest)
    } else if let Some(rest) = line[index..].strip_prefix("\\end{") {
        (false, rest)
    } else {
        return None;
    };
    let closing = rest.find('}')?;
    Some((
        begin,
        &rest[..closing],
        index + line[index..].len() - rest.len() + closing + 1,
    ))
}

fn is_math_environment(name: &str) -> bool {
    matches!(
        name,
        "equation"
            | "equation*"
            | "align"
            | "align*"
            | "alignat"
            | "alignat*"
            | "gather"
            | "gather*"
            | "multline"
            | "multline*"
            | "flalign"
            | "flalign*"
            | "displaymath"
            | "math"
            | "eqnarray"
            | "eqnarray*"
    )
}

fn is_verbatim_environment(name: &str) -> bool {
    matches!(
        name,
        "verbatim" | "Verbatim" | "lstlisting" | "minted" | "alltt"
    )
}

fn is_escaped(line: &str, byte_index: usize) -> bool {
    line.as_bytes()[..byte_index]
        .iter()
        .rev()
        .take_while(|byte| **byte == b'\\')
        .count()
        % 2
        == 1
}

fn byte_to_column(line: &str, byte_index: usize) -> usize {
    line[..byte_index].chars().count() + 1
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use crate::rules::Rule;

    use super::UnbracedMathScript;

    #[test]
    fn detects_multi_character_scripts_in_math() {
        let content = "$x^10 + a_ij$\n\\[b_23\\]\n";
        let diagnostics = UnbracedMathScript.check_file(Path::new("paper.tex"), content);

        assert_eq!(diagnostics.len(), 3);
        assert!(diagnostics
            .iter()
            .all(|diagnostic| diagnostic.code == "MTH002"));
    }

    #[test]
    fn accepts_braced_single_character_and_command_scripts() {
        let content = "$x^{10} + a_{ij} + y_i + z^2 + n_\\mathrm{max}$\n";
        let diagnostics = UnbracedMathScript.check_file(Path::new("paper.tex"), content);

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn scans_multiline_math_environments() {
        let content = "\\begin{equation}\nx^10 + y_i\n\\end{equation}\n";
        let diagnostics = UnbracedMathScript.check_file(Path::new("paper.tex"), content);

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 2);
    }

    #[test]
    fn ignores_comments_verbatim_and_prose() {
        let content = "x^10 in prose\n% $x^10$\n\\begin{verbatim}\n$x^10$\n\\end{verbatim}\n";
        let diagnostics = UnbracedMathScript.check_file(Path::new("paper.tex"), content);

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn ignores_non_math_command_arguments_inside_math() {
        let content = "\\begin{equation}\n\\label{eq:main_task} \\ref{alg:local_sgd} \\includegraphics{pics/ipmf_diagramm_new.png} + x^10\n\\end{equation}\n";
        let diagnostics = UnbracedMathScript.check_file(Path::new("paper.tex"), content);

        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0].message.contains("multiple characters"));
    }
}
