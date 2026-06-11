#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EnvironmentEventKind {
    Begin,
    End,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnvironmentEvent {
    pub kind: EnvironmentEventKind,
    pub name: String,
    pub line: usize,
    pub column: usize,
}

pub fn environment_events(content: &str) -> Vec<EnvironmentEvent> {
    let mut events = Vec::new();
    let mut definition_brace_depth = 0i32;
    let mut skip_until_document = false;

    for (line_index, line) in content.lines().enumerate() {
        let line_number = line_index + 1;
        let line = uncommented_line(line);

        if skip_until_document {
            if line.contains("\\begin{document}") {
                skip_until_document = false;
            }
            continue;
        }

        if definition_brace_depth > 0 {
            definition_brace_depth += brace_delta(line);
            definition_brace_depth = definition_brace_depth.max(0);
            continue;
        }

        if let Some((definition_start, command)) = definition_start(line) {
            events.extend(environment_events_in_line(
                &line[..definition_start],
                line_number,
            ));

            if let Some(definition_end) = simple_definition_end(line, definition_start, command) {
                events.extend(environment_events_in_line(
                    &line[definition_end..],
                    line_number,
                ));
                continue;
            }

            if is_environment_definition_command(command) {
                skip_until_document = true;
                continue;
            }

            definition_brace_depth = brace_delta(&line[definition_start..]).max(0);
            continue;
        }

        events.extend(environment_events_in_line(line, line_number));
    }

    events
}

fn environment_events_in_line(line: &str, line_number: usize) -> Vec<EnvironmentEvent> {
    let mut events = Vec::new();
    let mut search_start = 0;

    while search_start < line.len() {
        let Some(relative_index) = line[search_start..].find('\\') else {
            break;
        };
        let index = search_start + relative_index;

        if let Some(end) = inline_verb_end(line, index) {
            search_start = end;
            continue;
        }

        if line[..index].ends_with("\\string") {
            search_start = index + 1;
            continue;
        }

        if let Some((kind, name, end)) = parse_environment_event(line, index) {
            events.push(EnvironmentEvent {
                kind,
                name,
                line: line_number,
                column: byte_to_column(line, index),
            });
            search_start = end;
        } else {
            search_start = index + 1;
        }
    }

    events
}

fn definition_start(line: &str) -> Option<(usize, &'static str)> {
    let commands = [
        "\\def",
        "\\gdef",
        "\\edef",
        "\\xdef",
        "\\newcommand",
        "\\renewcommand",
        "\\providecommand",
        "\\DeclareRobustCommand",
        "\\newenvironment",
        "\\renewenvironment",
    ];

    commands
        .iter()
        .filter_map(|command| command_start(line, command).map(|index| (index, *command)))
        .min()
}

fn command_start(line: &str, command: &str) -> Option<usize> {
    let mut search_start = 0;

    while let Some(relative_index) = line[search_start..].find(command) {
        let index = search_start + relative_index;
        let after_command = index + command.len();

        if line[after_command..]
            .chars()
            .next()
            .is_none_or(|ch| !ch.is_ascii_alphabetic())
        {
            return Some(index);
        }

        search_start = after_command;
    }

    None
}

fn simple_definition_end(line: &str, index: usize, command: &str) -> Option<usize> {
    if is_tex_definition_command(command) {
        let body_start = line[index + command.len()..].find('{')? + index + command.len();
        return matching_closing_brace(line, body_start);
    }

    if is_command_definition_command(command) {
        return command_definition_end(line, index, command);
    }

    None
}

fn command_definition_end(line: &str, index: usize, command: &str) -> Option<usize> {
    let mut cursor = index + command.len();
    cursor = required_arg_end(line, cursor)?;

    loop {
        let next = skip_ws(line, cursor);
        if !line[next..].starts_with('[') {
            cursor = next;
            break;
        }
        cursor = matching_delimiter(line, next, '[', ']')?;
    }

    required_arg_end(line, cursor)
}

fn required_arg_end(line: &str, index: usize) -> Option<usize> {
    let open = skip_ws(line, index);
    if !line[open..].starts_with('{') {
        return None;
    }
    matching_closing_brace(line, open)
}

fn skip_ws(line: &str, mut index: usize) -> usize {
    while line
        .as_bytes()
        .get(index)
        .is_some_and(|byte| byte.is_ascii_whitespace())
    {
        index += 1;
    }
    index
}

fn matching_closing_brace(line: &str, opening_brace: usize) -> Option<usize> {
    matching_delimiter(line, opening_brace, '{', '}')
}

fn matching_delimiter(
    line: &str,
    opening_delimiter: usize,
    open: char,
    close: char,
) -> Option<usize> {
    let mut depth = 0i32;
    let mut escaped = false;

    for (index, ch) in line[opening_delimiter..].char_indices() {
        if !escaped {
            if ch == open {
                depth += 1;
            } else if ch == close {
                depth -= 1;
                if depth == 0 {
                    return Some(opening_delimiter + index + ch.len_utf8());
                }
            }
        }

        escaped = ch == '\\' && !escaped;
        if ch != '\\' {
            escaped = false;
        }
    }

    None
}

fn is_tex_definition_command(command: &str) -> bool {
    matches!(command, "\\def" | "\\gdef" | "\\edef" | "\\xdef")
}

fn is_command_definition_command(command: &str) -> bool {
    matches!(
        command,
        "\\newcommand" | "\\renewcommand" | "\\providecommand" | "\\DeclareRobustCommand"
    )
}

fn is_environment_definition_command(command: &str) -> bool {
    matches!(command, "\\newenvironment" | "\\renewenvironment")
}

