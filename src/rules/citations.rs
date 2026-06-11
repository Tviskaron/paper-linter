use std::collections::{HashMap, HashSet};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::diagnostic::{Diagnostic, Severity};

mod bbl;
mod bibtex;
mod latex;
mod paths;
mod similarity;
mod syntax;

use bbl::parse_bbl_keys;
use bibtex::{parse_bib_entries, parse_bib_entries_for_keys};
use latex::{find_bibliographies, find_citations};
pub use paths::explicit_bib_files;
use paths::{alternate_bib_paths, bbl_fallback_paths, bibliography_paths, resolve_bib_path};
use similarity::find_similar_titles;

const LARGE_BIB_FAST_PATH_BYTES: u64 = 8 * 1024 * 1024;

#[derive(Debug, Clone)]
pub struct SourceFile {
    pub path: PathBuf,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CitationUse {
    key: String,
    command: String,
    kind: CitationKind,
    file: PathBuf,
    line: usize,
    column: usize,
    is_nocite: bool,
    is_starred: bool,
    has_optional_arg: bool,
    command_start: usize,
    command_end: usize,
    argument_start: usize,
    argument_end: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CitationKind {
    Parenthetical,
    Textual,
    Neutral,
    AuthorYearOnly,
    NoCite,
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
    let mut source_bibitem_keys = HashSet::new();

    for file in tex_files {
        citations.extend(find_citations(&file.path, &file.content));
        declarations.extend(find_bibliographies(&file.path, &file.content));
        source_bibitem_keys.extend(parse_bbl_keys(&file.content));
    }

    let mut diagnostics = Vec::new();
    diagnostics.extend(find_duplicate_bibliography_declarations(
        &declarations,
        tex_files,
    ));
    diagnostics.extend(find_citation_punctuation(&citations, tex_files));
    diagnostics.extend(find_collapsible_citations(&citations, tex_files));

    let bib_paths = bibliography_paths(&declarations, explicit_bib_files);
    let mut bbl_keys = HashSet::new();
    let source_files: Vec<_> = tex_files.iter().map(|file| file.path.clone()).collect();

    for declaration in &declarations {
        bbl_keys.extend(parse_bbl_fallback_keys(declaration, &source_files));
    }

    let mut cited_keys = HashSet::new();
    let has_nocite_all = citations
        .iter()
        .any(|citation| citation.is_nocite && citation.key == "*");

    for citation in &citations {
        if citation.key != "*" {
            cited_keys.insert(citation.key.as_str());
        }
    }

    let active_key_filter =
        active_bibliography_keys(&cited_keys, &bbl_keys, &source_bibitem_keys, has_nocite_all);

    let mut entries = Vec::new();
    for bib_path in bib_paths {
        let is_explicit_bib = explicit_bib_files.iter().any(|path| path == &bib_path);
        if should_skip_large_bib_with_bbl(&bib_path, is_explicit_bib, has_nocite_all, &bbl_keys) {
            continue;
        }

        match fs::read_to_string(&bib_path) {
            Ok(content) => entries.extend(parse_bib_entries_with_filter(
                &bib_path,
                &content,
                active_key_filter.as_ref(),
            )),
            Err(error) if explicit_bib_files.iter().any(|path| path == &bib_path) => {
                return Err(error);
            }
            Err(_) => {
                if let Some(declaration) = declarations
                    .iter()
                    .find(|declaration| resolve_bib_path(declaration) == bib_path)
                {
                    let fallback_keys = parse_bbl_fallback_keys(declaration, &source_files);
                    if let Some((alternate_path, content)) =
                        read_first_existing_bib(declaration, &source_files)
                    {
                        entries.extend(parse_bib_entries_with_filter(
                            &alternate_path,
                            &content,
                            active_key_filter.as_ref(),
                        ));
                        bbl_keys.extend(fallback_keys);
                        continue;
                    }

                    if fallback_keys.is_empty() {
                        diagnostics.push(Diagnostic::new(
                            "CIT003",
                            Severity::Error,
                            format!("bibliography file '{}' was not found", bib_path.display()),
                            &declaration.file,
                            declaration.line,
                            declaration.column,
                        ));
                    } else {
                        bbl_keys.extend(fallback_keys);
                    }
                }
            }
        }
    }

    let entry_keys: HashSet<&str> = entries.iter().map(|entry| entry.key.as_str()).collect();
    let known_keys: HashSet<&str> = entry_keys
        .iter()
        .copied()
        .chain(bbl_keys.iter().map(String::as_str))
        .chain(source_bibitem_keys.iter().map(String::as_str))
        .collect();

    for citation in &citations {
        if citation.key == "*" {
            continue;
        }

        if !known_keys.contains(citation.key.as_str()) {
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

    let scoped_entries = scoped_bibliography_entries(
        &entries,
        &cited_keys,
        &bbl_keys,
        &source_bibitem_keys,
        has_nocite_all,
    );

    if !has_nocite_all {
        for entry in &scoped_entries {
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

    for entry in &scoped_entries {
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

    diagnostics.extend(find_duplicate_keys(&scoped_entries));
    diagnostics.extend(find_similar_titles(&scoped_entries));

    Ok(diagnostics)
}

fn should_skip_large_bib_with_bbl(
    path: &Path,
    is_explicit_bib: bool,
    has_nocite_all: bool,
    bbl_keys: &HashSet<String>,
) -> bool {
    if is_explicit_bib || has_nocite_all || bbl_keys.is_empty() {
        return false;
    }

    fs::metadata(path)
        .map(|metadata| metadata.len() > LARGE_BIB_FAST_PATH_BYTES)
        .unwrap_or(false)
}

fn active_bibliography_keys(
    cited_keys: &HashSet<&str>,
    bbl_keys: &HashSet<String>,
    source_bibitem_keys: &HashSet<String>,
    has_nocite_all: bool,
) -> Option<HashSet<String>> {
    if has_nocite_all {
        return None;
    }

    let active_keys: HashSet<String> = cited_keys
        .iter()
        .map(|key| (*key).to_string())
        .chain(bbl_keys.iter().cloned())
        .chain(source_bibitem_keys.iter().cloned())
        .collect();

    if active_keys.is_empty() {
        None
    } else {
        Some(active_keys)
    }
}

fn parse_bib_entries_with_filter(
    path: &Path,
    content: &str,
    active_keys: Option<&HashSet<String>>,
) -> Vec<BibEntry> {
    match active_keys {
        Some(active_keys) => parse_bib_entries_for_keys(path, content, active_keys),
        None => parse_bib_entries(path, content),
    }
}

fn scoped_bibliography_entries<'a>(
    entries: &'a [BibEntry],
    cited_keys: &HashSet<&str>,
    bbl_keys: &HashSet<String>,
    source_bibitem_keys: &HashSet<String>,
    has_nocite_all: bool,
) -> Vec<&'a BibEntry> {
    if has_nocite_all {
        return entries.iter().collect();
    }

    let active_keys: HashSet<&str> = cited_keys
        .iter()
        .copied()
        .chain(bbl_keys.iter().map(String::as_str))
        .chain(source_bibitem_keys.iter().map(String::as_str))
        .collect();

    if active_keys.is_empty() {
        return entries.iter().collect();
    }

    entries
        .iter()
        .filter(|entry| active_keys.contains(entry.key.as_str()))
        .collect()
}

fn read_first_existing_bib(
    declaration: &BibliographyDecl,
    source_files: &[PathBuf],
) -> Option<(PathBuf, String)> {
    alternate_bib_paths(declaration, source_files)
        .into_iter()
        .find_map(|path| {
            fs::read_to_string(&path)
                .ok()
                .map(|content| (path, content))
        })
}

fn parse_bbl_fallback_keys(
    declaration: &BibliographyDecl,
    source_files: &[PathBuf],
) -> Vec<String> {
    bbl_fallback_paths(declaration, source_files)
        .into_iter()
        .filter_map(|path| fs::read_to_string(path).ok())
        .flat_map(|content| parse_bbl_keys(&content))
        .collect()
}

fn missing_required_fields(entry: &BibEntry) -> Vec<&'static str> {
    let mut missing = Vec::new();

    if !has_any_field(entry, &["author", "editor"]) {
        missing.push("author/editor");
    }
    if !has_date_field(entry) {
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

fn has_date_field(entry: &BibEntry) -> bool {
    has_any_field(entry, &["year", "date", "urldate"])
        || entry
            .fields
            .get("note")
            .is_some_and(|note| note.to_ascii_lowercase().contains("access"))
}

fn has_venue_field(entry: &BibEntry) -> bool {
    if has_any_field(
        entry,
        &[
            "journal",
            "booktitle",
            "publisher",
            "school",
            "institution",
            "howpublished",
            "archiveprefix",
            "eprint",
            "url",
            "doi",
        ],
    ) {
        return true;
    }

    !matches!(
        entry.entry_type.as_str(),
        "article"
            | "inproceedings"
            | "conference"
            | "incollection"
            | "book"
            | "inbook"
            | "phdthesis"
            | "mastersthesis"
            | "techreport"
            | "misc"
    )
}

fn find_duplicate_keys(entries: &[&BibEntry]) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    let mut first_seen: HashMap<&str, &BibEntry> = HashMap::new();

    for &entry in entries {
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

fn find_duplicate_bibliography_declarations(
    declarations: &[BibliographyDecl],
    tex_files: &[SourceFile],
) -> Vec<Diagnostic> {
    if package_is_included(tex_files, "chapterbib") {
        return Vec::new();
    }

    let mut grouped = HashMap::<PathBuf, Vec<&BibliographyDecl>>::new();
    for declaration in declarations {
        let path = resolve_bib_path(declaration);
        let key = path.canonicalize().unwrap_or(path);
        grouped.entry(key).or_default().push(declaration);
    }

    let mut diagnostics = Vec::new();
    for (path, mut group) in grouped {
        if group.len() < 2 {
            continue;
        }
        group.sort_by(|left, right| {
            left.file
                .cmp(&right.file)
                .then(left.line.cmp(&right.line))
                .then(left.column.cmp(&right.column))
        });
        for declaration in group.into_iter().skip(1) {
            diagnostics.push(
                Diagnostic::new(
                    "CIT007",
                    Severity::Warning,
                    format!(
                        "bibliography file '{}' is declared multiple times",
                        path.display()
                    ),
                    &declaration.file,
                    declaration.line,
                    declaration.column,
                )
                .with_hint("remove the duplicate bibliography declaration"),
            );
        }
    }
    diagnostics
}

fn package_is_included(tex_files: &[SourceFile], package: &str) -> bool {
    tex_files
        .iter()
        .any(|file| content_includes_package(&file.content, package))
}

fn content_includes_package(content: &str, package: &str) -> bool {
    let mut offset = 0;
    while let Some(relative) = content[offset..].find("\\usepackage") {
        let command_start = offset + relative;
        if is_commented_position(content, command_start) {
            offset = command_start + "\\usepackage".len();
            continue;
        }

        let after_name = command_start + "\\usepackage".len();
        let Some((body_start, body_end)) =
            read_latex_required_arg_after_options(content, after_name)
        else {
            offset = after_name;
            continue;
        };
        if content[body_start..body_end]
            .split(',')
            .map(str::trim)
            .any(|name| name == package)
        {
            return true;
        }
        offset = body_end + 1;
    }
    false
}

fn read_latex_required_arg_after_options(
    content: &str,
    mut offset: usize,
) -> Option<(usize, usize)> {
    offset = skip_ascii_whitespace(content, offset);
    while content[offset..].starts_with('[') {
        offset = balanced_group_end(content, offset, '[', ']')? + 1;
        offset = skip_ascii_whitespace(content, offset);
    }
    if !content[offset..].starts_with('{') {
        return None;
    }
    let end = balanced_group_end(content, offset, '{', '}')?;
    Some((offset + 1, end))
}

fn find_citation_punctuation(
    citations: &[CitationUse],
    tex_files: &[SourceFile],
) -> Vec<Diagnostic> {
    let source_by_path = source_content_by_path(tex_files);
    let mut seen_commands = HashSet::new();
    let mut diagnostics = Vec::new();

    for citation in citations {
        if citation.command != "cite"
            || !seen_commands.insert((&citation.file, citation.command_start))
        {
            continue;
        }
        let Some(content) = source_by_path.get(&citation.file) else {
            continue;
        };
        if citation.command_start < 2 || citation.command_start > content.len() {
            continue;
        }
        let previous = &content[..citation.command_start];
        let Some(punctuation) = punctuation_before_citation(previous) else {
            continue;
        };
        if is_citation_punctuation_exception(previous) {
            continue;
        }

        diagnostics.push(
            Diagnostic::new(
                "CIT008",
                Severity::Warning,
                format!("\\cite is placed after punctuation '{punctuation}'"),
                &citation.file,
                citation.line,
                citation.column,
            )
            .with_hint(format!(
                "move punctuation after the citation, e.g. text~\\cite{{...}}{punctuation}"
            )),
        );
    }

    diagnostics
}

fn punctuation_before_citation(previous: &str) -> Option<char> {
    let mut chars = previous.chars().rev();
    if chars.next()? != '~' {
        return None;
    }
    let punctuation = chars.next()?;
    matches!(punctuation, '.' | ',' | '?' | '!' | ';' | ':').then_some(punctuation)
}

fn is_citation_punctuation_exception(previous: &str) -> bool {
    let line = previous
        .rsplit_once('\n')
        .map(|(_, line)| line)
        .unwrap_or(previous);
    let tail_start = line
        .char_indices()
        .rev()
        .nth(40)
        .map(|(index, _)| index)
        .unwrap_or(0);
    let tail = line[tail_start..].to_ascii_lowercase();

    [
        "e.g.~", "e.g.,~", "i.e.~", "i.e.,~", "et al.~", "et al.,~", "etc.~", "cf.~", "cf.,~",
        "vs.~", "\\eg,~", "\\eg~", "\\ie,~", "\\ie~",
    ]
    .iter()
    .any(|prefix| tail.ends_with(prefix))
        || [
            "recently,~",
            "additionally,~",
            "furthermore,~",
            "moreover,~",
            "however,~",
        ]
        .iter()
        .any(|prefix| line.trim_start().to_ascii_lowercase().ends_with(prefix))
}

fn find_collapsible_citations(
    citations: &[CitationUse],
    tex_files: &[SourceFile],
) -> Vec<Diagnostic> {
    let source_by_path = source_content_by_path(tex_files);
    let mut commands = unique_citation_commands(citations);
    commands.sort_by(|left, right| {
        left.file
            .cmp(&right.file)
            .then(left.command_start.cmp(&right.command_start))
    });

    let mut diagnostics = Vec::new();
    let mut index = 0;
    while index < commands.len() {
        let mut group_end = index + 1;
        while group_end < commands.len()
            && citations_are_collapsible(
                &commands[group_end - 1],
                &commands[group_end],
                &source_by_path,
            )
        {
            group_end += 1;
        }

        if group_end - index >= 2 {
            let anchor = &commands[index + 1];
            diagnostics.push(
                Diagnostic::new(
                    "CIT009",
                    Severity::Warning,
                    "adjacent citation commands can be collapsed",
                    &anchor.file,
                    anchor.line,
                    anchor.column,
                )
                .with_hint(format!(
                    "merge adjacent citations into one \\{}{{...}} command",
                    anchor.command
                )),
            );
        }

        index = group_end;
    }

    diagnostics
}

#[derive(Debug, Clone)]
struct CitationCommandUse {
    command: String,
    kind: CitationKind,
    file: PathBuf,
    line: usize,
    column: usize,
    is_starred: bool,
    has_optional_arg: bool,
    command_start: usize,
    command_end: usize,
}

fn unique_citation_commands(citations: &[CitationUse]) -> Vec<CitationCommandUse> {
    let mut seen = HashSet::new();
    let mut commands = Vec::new();
    for citation in citations {
        if citation.is_nocite || !seen.insert((citation.file.clone(), citation.command_start)) {
            continue;
        }
        commands.push(CitationCommandUse {
            command: citation.command.clone(),
            kind: citation.kind,
            file: citation.file.clone(),
            line: citation.line,
            column: citation.column,
            is_starred: citation.is_starred,
            has_optional_arg: citation.has_optional_arg,
            command_start: citation.command_start,
            command_end: citation.command_end,
        });
    }
    commands
}

fn citations_are_collapsible(
    left: &CitationCommandUse,
    right: &CitationCommandUse,
    source_by_path: &HashMap<PathBuf, &str>,
) -> bool {
    if left.file != right.file
        || left.command != right.command
        || left.kind != right.kind
        || left.is_starred != right.is_starred
        || left.has_optional_arg
        || right.has_optional_arg
    {
        return false;
    }

    let Some(content) = source_by_path.get(&left.file) else {
        return false;
    };
    if left.command_end > right.command_start || right.command_start > content.len() {
        return false;
    }

    content[left.command_end..right.command_start]
        .chars()
        .all(|character| character.is_whitespace() || character == '~')
}

fn source_content_by_path(tex_files: &[SourceFile]) -> HashMap<PathBuf, &str> {
    tex_files
        .iter()
        .map(|file| (file.path.clone(), file.content.as_str()))
        .collect()
}

fn skip_ascii_whitespace(content: &str, mut offset: usize) -> usize {
    while let Some(byte) = content.as_bytes().get(offset) {
        if !byte.is_ascii_whitespace() {
            break;
        }
        offset += 1;
    }
    offset
}

fn balanced_group_end(content: &str, start: usize, open: char, close: char) -> Option<usize> {
    let mut depth = 0;
    let mut escaped = false;

    for (relative, character) in content[start..].char_indices() {
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

fn is_commented_position(content: &str, byte_index: usize) -> bool {
    let line_start = content[..byte_index]
        .rfind('\n')
        .map(|index| index + 1)
        .unwrap_or(0);
    let mut escaped = false;

    for character in content[line_start..byte_index].chars() {
        if escaped {
            escaped = false;
            continue;
        }
        if character == '\\' {
            escaped = true;
            continue;
        }
        if character == '%' {
            return true;
        }
    }

    false
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;
    use std::path::Path;

    use super::{
        bibtex::parse_bib_entries, find_duplicate_keys, missing_required_fields,
        scoped_bibliography_entries,
    };

    #[test]
    fn detects_duplicate_bib_keys() {
        let entries = parse_bib_entries(
            Path::new("refs.bib"),
            r"@article{alpha, author={A}, title={Long Enough Title One}, journal={J}, year={2024}}
@misc{alpha, author={B}, title={Long Enough Title Two}, year={2024}, eprint={1}}",
        );

        let scoped_entries = entries.iter().collect::<Vec<_>>();
        let diagnostics = find_duplicate_keys(&scoped_entries);

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, "CIT005");
        assert!(diagnostics[0].message.contains("first defined"));
    }

    #[test]
    fn scopes_bibliography_entries_to_active_keys() {
        let entries = parse_bib_entries(
            Path::new("refs.bib"),
            r"@article{alpha, author={A}, title={Alpha Long Enough Title}, journal={J}, year={2024}}
@article{unused, author={B}, title={Unused Long Enough Title}, journal={J}, year={2024}}",
        );
        let cited_keys = HashSet::from(["alpha"]);
        let bbl_keys = HashSet::new();
        let source_bibitem_keys = HashSet::new();

        let scoped = scoped_bibliography_entries(
            &entries,
            &cited_keys,
            &bbl_keys,
            &source_bibitem_keys,
            false,
        );

        assert_eq!(scoped.len(), 1);
        assert_eq!(scoped[0].key, "alpha");
    }

    #[test]
    fn accepts_preprint_and_accessed_web_metadata() {
        let entries = parse_bib_entries(
            Path::new("refs.bib"),
            r"@article{preprint, author={A}, title={Preprint Title Long Enough}, year={2024}, eprint={2401.12345}, archivePrefix={arXiv}}
@misc{web, author={B}, title={Software Page Long Enough}, howpublished={\url{https://example.com}}, note={Accessed: 2024-01-01}}",
        );

        assert!(missing_required_fields(&entries[0]).is_empty());
        assert!(missing_required_fields(&entries[1]).is_empty());
    }
}
