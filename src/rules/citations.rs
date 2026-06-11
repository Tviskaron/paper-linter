use std::collections::{HashMap, HashSet};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::diagnostic::{Diagnostic, Severity};

pub const CODES: [&str; 6] = ["CIT001", "CIT002", "CIT003", "CIT004", "CIT005", "CIT006"];

#[derive(Debug, Clone)]
pub struct SourceFile {
    pub path: PathBuf,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CitationUse {
    key: String,
    file: PathBuf,
    line: usize,
    column: usize,
    is_nocite: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct BibliographyDecl {
    path: String,
    file: PathBuf,
    line: usize,
    column: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct BibEntry {
    entry_type: String,
    key: String,
    fields: HashMap<String, String>,
    file: PathBuf,
    line: usize,
    column: usize,
}

pub fn check_project(
    tex_files: &[SourceFile],
    explicit_bib_files: &[PathBuf],
) -> Result<Vec<Diagnostic>, io::Error> {
    let mut citations = Vec::new();
    let mut declarations = Vec::new();

    for file in tex_files {
        citations.extend(find_citations(&file.path, &file.content));
        declarations.extend(find_bibliographies(&file.path, &file.content));
    }

    let mut diagnostics = Vec::new();
    let bib_paths = bibliography_paths(&declarations, explicit_bib_files);
    let mut entries = Vec::new();

    for bib_path in bib_paths {
        match fs::read_to_string(&bib_path) {
            Ok(content) => entries.extend(parse_bib_entries(&bib_path, &content)),
            Err(error) if explicit_bib_files.iter().any(|path| path == &bib_path) => {
                return Err(error);
            }
            Err(_) => {
                if let Some(declaration) = declarations
                    .iter()
                    .find(|declaration| resolve_bib_path(declaration) == bib_path)
                {
                    diagnostics.push(Diagnostic::new(
                        "CIT003",
                        Severity::Error,
                        format!("bibliography file '{}' was not found", bib_path.display()),
                        &declaration.file,
                        declaration.line,
                        declaration.column,
                    ));
                }
            }
        }
    }

    let entry_keys: HashSet<&str> = entries.iter().map(|entry| entry.key.as_str()).collect();
    let mut cited_keys = HashSet::new();
    let has_nocite_all = citations
        .iter()
        .any(|citation| citation.is_nocite && citation.key == "*");

    for citation in &citations {
        if citation.key == "*" {
            continue;
        }

        cited_keys.insert(citation.key.as_str());
        if !entry_keys.contains(citation.key.as_str()) {
            diagnostics.push(Diagnostic::new(
                "CIT001",
                Severity::Error,
                format!("citation key '{}' not found in bibliography", citation.key),
                &citation.file,
                citation.line,
                citation.column,
            ));
        }
    }

    if !has_nocite_all {
        for entry in &entries {
            if !cited_keys.contains(entry.key.as_str()) {
                diagnostics.push(Diagnostic::new(
                    "CIT002",
                    Severity::Warning,
                    format!("bibliography entry '{}' is never cited", entry.key),
                    &entry.file,
                    entry.line,
                    entry.column,
                ));
            }
        }
    }

    for entry in &entries {
        let missing = missing_required_fields(entry);
        if !missing.is_empty() {
            diagnostics.push(Diagnostic::new(
                "CIT004",
                Severity::Warning,
                format!(
                    "bibliography entry '{}' is missing required field(s): {}",
                    entry.key,
                    missing.join(", ")
                ),
                &entry.file,
                entry.line,
                entry.column,
            ));
        }
    }

    diagnostics.extend(find_duplicate_keys(&entries));
    diagnostics.extend(find_similar_titles(&entries));

    Ok(diagnostics)
}

pub fn explicit_bib_files(paths: &[PathBuf]) -> Vec<PathBuf> {
    paths
        .iter()
        .filter(|path| is_bib_file(path))
        .cloned()
        .collect()
}

fn is_bib_file(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("bib"))
}

fn bibliography_paths(
    declarations: &[BibliographyDecl],
    explicit_bib_files: &[PathBuf],
) -> Vec<PathBuf> {
    let mut paths: Vec<_> = declarations.iter().map(resolve_bib_path).collect();
    paths.extend(explicit_bib_files.iter().cloned());
    paths.sort();
    paths.dedup();
    paths
}

fn resolve_bib_path(declaration: &BibliographyDecl) -> PathBuf {
    let mut path = PathBuf::from(declaration.path.trim());
    if path.extension().is_none() {
        path.set_extension("bib");
    }

    if path.is_absolute() {
        path
    } else {
        declaration
            .file
            .parent()
            .unwrap_or_else(|| Path::new(""))
            .join(path)
    }
}

fn find_citations(path: &Path, content: &str) -> Vec<CitationUse> {
    let mut citations = Vec::new();

    for (line_index, raw_line) in content.lines().enumerate() {
        let line = uncommented_line(raw_line);
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
    }

    citations
}

fn find_bibliographies(path: &Path, content: &str) -> Vec<BibliographyDecl> {
    let mut declarations = Vec::new();

    for (line_index, raw_line) in content.lines().enumerate() {
        let line = uncommented_line(raw_line);
        let mut offset = 0;

        while let Some(relative) = line[offset..].find('\\') {
            let start = offset + relative;
            let Some((name, after_name)) = read_command_name(line, start) else {
                offset = start + 1;
                continue;
            };

            if name != "bibliography" && name != "addbibresource" {
                offset = after_name;
                continue;
            }

            let Some((body_start, body_end)) = read_command_argument(line, after_name) else {
                offset = after_name;
                continue;
            };

            for (bib_path, column_offset) in split_keys(&line[body_start..body_end]) {
                declarations.push(BibliographyDecl {
                    path: bib_path,
                    file: path.to_path_buf(),
                    line: line_index + 1,
                    column: char_column(line, body_start + column_offset),
                });
            }

            offset = body_end + 1;
        }
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

fn skip_ascii_whitespace(line: &str, mut offset: usize) -> usize {
    while let Some(byte) = line.as_bytes().get(offset) {
        if !byte.is_ascii_whitespace() {
            break;
        }
        offset += 1;
    }
    offset
}

fn balanced_group_end(line: &str, start: usize, open: char, close: char) -> Option<usize> {
    let mut depth = 0;
    let mut escaped = false;

    for (relative, character) in line[start..].char_indices() {
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
            depth -= 1;
            if depth == 0 {
                return Some(start + relative);
            }
        }
    }

    None
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

fn parse_bib_entries(path: &Path, content: &str) -> Vec<BibEntry> {
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

fn missing_required_fields(entry: &BibEntry) -> Vec<&'static str> {
    let mut missing = Vec::new();

    if !has_any_field(entry, &["author", "editor"]) {
        missing.push("author/editor");
    }
    if !has_any_field(entry, &["year"]) {
        missing.push("year");
    }
    if !has_venue_field(entry) {
        missing.push("venue");
    }

    missing
}

fn has_any_field(entry: &BibEntry, names: &[&str]) -> bool {
    names.iter().any(|name| entry.fields.contains_key(*name))
}

fn has_venue_field(entry: &BibEntry) -> bool {
    match entry.entry_type.as_str() {
        "article" => has_any_field(entry, &["journal"]),
        "inproceedings" | "conference" | "incollection" => has_any_field(entry, &["booktitle"]),
        "book" | "inbook" => has_any_field(entry, &["publisher"]),
        "phdthesis" | "mastersthesis" => has_any_field(entry, &["school"]),
        "techreport" => has_any_field(entry, &["institution"]),
        "misc" => has_any_field(
            entry,
            &["howpublished", "archiveprefix", "eprint", "url", "doi"],
        ),
        _ => true,
    }
}

fn find_duplicate_keys(entries: &[BibEntry]) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    let mut first_seen: HashMap<&str, &BibEntry> = HashMap::new();

    for entry in entries {
        if let Some(first) = first_seen.get(entry.key.as_str()) {
            diagnostics.push(Diagnostic::new(
                "CIT005",
                Severity::Warning,
                format!(
                    "duplicate bibliography key '{}' first defined at {}:{}",
                    entry.key,
                    first.file.display(),
                    first.line
                ),
                &entry.file,
                entry.line,
                entry.column,
            ));
        } else {
            first_seen.insert(entry.key.as_str(), entry);
        }
    }

    diagnostics
}

fn find_similar_titles(entries: &[BibEntry]) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    let titled_entries: Vec<_> = entries
        .iter()
        .filter_map(|entry| normalized_title(entry).map(|title| (entry, title)))
        .collect();

    for left_index in 0..titled_entries.len() {
        let (left, left_title) = &titled_entries[left_index];
        for (right, right_title) in titled_entries.iter().skip(left_index + 1) {
            if left.key == right.key {
                continue;
            }

            if titles_are_similar(left_title, right_title) {
                diagnostics.push(Diagnostic::new(
                    "CIT006",
                    Severity::Warning,
                    format!(
                        "bibliography entry '{}' has a title very similar to '{}'",
                        right.key, left.key
                    ),
                    &right.file,
                    right.line,
                    right.column,
                ));
            }
        }
    }

    diagnostics
}

fn normalized_title(entry: &BibEntry) -> Option<String> {
    let title = entry.fields.get("title")?;
    let mut normalized = String::new();
    let mut previous_was_space = false;

    for character in title.chars() {
        if character == '\\' || character == '{' || character == '}' {
            continue;
        }

        if character.is_ascii_alphanumeric() {
            normalized.push(character.to_ascii_lowercase());
            previous_was_space = false;
        } else if (character.is_whitespace()
            || matches!(character, '-' | '_' | ':' | ';' | ',' | '.'))
            && !previous_was_space
            && !normalized.is_empty()
        {
            normalized.push(' ');
            previous_was_space = true;
        }
    }

    let normalized = normalized.trim().to_string();
    if normalized.len() < 24 {
        None
    } else {
        Some(normalized)
    }
}

fn titles_are_similar(left: &str, right: &str) -> bool {
    if left == right {
        return true;
    }

    let shorter = left.len().min(right.len());
    let longer = left.len().max(right.len());
    if shorter < 24 || shorter * 100 / longer < 85 {
        return false;
    }

    levenshtein_at_most(left, right, 4)
}

fn levenshtein_at_most(left: &str, right: &str, max_distance: usize) -> bool {
    let left_chars: Vec<char> = left.chars().collect();
    let right_chars: Vec<char> = right.chars().collect();

    if left_chars.len().abs_diff(right_chars.len()) > max_distance {
        return false;
    }

    let mut previous: Vec<usize> = (0..=right_chars.len()).collect();
    let mut current = vec![0; right_chars.len() + 1];

    for (left_index, left_char) in left_chars.iter().enumerate() {
        current[0] = left_index + 1;
        let mut row_min = current[0];

        for (right_index, right_char) in right_chars.iter().enumerate() {
            let substitution_cost = usize::from(left_char != right_char);
            current[right_index + 1] = (previous[right_index + 1] + 1)
                .min(current[right_index] + 1)
                .min(previous[right_index] + substitution_cost);
            row_min = row_min.min(current[right_index + 1]);
        }

        if row_min > max_distance {
            return false;
        }

        std::mem::swap(&mut previous, &mut current);
    }

    previous[right_chars.len()] <= max_distance
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{
        find_bibliographies, find_citations, find_duplicate_keys, find_similar_titles,
        missing_required_fields, parse_bib_entries, BibliographyDecl,
    };

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
    fn detects_duplicate_bib_keys() {
        let entries = parse_bib_entries(
            Path::new("refs.bib"),
            r"@article{alpha, author={A}, title={Long Enough Title One}, journal={J}, year={2024}}
@misc{alpha, author={B}, title={Long Enough Title Two}, year={2024}, eprint={1}}",
        );

        let diagnostics = find_duplicate_keys(&entries);

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, "CIT005");
        assert!(diagnostics[0].message.contains("first defined"));
    }

    #[test]
    fn detects_different_keys_with_nearly_same_titles() {
        let entries = parse_bib_entries(
            Path::new("refs.bib"),
            r"@article{alpha, author={A}, title={Scaling LLM Test-Time Compute for Reasoning}, journal={J}, year={2024}}
@misc{beta, author={B}, title={Scaling LLM Test Time Compute for Reasoning}, year={2024}, eprint={1}}",
        );

        let diagnostics = find_similar_titles(&entries);

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, "CIT006");
        assert!(diagnostics[0].message.contains("beta"));
        assert!(diagnostics[0].message.contains("alpha"));
    }

    #[test]
    fn ignores_short_similar_titles() {
        let entries = parse_bib_entries(
            Path::new("refs.bib"),
            r"@article{alpha, author={A}, title={LLM Survey}, journal={J}, year={2024}}
@misc{beta, author={B}, title={LLM Survey}, year={2024}, eprint={1}}",
        );

        assert!(find_similar_titles(&entries).is_empty());
    }
}
