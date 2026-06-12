use std::collections::{HashMap, HashSet};
use std::path::Path;

use super::syntax::{balanced_group_end, skip_ascii_whitespace};
use super::BibEntry;

pub(super) fn parse_bib_entries(path: &Path, content: &str) -> Vec<BibEntry> {
    parse_bib_entries_matching(path, content, None)
}

pub(super) fn parse_bib_entries_for_keys(
    path: &Path,
    content: &str,
    keys: &HashSet<String>,
) -> Vec<BibEntry> {
    if keys.len() <= 4 && keys.iter().all(|key| key.len() >= 3) {
        return parse_bib_entries_by_key_search(path, content, keys);
    }

    parse_bib_entries_by_header_scan(path, content, keys)
}

fn parse_bib_entries_by_key_search(
    path: &Path,
    content: &str,
    keys: &HashSet<String>,
) -> Vec<BibEntry> {
    let lower_content = content.to_ascii_lowercase();
    let mut line_starts = None;
    let mut seen_starts = HashSet::new();
    let mut entries = Vec::new();

    for key in keys {
        let lower_key = key.to_ascii_lowercase();
        let mut offset = 0;
        while let Some(relative) = lower_content[offset..].find(&lower_key) {
            let key_index = offset + relative;
            offset = key_index + key.len();

            let Some(at_index) = content[..key_index].rfind('@') else {
                continue;
            };
            if !seen_starts.insert(at_index) {
                continue;
            }

            let Some((entry_type, after_type)) = read_bib_type(content, at_index) else {
                continue;
            };
            let after_type = skip_ascii_whitespace(content, after_type);
            let Some(open) = content[after_type..].chars().next() else {
                continue;
            };
            if open != '{' && open != '(' {
                continue;
            }

            let entry_type_lower = entry_type.to_ascii_lowercase();
            if matches!(entry_type_lower.as_str(), "comment" | "string" | "preamble") {
                continue;
            }

            let body_start = after_type + 1;
            let Some((entry_key, entry_key_start, comma)) = read_entry_key(content, body_start)
            else {
                continue;
            };
            if !key_matches(keys, entry_key) || entry_key_start != key_index {
                continue;
            }

            let close = if open == '{' { '}' } else { ')' };
            let Some(end) = balanced_group_end(content, after_type, open, close) else {
                continue;
            };
            if comma >= end {
                continue;
            }

            let line_starts = line_starts.get_or_insert_with(|| line_starts_for(content));
            if let Some(entry) = parse_bib_entry(
                path,
                content,
                line_starts,
                entry_type_lower,
                body_start,
                end,
            ) {
                entries.push(entry);
            }
        }
    }

    entries.sort_by(|left, right| {
        left.file
            .cmp(&right.file)
            .then(left.line.cmp(&right.line))
            .then(left.column.cmp(&right.column))
            .then(left.key.cmp(&right.key))
    });
    entries
}

fn parse_bib_entries_by_header_scan(
    path: &Path,
    content: &str,
    keys: &HashSet<String>,
) -> Vec<BibEntry> {
    let mut line_starts = None;
    let mut entries = Vec::new();
    let mut line_start = 0;
    let bytes = content.as_bytes();
    let mut active_entry: Option<(u8, u8, usize)> = None;

    while line_start < content.len() {
        let line_end = content[line_start..]
            .find('\n')
            .map(|relative| line_start + relative)
            .unwrap_or(content.len());
        let scan_end = line_end.saturating_add(1).min(content.len());

        if let Some((open, close, depth)) = active_entry {
            let depth = delimiter_depth(bytes, line_start, scan_end, open, close, depth);
            active_entry = (depth > 0).then_some((open, close, depth));
            line_start = line_end.saturating_add(1);
            continue;
        }

        let mut at_index = line_start;
        while at_index < line_end && bytes[at_index].is_ascii_whitespace() {
            at_index += 1;
        }

        if bytes.get(at_index) == Some(&b'@') {
            if let Some((entry_type, after_type)) = read_bib_type(content, at_index) {
                let after_type = skip_ascii_whitespace(content, after_type);
                if let Some(open) = content[after_type..].chars().next() {
                    if open == '{' || open == '(' {
                        let entry_type_lower = entry_type.to_ascii_lowercase();
                        let open_byte = open as u8;
                        let close = if open == '{' { '}' } else { ')' };
                        let close_byte = close as u8;

                        if !matches!(entry_type_lower.as_str(), "comment" | "string" | "preamble") {
                            let body_start = after_type + 1;
                            if let Some((key, _, _)) = read_entry_key(content, body_start) {
                                if key_matches(keys, key) {
                                    if let Some(end) =
                                        balanced_group_end(content, after_type, open, close)
                                    {
                                        let line_starts = line_starts
                                            .get_or_insert_with(|| line_starts_for(content));
                                        if let Some(entry) = parse_bib_entry(
                                            path,
                                            content,
                                            line_starts,
                                            entry_type_lower,
                                            body_start,
                                            end,
                                        ) {
                                            entries.push(entry);
                                        }
                                    }
                                }
                            }
                        }

                        let depth =
                            delimiter_depth(bytes, after_type, scan_end, open_byte, close_byte, 0);
                        active_entry = (depth > 0).then_some((open_byte, close_byte, depth));
                    }
                }
            }
        }

        line_start = line_end.saturating_add(1);
    }

    entries
}

