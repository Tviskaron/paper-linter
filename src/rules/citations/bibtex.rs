use std::collections::HashMap;
use std::path::Path;

use super::syntax::{balanced_group_end, skip_ascii_whitespace};
use super::BibEntry;

pub(super) fn parse_bib_entries(path: &Path, content: &str) -> Vec<BibEntry> {
    let line_starts = line_starts(content);
    let mut entries = Vec::new();
    let mut offset = 0;

    while let Some(relative) = content[offset..].find('@') {
        let at_index = offset + relative;
        let Some((entry_type, after_type)) = read_bib_type(content, at_index) else {
            offset = at_index + 1;
            continue;
        };

        let after_type = skip_ascii_whitespace(content, after_type);
        let Some(open) = content[after_type..].chars().next() else {
            break;
        };
        if open != '{' && open != '(' {
            offset = after_type;
            continue;
        }

        let close = if open == '{' { '}' } else { ')' };
        let Some(end) = balanced_group_end(content, after_type, open, close) else {
            break;
        };

        let entry_type_lower = entry_type.to_ascii_lowercase();
        if !matches!(entry_type_lower.as_str(), "comment" | "string" | "preamble") {
            if let Some(entry) = parse_bib_entry(
                path,
                content,
                &line_starts,
                entry_type_lower,
                after_type + 1,
                end,
            ) {
                entries.push(entry);
            }
        }

        offset = end + 1;
    }

    entries
}

fn read_bib_type(content: &str, at_index: usize) -> Option<(&str, usize)> {
    let type_start = at_index + 1;
    let mut type_end = type_start;

    for (index, character) in content[type_start..].char_indices() {
        if character.is_ascii_alphabetic() {
            type_end = type_start + index + character.len_utf8();
        } else {
            break;
        }
    }

    if type_end == type_start {
        None
    } else {
        Some((&content[type_start..type_end], type_end))
    }
}

fn parse_bib_entry(
    path: &Path,
    content: &str,
    line_starts: &[usize],
    entry_type: String,
    body_start: usize,
    body_end: usize,
) -> Option<BibEntry> {
    let mut comma = None;
    for (index, character) in content[body_start..body_end].char_indices() {
        if character == ',' {
            comma = Some(body_start + index);
            break;
        }
    }

    let comma = comma?;
    let key = content[body_start..comma].trim();
    if key.is_empty() {
        return None;
    }

    let key_start = content[body_start..comma]
        .find(key)
        .map(|relative| body_start + relative)
        .unwrap_or(body_start);
    let (line, column) = line_column(line_starts, key_start);
    let fields = parse_fields(&content[comma + 1..body_end]);

    Some(BibEntry {
        entry_type,
        key: key.to_string(),
        fields,
        file: path.to_path_buf(),
        line,
        column,
    })
}

fn parse_fields(body: &str) -> HashMap<String, String> {
    let mut fields = HashMap::new();
    let bytes = body.as_bytes();
    let mut offset = 0;

    while offset < body.len() {
        while offset < body.len() && !bytes[offset].is_ascii_alphabetic() && bytes[offset] != b'_' {
            offset += 1;
        }

        let field_start = offset;
        while offset < body.len()
            && (bytes[offset].is_ascii_alphanumeric()
                || bytes[offset] == b'_'
                || bytes[offset] == b'-')
        {
            offset += 1;
        }

        if field_start == offset {
            break;
        }

        let field_name = body[field_start..offset].to_ascii_lowercase();
        let after_name = skip_ascii_whitespace(body, offset);
        if body[after_name..].starts_with('=') {
            let (value, next_offset) = read_bib_value(body, after_name + 1);
            fields.insert(field_name, value);
            offset = next_offset;
        } else {
            offset = after_name.saturating_add(1);
        }
    }

    fields
}

fn read_bib_value(body: &str, mut offset: usize) -> (String, usize) {
    offset = skip_ascii_whitespace(body, offset);
    let Some(character) = body[offset..].chars().next() else {
        return (String::new(), offset);
    };

    if character == '{' {
        let end = balanced_group_end(body, offset, '{', '}')
            .map(|end| end + 1)
            .unwrap_or(body.len());
        return (
            strip_bib_value(&body[offset + 1..end.saturating_sub(1)]),
            end,
        );
    }

    if character == '"' {
        let end = quoted_string_end(body, offset).unwrap_or(body.len());
        return (
            strip_bib_value(&body[offset + 1..end.saturating_sub(1)]),
            end,
        );
    }

    let value_start = offset;
    while let Some(byte) = body.as_bytes().get(offset) {
        if *byte == b',' {
            break;
        }
        offset += 1;
    }

    (strip_bib_value(&body[value_start..offset]), offset)
}

fn strip_bib_value(value: &str) -> String {
    value.trim().to_string()
}

fn quoted_string_end(body: &str, start: usize) -> Option<usize> {
    let mut escaped = false;

    for (relative, character) in body[start + 1..].char_indices() {
        if escaped {
            escaped = false;
            continue;
        }

        if character == '\\' {
            escaped = true;
            continue;
        }

        if character == '"' {
            return Some(start + 1 + relative + 1);
        }
    }

    None
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

fn line_column(line_starts: &[usize], byte_index: usize) -> (usize, usize) {
    let line_index = line_starts.partition_point(|start| *start <= byte_index) - 1;
    let column = byte_index - line_starts[line_index] + 1;
    (line_index + 1, column)
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::parse_bib_entries;
    use crate::rules::citations::missing_required_fields;

    #[test]
    fn parses_bib_entries_and_nested_field_values() {
        let entries = parse_bib_entries(
            Path::new("refs.bib"),
            r#"@article{alpha,
  title = {A {Nested} Title},
  author = "Ada Lovelace",
  journal = {Journal},
  year = 1843
}
@string{ignored = "x"}
@misc{beta, title={Only Title}, eprint={1234.5678}}"#,
        );

        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].key, "alpha");
        assert!(entries[0].fields.contains_key("author"));
        assert!(entries[0].fields.contains_key("journal"));
        assert_eq!(
            missing_required_fields(&entries[0]),
            Vec::<&'static str>::new()
        );
        assert_eq!(
            missing_required_fields(&entries[1]),
            vec!["author/editor", "year"]
        );
    }
}
