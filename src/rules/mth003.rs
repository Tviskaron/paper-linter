use std::path::Path;

use crate::diagnostic::{Diagnostic, Severity};
use crate::rules::Rule;

pub struct TextOperatorInMath;

impl Rule for TextOperatorInMath {
    fn code(&self) -> &'static str {
        "MTH003"
    }

    fn name(&self) -> &'static str {
        "text operator in math"
    }

    fn strict_only(&self) -> bool {
        true
    }

    fn check_file(&self, path: &Path, content: &str) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        let mut verbatim_depth = 0usize;
        let mut in_display_environment = false;
        let mut inline_math = None;

        for (index, raw_line) in content.lines().enumerate() {
            let line_number = index + 1;
            let line = uncommented_line(raw_line);
            let scan_line = verbatim_depth == 0;
            let next_verbatim_depth = update_verbatim_depth(line, verbatim_depth);

            if scan_line && next_verbatim_depth == 0 {
                let ranges = math_ranges(line, &mut inline_math, &mut in_display_environment);
                for (start, end) in ranges {
                    for operator in raw_operator_uses(line, start, end) {
                        diagnostics.push(
                            Diagnostic::new(
                                self.code(),
                                Severity::Warning,
                                format!(
                                    "math operator '{}' should use a LaTeX command",
                                    operator.name
                                ),
                                path,
                                line_number,
                                operator.column,
                            )
                            .with_hint(format!(
                                "use \\{} instead of raw {}",
                                operator.name, operator.name
                            )),
                        );
                    }
                }
            } else {
                in_display_environment = false;
                inline_math = None;
            }

            verbatim_depth = next_verbatim_depth;
        }

        diagnostics
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct OperatorUse<'a> {
    name: &'a str,
    column: usize,
}

fn math_ranges(
    line: &str,
    inline_math: &mut Option<InlineMathDelimiter>,
    in_display_environment: &mut bool,
) -> Vec<(usize, usize)> {
    let mut ranges = Vec::new();

    if *in_display_environment {
        if let Some(end) = math_environment_end(line) {
            ranges.push((0, end));
            *in_display_environment = false;
        } else {
            ranges.push((0, line.len()));
            return ranges;
        }
    }

    let mut index = 0;
    while index < line.len() {
        if let Some((start, end, continues)) = next_math_range(line, index, *inline_math) {
            ranges.push((start, end));
            *inline_math = continues;
            index = end.saturating_add(1);
            continue;
        }

        if let Some((start, end, continues)) = next_environment_range(line, index) {
            ranges.push((start, end));
            *in_display_environment = continues;
            index = end.saturating_add(1);
            continue;
        }

        break;
    }

    ranges
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InlineMathDelimiter {
    Dollar,
    Paren,
    Bracket,
}

impl InlineMathDelimiter {
    fn start_marker(self) -> &'static str {
        match self {
            Self::Dollar => "$",
            Self::Paren => "\\(",
            Self::Bracket => "\\[",
        }
    }

    fn end_marker(self) -> &'static str {
        match self {
            Self::Dollar => "$",
            Self::Paren => "\\)",
            Self::Bracket => "\\]",
        }
    }
}

fn next_math_range(
    line: &str,
    start_at: usize,
    active: Option<InlineMathDelimiter>,
) -> Option<(usize, usize, Option<InlineMathDelimiter>)> {
    if let Some(delimiter) = active {
        let end = find_unescaped(line, start_at, delimiter.end_marker()).unwrap_or(line.len());
        return Some((start_at, end, (end == line.len()).then_some(delimiter)));
    }

    let (delimiter_start, delimiter) = next_inline_math_start(line, start_at)?;
    let content_start = delimiter_start + delimiter.start_marker().len();
    let end = find_unescaped(line, content_start, delimiter.end_marker()).unwrap_or(line.len());
    Some((content_start, end, (end == line.len()).then_some(delimiter)))
}

fn next_inline_math_start(line: &str, start_at: usize) -> Option<(usize, InlineMathDelimiter)> {
    [
        InlineMathDelimiter::Dollar,
        InlineMathDelimiter::Paren,
        InlineMathDelimiter::Bracket,
    ]
    .into_iter()
    .filter_map(|delimiter| {
        find_unescaped(line, start_at, delimiter.start_marker()).map(|index| (index, delimiter))
    })
    .min_by_key(|(index, _)| *index)
}