fn delimiter_depth(
    bytes: &[u8],
    start: usize,
    end: usize,
    open: u8,
    close: u8,
    mut depth: usize,
) -> usize {
    for byte in &bytes[start..end] {
        if *byte == open {
            depth += 1;
        } else if *byte == close && depth > 0 {
            depth -= 1;
        }
    }
    depth
}

fn parse_bib_entries_matching(
    path: &Path,
    content: &str,
    keys: Option<&HashSet<String>>,
) -> Vec<BibEntry> {
    let mut line_starts = None;
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

        let entry_type_lower = entry_type.to_ascii_lowercase();
        let close = if open == '{' { '}' } else { ')' };
        if matches!(entry_type_lower.as_str(), "comment" | "string" | "preamble") {
            let Some(end) = balanced_group_end(content, after_type, open, close) else {
                break;
            };
            offset = end + 1;
            continue;
        }

        let body_start = after_type + 1;
        if let Some(keys) = keys {
            let Some((key, _, comma)) = read_entry_key(content, body_start) else {
                offset = body_start;
                continue;
            };
            if !key_matches(keys, key) {
                offset = comma + 1;
                continue;
            }
        }

        let Some(end) = balanced_group_end(content, after_type, open, close) else {
            break;
        };

        let line_starts = line_starts.get_or_insert_with(|| line_starts_for(content));
        if let Some(entry) = parse_bib_entry(
            path,
            content,
            line_starts,
            entry_type_lower,
            body_start,
            end,
        ) {
            entries.push(entry);
        }

        offset = end + 1;
    }

    entries
}

fn key_matches(keys: &HashSet<String>, key: &str) -> bool {
    keys.iter()
        .any(|candidate| candidate.eq_ignore_ascii_case(key))
}

fn read_entry_key(content: &str, body_start: usize) -> Option<(&str, usize, usize)> {
    let comma = content[body_start..]
        .find(',')
        .map(|relative| body_start + relative)?;
    let key = content[body_start..comma].trim();
    if key.is_empty() {
        return None;
    }

    let key_start = content[body_start..comma]
        .find(key)
        .map(|relative| body_start + relative)
        .unwrap_or(body_start);

    Some((key, key_start, comma))
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
    let (key, key_start, comma) = read_entry_key(content, body_start)?;
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

fn line_starts_for(content: &str) -> Vec<usize> {
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
    use std::collections::HashSet;
    use std::path::Path;

    use super::{parse_bib_entries, parse_bib_entries_for_keys};
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

    #[test]
    fn parses_only_requested_bib_entries() {
        let entries = parse_bib_entries_for_keys(
            Path::new("refs.bib"),
            r#"@article{alpha,
  title = {A {Nested} Title},
  author = "Ada Lovelace",
  journal = {Journal},
  year = 1843
}
@misc{unused, title={Unused Long Enough Title}, year={2024}, eprint={1}}"#,
            &HashSet::from(["alpha".to_string()]),
        );

        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].key, "alpha");
        assert!(entries[0].fields.contains_key("author"));
    }

    #[test]
    fn parses_many_requested_keys_with_header_scan() {
        let keys = HashSet::from([
            "alpha".to_string(),
            "beta".to_string(),
            "gamma".to_string(),
            "delta".to_string(),
            "epsilon".to_string(),
            "x".to_string(),
        ]);

        let entries = parse_bib_entries_for_keys(
            Path::new("refs.bib"),
            r#"@misc{unused,
  title = {Unused},
  note = {
@article{alpha,
  title = {Not an entry}
}
  }
}
  @article{alpha,
    title = {A Real Title},
    author = {Ada Lovelace},
    journal = {Journal},
    year = {1843}
  }
@misc{zeta, title={Other}}"#,
            &keys,
        );

        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].key, "alpha");
        assert_eq!(entries[0].line, 9);
        assert_eq!(entries[0].fields["title"], "A Real Title");
    }
}
