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

    for (line_index, line) in content.lines().enumerate() {
        events.extend(environment_events_in_line(line, line_index + 1));
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

        if line[..index].contains('%') {
            break;
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

fn parse_environment_event(
    line: &str,
    index: usize,
) -> Option<(EnvironmentEventKind, String, usize)> {
    let rest = &line[index..];
    let (kind, rest) = if let Some(rest) = rest.strip_prefix("\\begin") {
        (EnvironmentEventKind::Begin, rest)
    } else if let Some(rest) = rest.strip_prefix("\\end") {
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
}