fn next_environment_range(line: &str, start_at: usize) -> Option<(usize, usize, bool)> {
    let (begin_start, begin_end) = math_environment_begin(line, start_at)?;
    let content_start = begin_end;
    if let Some(end) = math_environment_end(&line[content_start..]) {
        Some((content_start, content_start + end, false))
    } else {
        Some((content_start, line.len(), begin_start < line.len()))
    }
}

fn math_environment_begin(line: &str, start_at: usize) -> Option<(usize, usize)> {
    let mut search_start = start_at;
    while let Some(relative) = line[search_start..].find("\\begin{") {
        let start = search_start + relative;
        let rest = &line[start + "\\begin{".len()..];
        let close = rest.find('}')?;
        let name = &rest[..close];
        let end = start + "\\begin{".len() + close + 1;
        if is_math_environment(name) {
            return Some((start, end));
        }
        search_start = end;
    }
    None
}

fn math_environment_end(line: &str) -> Option<usize> {
    let mut search_start = 0;
    while let Some(relative) = line[search_start..].find("\\end{") {
        let start = search_start + relative;
        let rest = &line[start + "\\end{".len()..];
        let close = rest.find('}')?;
        let name = &rest[..close];
        if is_math_environment(name) {
            return Some(start);
        }
        search_start = start + "\\end{".len() + close + 1;
    }
    None
}

fn raw_operator_uses(line: &str, start: usize, end: usize) -> Vec<OperatorUse<'_>> {
    let mut uses = Vec::new();
    let mut index = start;

    while index < end {
        if line.as_bytes()[index].is_ascii_alphabetic() {
            let token_start = index;
            while index < end && line.as_bytes()[index].is_ascii_alphabetic() {
                index += 1;
            }
            let token = &line[token_start..index];
            if is_operator(token) && !is_escaped(line, token_start) {
                uses.push(OperatorUse {
                    name: token,
                    column: byte_to_column(line, token_start),
                });
            }
            continue;
        }

        index += 1;
    }

    uses
}

fn is_operator(token: &str) -> bool {
    matches!(
        token,
        "sin" | "cos" | "tan" | "log" | "ln" | "exp" | "max" | "min" | "lim"
    )
}

fn is_math_environment(name: &str) -> bool {
    matches!(
        name,
        "equation"
            | "equation*"
            | "align"
            | "align*"
            | "gather"
            | "gather*"
            | "multline"
            | "multline*"
            | "displaymath"
            | "math"
    )
}

fn update_verbatim_depth(line: &str, mut depth: usize) -> usize {
    let mut search_start = 0;

    while let Some(relative) = line[search_start..].find('\\') {
        let index = search_start + relative;
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

fn is_verbatim_environment(name: &str) -> bool {
    matches!(
        name,
        "verbatim" | "Verbatim" | "lstlisting" | "minted" | "alltt"
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

fn find_unescaped(line: &str, start: usize, needle: &str) -> Option<usize> {
    let mut search_start = start;
    while let Some(relative) = line[search_start..].find(needle) {
        let index = search_start + relative;
        if !is_escaped(line, index) {
            return Some(index);
        }
        search_start = index + needle.len();
    }
    None
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

    use super::TextOperatorInMath;

    #[test]
    fn detects_raw_text_operators_in_inline_math() {
        let diagnostics =
            TextOperatorInMath.check_file(Path::new("paper.tex"), "$sin x + log y$\n");

        assert_eq!(diagnostics.len(), 2);
        assert_eq!(diagnostics[0].code, "MTH003");
        assert!(diagnostics[0].message.contains("sin"));
    }

    #[test]
    fn detects_raw_text_operators_in_latex_inline_math() {
        let diagnostics =
            TextOperatorInMath.check_file(Path::new("paper.tex"), "\\(sin x\\)\n\\[log y\\]\n");

        assert_eq!(diagnostics.len(), 2);
        assert_eq!(diagnostics[0].line, 1);
        assert_eq!(diagnostics[1].line, 2);
    }

    #[test]
    fn accepts_latex_operator_commands() {
        let diagnostics =
            TextOperatorInMath.check_file(Path::new("paper.tex"), "$\\sin x + \\log y + signal$\n");

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn scans_math_environments() {
        let content = "\\begin{equation}\nmax_x f(x)\n\\end{equation}\n";
        let diagnostics = TextOperatorInMath.check_file(Path::new("paper.tex"), content);

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 2);
    }

    #[test]
    fn ignores_comments_and_prose() {
        let content = "sin in prose\n% $sin x$\n\\begin{verbatim}\n$sin x$\n\\end{verbatim}\n";
        let diagnostics = TextOperatorInMath.check_file(Path::new("paper.tex"), content);

        assert!(diagnostics.is_empty());
    }
}
