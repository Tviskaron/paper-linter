use std::collections::BTreeSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InactiveReason {
    LineComment,
    InlineVerb,
    OpaqueEnvironment,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InactiveSpan {
    pub start: usize,
    pub end: usize,
    pub reason: InactiveReason,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignificantSource {
    content: String,
    inactive_spans: Vec<InactiveSpan>,
}

impl SignificantSource {
    pub fn new(content: &str) -> Self {
        let inactive_spans = inactive_spans(content);
        let mut content = content.to_string();
        for span in inactive_spans.iter().rev() {
            if span.reason == InactiveReason::LineComment {
                continue;
            }
            mask_range(&mut content, span.start, span.end);
        }

        Self {
            content,
            inactive_spans,
        }
    }

    pub fn content(&self) -> &str {
        &self.content
    }

    pub fn inactive_spans(&self) -> &[InactiveSpan] {
        &self.inactive_spans
    }
}

pub fn mask_inactive_regions(content: &str) -> String {
    SignificantSource::new(content).content
}

pub fn mask_discarded_macro_arguments(content: &str) -> String {
    let mut masked = content.to_string();
    let mut discard_macros = BTreeSet::new();
    let mut index = 0;

    while index < content.len() {
        if content.as_bytes()[index] == b'%' && !is_escaped(content.as_bytes(), index) {
            index = skip_comment(content.as_bytes(), index);
            continue;
        }

        if content.as_bytes()[index] != b'\\' {
            index += 1;
            continue;
        }

        let command_start = index;
        let Some((command, after_command)) = read_command_name(content, command_start) else {
            index += 1;
            continue;
        };

        match command {
            "long" => {
                if let Some((macro_name, body, end)) = read_long_def(content, after_command) {
                    update_discard_macro(&mut discard_macros, macro_name, body);
                    index = end;
                } else {
                    index = after_command;
                }
            }
            "def" => {
                if let Some((macro_name, body, end)) = read_def(content, after_command) {
                    update_discard_macro(&mut discard_macros, macro_name, body);
                    index = end;
                } else {
                    index = after_command;
                }
            }
            "newcommand" | "renewcommand" | "providecommand" => {
                if let Some((macro_name, body, end)) =
                    read_command_definition(content, after_command)
                {
                    update_discard_macro(&mut discard_macros, macro_name, body);
                    index = end;
                } else {
                    index = after_command;
                }
            }
            _ if discard_macros.contains(command) => {
                let Some((arg_start, end)) = read_required_group_range(content, after_command)
                else {
                    index = after_command;
                    continue;
                };
                mask_range(&mut masked, arg_start, end);
                index = end;
            }
            _ => {
                index = after_command;
            }
        }
    }

    masked
}

fn inactive_spans(content: &str) -> Vec<InactiveSpan> {
    let mut spans = Vec::new();
    let mut opaque_environments = BTreeSet::from([
        "comment".to_string(),
        "verbatim".to_string(),
        "Verbatim".to_string(),
        "minted".to_string(),
        "lstlisting".to_string(),
    ]);
    let bytes = content.as_bytes();
    let mut index = 0;

    while index < content.len() {
        if bytes[index] == b'%' && !is_escaped(bytes, index) {
            let end = skip_comment(bytes, index);
            spans.push(InactiveSpan {
                start: index,
                end,
                reason: InactiveReason::LineComment,
            });
            index = end;
            continue;
        }

        if bytes[index] != b'\\' {
            index += 1;
            continue;
        }

        let Some((command, after_command)) = read_command_name(content, index) else {
            index += 1;
            continue;
        };

        if command == "verb" {
            if let Some(end) = inline_verb_end(content, index, after_command) {
                spans.push(InactiveSpan {
                    start: index,
                    end,
                    reason: InactiveReason::InlineVerb,
                });
                index = end;
                continue;
            }
        }

        if command == "excludecomment" {
            if let Some((arg_start, end)) = read_required_group_range(content, after_command) {
                let env_name = content[arg_start + 1..end - 1].trim();
                if !env_name.is_empty() {
                    opaque_environments.insert(env_name.to_string());
                }
                index = end;
                continue;
            }
        }

        if command == "begin" {
            let Some((arg_start, after_env)) = read_required_group_range(content, after_command)
            else {
                index = after_command;
                continue;
            };
            let env_name = content[arg_start + 1..after_env - 1].trim();
            if opaque_environments.contains(env_name) {
                let end = opaque_environment_end(content, env_name, after_env);
                spans.push(InactiveSpan {
                    start: index,
                    end,
                    reason: InactiveReason::OpaqueEnvironment,
                });
                index = end;
                continue;
            }
        }

        index = after_command;
    }

    spans
}

fn opaque_environment_end(content: &str, env_name: &str, start: usize) -> usize {
    let needle = format!("\\end{{{env_name}}}");
    let mut index = start;

    while let Some(relative) = content[index..].find(&needle) {
        let candidate = index + relative;
        if !is_escaped(content.as_bytes(), candidate) {
            return candidate + needle.len();
        }
        index = candidate + 1;
    }

    content.len()
}

fn inline_verb_end(content: &str, verb_start: usize, after_command: usize) -> Option<usize> {
    let mut index = after_command;
    if content.as_bytes().get(index) == Some(&b'*') {
        index += 1;
    }

    let delimiter = content[index..].chars().next()?;
    if delimiter.is_ascii_whitespace() || delimiter == '{' || delimiter == '}' {
        return None;
    }

    let content_start = index + delimiter.len_utf8();
    let relative_end = content[content_start..].find(delimiter)?;
    let end = content_start + relative_end + delimiter.len_utf8();
    (end > verb_start).then_some(end)
}

fn update_discard_macro(discard_macros: &mut BTreeSet<String>, macro_name: &str, body: &str) {
    if is_discard_body(body) {
        discard_macros.insert(macro_name.to_string());
    } else {
        discard_macros.remove(macro_name);
    }
}

fn is_discard_body(body: &str) -> bool {
    matches!(body.trim(), "" | "\\relax")
}

fn read_long_def(content: &str, after_long: usize) -> Option<(&str, &str, usize)> {
    let index = skip_ascii_whitespace(content, after_long);
    let (command, after_command) = read_command_name(content, index)?;
    if command != "def" {
        return None;
    }

    read_def(content, after_command)
}

fn read_def(content: &str, after_def: usize) -> Option<(&str, &str, usize)> {
    let index = skip_ascii_whitespace(content, after_def);
    let (macro_name, after_macro) = read_command_name(content, index)?;
    let index = skip_ascii_whitespace(content, after_macro);
    let after_parameter = content[index..].strip_prefix("#1").map(|_| index + 2)?;
    let (body_start, end) = read_required_group_range(content, after_parameter)?;

    Some((macro_name, &content[body_start + 1..end - 1], end))
}

fn read_command_definition(content: &str, after_command: usize) -> Option<(&str, &str, usize)> {
    let index = skip_command_star(content, after_command);
    let (name_start, after_name) = read_required_group_range(content, index)?;
    let macro_name = content[name_start + 1..after_name - 1].trim();
    let macro_name = macro_name.strip_prefix('\\')?;
    if macro_name.is_empty() || !macro_name.chars().all(|ch| ch.is_ascii_alphabetic()) {
        return None;
    }

    let index = skip_ascii_whitespace(content, after_name);
    let (arg_count, after_arg_count) = read_optional_group(content, index)?;
    if arg_count.trim() != "1" {
        return None;
    }

    let index = skip_ascii_whitespace(content, after_arg_count);
    if content[index..].starts_with('[') {
        return None;
    }

    let (body_start, end) = read_required_group_range(content, index)?;
    Some((macro_name, &content[body_start + 1..end - 1], end))
}

fn read_command_name(content: &str, slash_index: usize) -> Option<(&str, usize)> {
    if content.as_bytes().get(slash_index) != Some(&b'\\') {
        return None;
    }

    let command_start = slash_index + 1;
    let mut command_end = command_start;

    while command_end < content.len() && content.as_bytes()[command_end].is_ascii_alphabetic() {
        command_end += 1;
    }

    if command_end == command_start {
        None
    } else {
        Some((&content[command_start..command_end], command_end))
    }
}

fn read_required_group_range(content: &str, offset: usize) -> Option<(usize, usize)> {
    let start = skip_ascii_whitespace(content, offset);
    if content.as_bytes().get(start) != Some(&b'{') {
        return None;
    }

    let end = balanced_group_end(content, start, b'{', b'}')?;
    Some((start, end + 1))
}

fn read_optional_group(content: &str, offset: usize) -> Option<(&str, usize)> {
    let start = skip_ascii_whitespace(content, offset);
    if content.as_bytes().get(start) != Some(&b'[') {
        return None;
    }

    let end = balanced_group_end(content, start, b'[', b']')?;
    Some((&content[start + 1..end], end + 1))
}

fn balanced_group_end(content: &str, start: usize, open: u8, close: u8) -> Option<usize> {
    let bytes = content.as_bytes();
    let mut depth = 0usize;
    let mut index = start;

    while index < bytes.len() {
        match bytes[index] {
            b'\\' => {
                index = (index + 2).min(bytes.len());
            }
            byte if byte == open => {
                depth += 1;
                index += 1;
            }
            byte if byte == close => {
                depth -= 1;
                if depth == 0 {
                    return Some(index);
                }
                index += 1;
            }
            _ => {
                index += 1;
            }
        }
    }

    None
}

fn skip_command_star(content: &str, offset: usize) -> usize {
    let index = skip_ascii_whitespace(content, offset);
    if content.as_bytes().get(index) == Some(&b'*') {
        index + 1
    } else {
        index
    }
}

fn skip_ascii_whitespace(content: &str, mut offset: usize) -> usize {
    while content
        .as_bytes()
        .get(offset)
        .is_some_and(u8::is_ascii_whitespace)
    {
        offset += 1;
    }
    offset
}

fn skip_comment(bytes: &[u8], mut index: usize) -> usize {
    while index < bytes.len() && bytes[index] != b'\n' {
        index += 1;
    }
    index
}

fn is_escaped(bytes: &[u8], index: usize) -> bool {
    let mut count = 0usize;
    let mut cursor = index;
    while cursor > 0 && bytes[cursor - 1] == b'\\' {
        count += 1;
        cursor -= 1;
    }
    count % 2 == 1
}

fn mask_range(content: &mut String, start: usize, end: usize) {
    let replacement: String = content[start..end]
        .chars()
        .map(|ch| if ch == '\n' { '\n' } else { 'x' })
        .collect();
    content.replace_range(start..end, &replacement);
}

#[cfg(test)]
mod tests {
    use super::{
        mask_discarded_macro_arguments, mask_inactive_regions, InactiveReason, SignificantSource,
    };

    #[test]
    fn masks_long_def_relax_macro_argument() {
        let content = "\\long\\def\\todel#1{\\relax}\nText \\todel{TODO\n\\label{dead}} done\n";

        let masked = mask_discarded_macro_arguments(content);

        assert!(masked.contains("\\long\\def\\todel#1{\\relax}"));
        assert!(!masked.contains("TODO"));
        assert!(!masked.contains("\\label{dead}"));
        assert_eq!(masked.lines().count(), content.lines().count());
    }

    #[test]
    fn does_not_mask_macro_that_prints_its_argument() {
        let content = "\\def\\note#1{#1}\n\\note{TODO}\n";

        let masked = mask_discarded_macro_arguments(content);

        assert!(masked.contains("\\note{TODO}"));
    }

    #[test]
    fn later_definition_overrides_discard_definition() {
        let content = "\\def\\note#1{\\relax}\n\\note{TODO hidden}\n\\def\\note#1{#1}\n\\note{TODO visible}\n";

        let masked = mask_discarded_macro_arguments(content);

        assert!(!masked.contains("TODO hidden"));
        assert!(masked.contains("TODO visible"));
    }

    #[test]
    fn masks_newcommand_empty_body_argument() {
        let content = "\\newcommand{\\ignore}[1]{}\n\\ignore{TODO}\n";

        let masked = mask_discarded_macro_arguments(content);

        assert!(!masked.contains("TODO"));
    }

    #[test]
    fn masks_inline_verb_regions() {
        let content = "Text \\verb|\\end{document}| after\n\\end{document}\n";

        let source = SignificantSource::new(content);

        assert!(!source.content().contains("\\verb|\\end{document}|"));
        assert!(source.content().contains("\\end{document}"));
        assert_eq!(
            source.inactive_spans()[0].reason,
            InactiveReason::InlineVerb
        );
        assert_eq!(source.content().lines().count(), content.lines().count());
        assert_eq!(source.content().len(), content.len());
    }

    #[test]
    fn masks_comment_environments() {
        let content = "\\begin{comment}\nTODO\n\\end{comment}\nActive\n";

        let masked = mask_inactive_regions(content);

        assert!(!masked.contains("TODO"));
        assert!(masked.contains("Active"));
        assert_eq!(masked.lines().count(), content.lines().count());
    }

    #[test]
    fn masks_excluded_comment_environments() {
        let content =
            "\\excludecomment{draftonly}\n\\begin{draftonly}\nTODO\n\\end{draftonly}\nActive\n";

        let masked = mask_inactive_regions(content);

        assert!(masked.contains("\\excludecomment{draftonly}"));
        assert!(!masked.contains("TODO"));
        assert!(masked.contains("Active"));
    }

    #[test]
    fn masks_multiple_opaque_environments_after_unicode_text() {
        let content = "\\begin{comment}\n“unicode”\n\\end{comment}\n\\begin{comment}\nTODO\n\\end{comment}\nActive\n";

        let masked = mask_inactive_regions(content);

        assert!(!masked.contains("\\begin{comment}"));
        assert!(!masked.contains("TODO"));
        assert!(masked.contains("Active"));
    }

    #[test]
    fn masks_line_comments() {
        let content = "Active % TODO hidden\nNext\n";

        let source = SignificantSource::new(content);

        assert_eq!(
            source.inactive_spans()[0].reason,
            InactiveReason::LineComment
        );
        assert!(source.content().contains("Active"));
        assert!(source.content().contains("TODO"));
        assert!(source.content().contains("Next"));
    }
}