fn brace_delta(line: &str) -> i32 {
    let mut delta = 0;
    let mut escaped = false;

    for ch in line.chars() {
        if !escaped {
            match ch {
                '{' => delta += 1,
                '}' => delta -= 1,
                _ => {}
            }
        }

        escaped = ch == '\\' && !escaped;
        if ch != '\\' {
            escaped = false;
        }
    }

    delta
}

fn uncommented_line(line: &str) -> &str {
    let mut escaped = false;

    for (index, ch) in line.char_indices() {
        if ch == '%' && !escaped {
            return &line[..index];
        }

        escaped = ch == '\\' && !escaped;
        if ch != '\\' {
            escaped = false;
        }
    }

    line
}

fn inline_verb_end(line: &str, index: usize) -> Option<usize> {
    let mut rest = line[index..].strip_prefix("\\verb")?;
    if rest
        .chars()
        .next()
        .is_some_and(|ch| ch.is_ascii_alphabetic())
    {
        return None;
    }

    if let Some(after_star) = rest.strip_prefix('*') {
        rest = after_star;
    }

    let delimiter = rest.chars().next()?;
    if delimiter.is_ascii_whitespace() || delimiter == '{' || delimiter == '}' {
        return None;
    }

    let content_start = index + line[index..].len() - rest.len() + delimiter.len_utf8();
    let relative_end = line[content_start..].find(delimiter)?;

    Some(content_start + relative_end + delimiter.len_utf8())
}

fn parse_environment_event(
    line: &str,
    index: usize,
) -> Option<(EnvironmentEventKind, String, usize)> {
    let rest = &line[index..];
    let (kind, rest) = if let Some(rest) = rest.strip_prefix("\\begin") {
        if rest
            .chars()
            .next()
            .is_some_and(|ch| ch.is_ascii_alphabetic())
        {
            return None;
        }
        (EnvironmentEventKind::Begin, rest)
    } else if let Some(rest) = rest.strip_prefix("\\end") {
        if rest
            .chars()
            .next()
            .is_some_and(|ch| ch.is_ascii_alphabetic())
        {
            return None;
        }
        (EnvironmentEventKind::End, rest)
    } else {
        return None;
    };

    let rest = rest.trim_start();
    let name_start = line.len() - rest.len();
    let rest = rest.strip_prefix('{')?;
    let name_start = name_start + 1;
    let name_end = name_start + rest.find('}')?;

    Some((kind, line[name_start..name_end].to_string(), name_end + 1))
}

fn byte_to_column(line: &str, byte_index: usize) -> usize {
    line[..byte_index].chars().count() + 1
}

#[cfg(test)]
mod tests {
    use super::{environment_events, EnvironmentEventKind};

    #[test]
    fn finds_begin_and_end_events() {
        let events = environment_events("\\begin{figure}\n\\end{figure}\n");

        assert_eq!(events.len(), 2);
        assert_eq!(events[0].kind, EnvironmentEventKind::Begin);
        assert_eq!(events[0].name, "figure");
        assert_eq!(events[1].kind, EnvironmentEventKind::End);
    }

    #[test]
    fn ignores_comment_events() {
        let events = environment_events("% \\begin{figure}\nText\n");

        assert!(events.is_empty());
    }

    #[test]
    fn keeps_events_after_escaped_percent() {
        let events = environment_events("Accuracy is 90\\% \\begin{figure}\n\\end{figure}\n");

        assert_eq!(events.len(), 2);
        assert_eq!(events[0].name, "figure");
    }

    #[test]
    fn ignores_inline_verb_events() {
        let events = environment_events("\\verb|\\end{document}| \\begin{figure}\n");

        assert_eq!(events.len(), 1);
        assert_eq!(events[0].kind, EnvironmentEventKind::Begin);
        assert_eq!(events[0].name, "figure");
    }

    #[test]
    fn ignores_stringified_events() {
        let events = environment_events("\\typeout{after \\string\\begin{document}}\n");

        assert!(events.is_empty());
    }

    #[test]
    fn ignores_environment_events_inside_macro_definitions() {
        let events = environment_events(
            "\\def\\And{\\end{tabular}\n\\hbox{\\begin{tabular}{c}}}\n\\begin{figure}\n",
        );

        assert_eq!(events.len(), 1);
        assert_eq!(events[0].name, "figure");
    }

    #[test]
    fn keeps_events_after_inline_simple_definition() {
        let events = environment_events(
            "\\begin{figure}\n\\def\\arraystretch{1.2}\\begin{tabular}{c}\n\\end{tabular}\n\\end{figure}\n",
        );

        assert_eq!(events.len(), 4);
        assert_eq!(events[1].name, "tabular");
    }

    #[test]
    fn keeps_events_after_inline_newcommand_definition() {
        let events = environment_events(
            "\\newcommand{\\Call}[2]{\\textsc{#1}(#2)} \\begin{algorithm}[H]\n\\begin{algorithmic}\n\\end{algorithmic}\n\\end{algorithm}\n",
        );

        assert_eq!(events.len(), 4);
        assert_eq!(events[0].kind, EnvironmentEventKind::Begin);
        assert_eq!(events[0].name, "algorithm");
        assert_eq!(events[3].kind, EnvironmentEventKind::End);
        assert_eq!(events[3].name, "algorithm");
    }

    #[test]
    fn ignores_multiline_environment_definitions_until_document() {
        let events = environment_events(
            "\\newenvironment{testpage}{\n\\begin{minipage}{\\textwidth}}\n{\\end{minipage}}\n\\begin{document}\n\\begin{figure}\n",
        );

        assert_eq!(events.len(), 1);
        assert_eq!(events[0].name, "figure");
    }
}
