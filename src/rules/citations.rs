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
use latex::{find_bbl_inputs, find_bibliographies, find_citations, uses_bibunits};
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

#[derive(Debug, Clone)]
struct BibSource {
    path: PathBuf,
    is_explicit: bool,
    size_bytes: Option<u64>,
}

pub fn check_project(
    tex_files: &[SourceFile],
    explicit_bib_files: &[PathBuf],
) -> Result<Vec<Diagnostic>, io::Error> {
    let mut citations = Vec::new();
    let mut declarations = Vec::new();
    let mut bbl_inputs = Vec::new();
    let mut has_bibunits = false;
    let mut source_bibitem_keys = HashSet::new();

    for file in tex_files {
        citations.extend(find_citations(&file.path, &file.content));
        declarations.extend(find_bibliographies(&file.path, &file.content));
        bbl_inputs.extend(find_bbl_inputs(&file.path, &file.content));
        has_bibunits |= uses_bibunits(&file.content);
        source_bibitem_keys.extend(parse_bbl_keys(&file.content));
    }

    let mut diagnostics = Vec::new();
    diagnostics.extend(find_duplicate_bibliography_declarations(
        &declarations,
        tex_files,
    ));
    diagnostics.extend(find_citation_punctuation(&citations, tex_files));
    diagnostics.extend(find_collapsible_citations(&citations, tex_files));
    diagnostics.extend(find_mixed_citation_command_families(&citations));

    let bib_sources = ordered_bib_sources(
        bibliography_paths(&declarations, explicit_bib_files),
        explicit_bib_files,
    );
    let mut bbl_keys = HashSet::new();
    let source_files: Vec<_> = tex_files.iter().map(|file| file.path.clone()).collect();

    bbl_keys.extend(parse_bbl_input_keys(&bbl_inputs));
    if has_bibunits {
        bbl_keys.extend(parse_bibunit_bbl_keys(&source_files));
    }

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
    let mut resolved_active_keys = HashSet::new();
    for bib_source in bib_sources {
        let bib_path = &bib_source.path;
        if should_skip_large_bib_with_bbl(
            bib_path,
            bib_source.is_explicit,
            has_nocite_all,
            &bbl_keys,
        ) {
            continue;
        }
        if should_skip_resolved_large_bib(
            &bib_source,
            active_key_filter.as_ref(),
            &resolved_active_keys,
            has_nocite_all,
        ) {
            continue;
        }

        match fs::read_to_string(bib_path) {
            Ok(content) => {
                let parsed =
                    parse_bib_entries_with_filter(bib_path, &content, active_key_filter.as_ref());
                resolved_active_keys.extend(
                    parsed
                        .iter()
                        .filter(|entry| {
                            active_key_filter
                                .as_ref()
                                .is_some_and(|keys| keys.contains(entry.key.as_str()))
                        })
                        .map(|entry| entry.key.clone()),
                );
                entries.extend(parsed);
            }
            Err(error) if explicit_bib_files.iter().any(|path| path == bib_path) => {
                return Err(error);
            }
            Err(_) => {
                if let Some(declaration) = declarations
                    .iter()
                    .find(|declaration| resolve_bib_path(declaration) == *bib_path)
                {
                    let fallback_keys = parse_bbl_fallback_keys(declaration, &source_files);
                    if let Some((alternate_path, content)) =
                        read_first_existing_bib(declaration, &source_files)
                    {
                        let parsed = parse_bib_entries_with_filter(
                            &alternate_path,
                            &content,
                            active_key_filter.as_ref(),
                        );
                        resolved_active_keys.extend(
                            parsed
                                .iter()
                                .filter(|entry| {
                                    active_key_filter
                                        .as_ref()
                                        .is_some_and(|keys| keys.contains(entry.key.as_str()))
                                })
                                .map(|entry| entry.key.clone()),
                        );
                        entries.extend(parsed);
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

    diagnostics.extend(find_invalid_bibliography_metadata(&scoped_entries));
    diagnostics.extend(find_bibliography_style_policy(&scoped_entries));
    diagnostics.extend(find_duplicate_keys(&scoped_entries));
    diagnostics.extend(find_similar_titles(&scoped_entries));
    diagnostics.extend(find_non_canonical_bibliography_keys(&scoped_entries));

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

fn ordered_bib_sources(paths: Vec<PathBuf>, explicit_bib_files: &[PathBuf]) -> Vec<BibSource> {
    let mut sources = paths
        .into_iter()
        .enumerate()
        .map(|(index, path)| {
            let size_bytes = fs::metadata(&path).ok().map(|metadata| metadata.len());
            let is_explicit = explicit_bib_files.iter().any(|explicit| explicit == &path);
            (
                index,
                BibSource {
                    path,
                    is_explicit,
                    size_bytes,
                },
            )
        })
        .collect::<Vec<_>>();

    sources.sort_by_key(|(index, source)| (source.size_bytes.unwrap_or(u64::MAX), *index));
    sources.into_iter().map(|(_, source)| source).collect()
}

fn should_skip_resolved_large_bib(
    source: &BibSource,
    active_keys: Option<&HashSet<String>>,
    resolved_active_keys: &HashSet<String>,
    has_nocite_all: bool,
) -> bool {
    if source.is_explicit || has_nocite_all {
        return false;
    }
    if source
        .size_bytes
        .is_none_or(|size| size <= LARGE_BIB_FAST_PATH_BYTES)
    {
        return false;
    }

    let Some(active_keys) = active_keys else {
        return false;
    };
    !active_keys.is_empty() && active_keys.is_subset(resolved_active_keys)
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

fn parse_bbl_input_keys(paths: &[PathBuf]) -> Vec<String> {
    paths
        .iter()
        .filter_map(|path| fs::read_to_string(path).ok())
        .flat_map(|content| parse_bbl_keys(&content))
        .collect()
}

fn parse_bibunit_bbl_keys(source_files: &[PathBuf]) -> Vec<String> {
    let mut directories: Vec<_> = source_files
        .iter()
        .filter_map(|path| path.parent().map(Path::to_path_buf))
        .collect();
    directories.sort();
    directories.dedup();

    directories
        .into_iter()
        .filter_map(|directory| fs::read_dir(directory).ok())
        .flat_map(|entries| entries.filter_map(Result::ok))
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| {
                    name.starts_with("bu")
                        && name.ends_with(".bbl")
                        && name[2..name.len() - ".bbl".len()]
                            .chars()
                            .all(|character| character.is_ascii_digit())
                })
        })
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

fn find_invalid_bibliography_metadata(entries: &[&BibEntry]) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();

    for &entry in entries {
        for (field, value) in &entry.fields {
            let normalized = normalize_braced_value(value);
            let message = match field.as_str() {
                "doi" if !is_valid_doi(&normalized) => Some(format!(
                    "bibliography entry '{}' has invalid DOI '{}'",
                    entry.key, normalized
                )),
                "url" if !is_valid_url(&normalized) => Some(format!(
                    "bibliography entry '{}' has invalid URL '{}'",
                    entry.key, normalized
                )),
                "eprint" if entry_has_arxiv_prefix(entry) && !is_valid_arxiv_id(&normalized) => {
                    Some(format!(
                        "bibliography entry '{}' has invalid arXiv id '{}'",
                        entry.key, normalized
                    ))
                }
                _ => None,
            };

            if let Some(message) = message {
                diagnostics.push(
                    Diagnostic::new(
                        "BIB001",
                        Severity::Warning,
                        message,
                        &entry.file,
                        entry.line,
                        entry.column,
                    )
                    .with_hint("fix the bibliography identifier syntax"),
                );
            }
        }
    }

    diagnostics
}

fn find_bibliography_style_policy(entries: &[&BibEntry]) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();

    for &entry in entries {
        for field in entry.fields.keys() {
            if !is_forbidden_bibliography_field(field) {
                continue;
            }

            diagnostics.push(
                Diagnostic::new(
                    "BIB002",
                    Severity::Warning,
                    format!(
                        "bibliography entry '{}' contains private field '{}'",
                        entry.key, field
                    ),
                    &entry.file,
                    entry.line,
                    entry.column,
                )
                .with_hint("remove local-only bibliography fields before submission"),
            );
        }
    }

    diagnostics
}

fn is_forbidden_bibliography_field(field: &str) -> bool {
    matches!(field, "file")
}

fn normalize_braced_value(value: &str) -> String {
    value
        .trim()
        .trim_start_matches('{')
        .trim_end_matches('}')
        .trim()
        .to_string()
}

fn is_valid_doi(value: &str) -> bool {
    let doi = value
        .strip_prefix("https://doi.org/")
        .or_else(|| value.strip_prefix("http://doi.org/"))
        .or_else(|| value.strip_prefix("doi:"))
        .unwrap_or(value);

    doi.starts_with("10.")
        && doi.contains('/')
        && !doi.chars().any(char::is_whitespace)
        && doi.split_once('/').is_some_and(|(prefix, suffix)| {
            prefix.len() > 3
                && prefix[3..].chars().all(|ch| ch.is_ascii_digit())
                && !suffix.is_empty()
        })
}

fn is_valid_url(value: &str) -> bool {
    if value.starts_with("\\url{") && value.ends_with('}') {
        return true;
    }

    let Some((scheme, rest)) = value.split_once(':') else {
        return false;
    };
    matches!(scheme, "http" | "https" | "ftp" | "mailto")
        && !rest.is_empty()
        && !value.chars().any(char::is_whitespace)
}

fn entry_has_arxiv_prefix(entry: &BibEntry) -> bool {
    entry
        .fields
        .get("archiveprefix")
        .or_else(|| entry.fields.get("archivePrefix"))
        .is_some_and(|value| normalize_braced_value(value).eq_ignore_ascii_case("arxiv"))
}

fn is_valid_arxiv_id(value: &str) -> bool {
    let value = value.strip_prefix("arXiv:").unwrap_or(value);
    is_new_arxiv_id(value) || is_old_arxiv_id(value)
}

fn is_new_arxiv_id(value: &str) -> bool {
    let (main, version) = split_arxiv_version(value);
    if version.is_some_and(|version| !is_positive_version(version)) {
        return false;
    }

    let Some((year_month, number)) = main.split_once('.') else {
        return false;
    };
    year_month.len() == 4
        && year_month.chars().all(|ch| ch.is_ascii_digit())
        && matches!(number.len(), 4 | 5)
        && number.chars().all(|ch| ch.is_ascii_digit())
}

fn is_old_arxiv_id(value: &str) -> bool {
    let (main, version) = split_arxiv_version(value);
    if version.is_some_and(|version| !is_positive_version(version)) {
        return false;
    }

    let Some((archive, number)) = main.split_once('/') else {
        return false;
    };
    !archive.is_empty()
        && archive
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '.'))
        && number.len() == 7
        && number.chars().all(|ch| ch.is_ascii_digit())
}

fn split_arxiv_version(value: &str) -> (&str, Option<&str>) {
    let Some(index) = value.rfind('v') else {
        return (value, None);
    };
    let version = &value[index + 1..];
    if version.is_empty() || !version.chars().all(|ch| ch.is_ascii_digit()) {
        return (value, None);
    }
    (&value[..index], Some(version))
}

fn is_positive_version(version: &str) -> bool {
    version.parse::<usize>().is_ok_and(|version| version > 0)
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

fn find_non_canonical_bibliography_keys(entries: &[&BibEntry]) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();

    for &entry in entries {
        let Some(expected) = canonical_bibliography_key(entry) else {
            continue;
        };

        if !bibliography_key_matches_canonical(entry.key.as_str(), expected.as_str()) {
            diagnostics.push(
                Diagnostic::new(
                    "CIT011",
                    Severity::Warning,
                    format!(
                        "bibliography key '{}' does not follow canonical key '{}'",
                        entry.key, expected
                    ),
                    &entry.file,
                    entry.line,
                    entry.column,
                )
                .with_hint(format!(
                    "rename '{}' and matching citations to '{}'",
                    entry.key, expected
                )),
            );
        }
    }

    diagnostics
}

fn canonical_bibliography_key(entry: &BibEntry) -> Option<String> {
    let author = entry
        .fields
        .get("author")
        .or_else(|| entry.fields.get("editor"))?;
    let year = entry
        .fields
        .get("year")
        .or_else(|| entry.fields.get("date"))
        .and_then(|value| first_four_digit_year(value))?;
    let title = entry.fields.get("title")?;

    let surname = first_author_surname(author)?;
    let title_word = first_title_word(title)?;

    Some(format!("{surname}{year}{title_word}"))
}

fn bibliography_key_matches_canonical(key: &str, canonical: &str) -> bool {
    if key == canonical {
        return true;
    }

    let Some((key_prefix, key_title)) = split_key_title_part(key) else {
        return false;
    };
    let Some((canonical_prefix, canonical_title)) = split_key_title_part(canonical) else {
        return false;
    };

    key_prefix == canonical_prefix
        && canonical_title.starts_with(key_title)
        && (key_title.len() >= 4 || key_title.len() == canonical_title.len())
}

fn split_key_title_part(key: &str) -> Option<(&str, &str)> {
    let year_start = key
        .as_bytes()
        .windows(4)
        .position(|window| window.iter().all(u8::is_ascii_digit))?;
    let title_start = year_start + 4;
    if title_start >= key.len() {
        return None;
    }
    Some((&key[..title_start], &key[title_start..]))
}

fn first_four_digit_year(value: &str) -> Option<&str> {
    value
        .as_bytes()
        .windows(4)
        .position(|window| window.iter().all(u8::is_ascii_digit))
        .map(|start| &value[start..start + 4])
}

fn first_author_surname(value: &str) -> Option<String> {
    let first_author = value.split(" and ").next().unwrap_or(value);
    let surname = first_author
        .split_once(',')
        .map(|(surname, _)| surname)
        .unwrap_or_else(|| {
            first_author
                .split_whitespace()
                .last()
                .unwrap_or(first_author)
        });

    normalize_key_component(surname)
}

fn first_title_word(value: &str) -> Option<String> {
    normalize_key_component(value).and_then(|normalized| {
        normalized
            .split_whitespace()
            .find(|word| !is_leading_title_stopword(word))
            .map(str::to_string)
    })
}

fn is_leading_title_stopword(word: &str) -> bool {
    matches!(word, "a" | "an" | "the" | "on")
}

fn normalize_key_component(value: &str) -> Option<String> {
    let mut output = String::new();
    let mut command = false;

    for character in value.chars() {
        if character == '\\' {
            command = true;
            continue;
        }

        if command {
            if character.is_ascii_alphabetic() {
                continue;
            }
            command = false;
        }

        if character == '-' {
            continue;
        }

        if character.is_ascii_alphanumeric() {
            output.push(character.to_ascii_lowercase());
        } else if !output.ends_with(' ') {
            output.push(' ');
        }
    }

    let normalized = output.trim();
    (!normalized.is_empty()).then(|| normalized.to_string())
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
        if is_citation_punctuation_exception(previous, punctuation) {
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

fn is_citation_punctuation_exception(previous: &str, punctuation: char) -> bool {
    let line = previous
        .rsplit_once('\n')
        .map(|(_, line)| line)
        .unwrap_or(previous);
    if punctuation == ',' && comma_before_citation_exception(line) {
        return true;
    }

    let tail_start = line
        .char_indices()
        .rev()
        .nth(40)
        .map(|(index, _)| index)
        .unwrap_or(0);
    let tail = line[tail_start..].to_ascii_lowercase();

    [
        "e.g.~",
        "e.g.,~",
        "i.e.~",
        "i.e.,~",
        "et al.~",
        "et al.,~",
        "et~al.~",
        "et~al.,~",
        "\\etal.~",
        "\\etal.,~",
        "etc.~",
        "cf.~",
        "cf.,~",
        "vs.~",
        "\\eg,~",
        "\\eg~",
        "\\ie,~",
        "\\ie~",
    ]
    .iter()
    .any(|prefix| tail.ends_with(prefix))
        || [
            "recently,~",
            "additionally,~",
            "conversely,~",
            "furthermore,~",
            "moreover,~",
            "however,~",
        ]
        .iter()
        .any(|prefix| line.trim_start().to_ascii_lowercase().ends_with(prefix))
}

fn comma_before_citation_exception(line: &str) -> bool {
    let Some(before_tilde) = line.strip_suffix('~') else {
        return false;
    };
    let Some(before_comma) = before_tilde.strip_suffix(',') else {
        return false;
    };
    let before_comma = before_comma.trim_end();
    let tail_start = before_comma
        .char_indices()
        .rev()
        .nth(80)
        .map(|(index, _)| index)
        .unwrap_or(0);
    let tail = &before_comma[tail_start..];

    tail.contains("\\cite{")
        || tail.contains("\\citep{")
        || tail.contains("\\citet{")
        || tail.contains("\\parencite{")
        || tail.contains("\\textcite{")
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

fn find_mixed_citation_command_families(citations: &[CitationUse]) -> Vec<Diagnostic> {
    let mut commands = unique_citation_commands(citations);
    commands.sort_by(|left, right| {
        left.file
            .cmp(&right.file)
            .then(left.command_start.cmp(&right.command_start))
    });

    let mut first_by_family: HashMap<CitationCommandFamily, CitationCommandUse> = HashMap::new();

    for command in commands {
        let Some(family) = citation_command_family(&command.command) else {
            continue;
        };

        if let Some((other_family, first)) = first_by_family
            .iter()
            .find(|(other_family, _)| **other_family != family)
        {
            return vec![Diagnostic::new(
                "CIT010",
                Severity::Warning,
                format!(
                    "mixed citation command families: {} '\\{}' and {} '\\{}'",
                    other_family.name(),
                    first.command,
                    family.name(),
                    command.command
                ),
                &command.file,
                command.line,
                command.column,
            )
            .with_hint("use one citation package command family consistently")];
        }

        first_by_family.entry(family).or_insert(command);
    }

    Vec::new()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum CitationCommandFamily {
    Natbib,
    Biblatex,
}

impl CitationCommandFamily {
    fn name(self) -> &'static str {
        match self {
            CitationCommandFamily::Natbib => "natbib",
            CitationCommandFamily::Biblatex => "biblatex",
        }
    }
}

fn citation_command_family(command: &str) -> Option<CitationCommandFamily> {
    match command {
        "citep" | "Citep" | "citealp" | "Citealp" | "citealt" | "Citealt" | "citet" | "Citet"
        | "citeyearpar" | "Citeyearpar" => Some(CitationCommandFamily::Natbib),
        "parencite" | "Parencite" | "parencites" | "Parencites" | "textcite" | "Textcite"
        | "textcites" | "Textcites" | "autocite" | "Autocite" | "autocites" | "Autocites"
        | "smartcite" | "Smartcite" | "smartcites" | "Smartcites" | "footcite" | "Footcite"
        | "footcites" | "supercite" | "Supercite" | "supercites" => {
            Some(CitationCommandFamily::Biblatex)
        }
        _ => None,
    }
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
    use std::path::{Path, PathBuf};

    use super::{
        bibtex::parse_bib_entries, canonical_bibliography_key, find_duplicate_keys,
        find_non_canonical_bibliography_keys, is_citation_punctuation_exception, is_valid_arxiv_id,
        is_valid_doi, is_valid_url, missing_required_fields, scoped_bibliography_entries,
        should_skip_resolved_large_bib, BibSource, LARGE_BIB_FAST_PATH_BYTES,
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
    fn builds_google_scholar_style_bibliography_key() {
        let entries = parse_bib_entries(
            Path::new("refs.bib"),
            r"@inproceedings{skrynnik2024learn,
  title={Learn to follow: Decentralized lifelong multi-agent pathfinding via planning and learning},
  author={Skrynnik, Alexey and Andreychuk, Anton and Nesterova, Maria},
  booktitle={Proceedings of the AAAI conference on artificial intelligence},
  year={2024}
}",
        );

        assert_eq!(
            canonical_bibliography_key(&entries[0]).as_deref(),
            Some("skrynnik2024learn")
        );
    }

    #[test]
    fn detects_non_canonical_bibliography_keys() {
        let entries = parse_bib_entries(
            Path::new("refs.bib"),
            r"@article{paper1, author={Jane Smith}, title={Efficient Planning}, journal={J}, year={2024}}",
        );
        let scoped_entries = entries.iter().collect::<Vec<_>>();
        let diagnostics = find_non_canonical_bibliography_keys(&scoped_entries);

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, "CIT011");
        assert!(diagnostics[0]
            .message
            .contains("canonical key 'smith2024efficient'"));
    }

    #[test]
    fn accepts_canonical_bibliography_keys() {
        let entries = parse_bib_entries(
            Path::new("refs.bib"),
            r"@article{smith2024efficient, author={Jane Smith}, title={Efficient Planning}, journal={J}, year={2024}}",
        );
        let scoped_entries = entries.iter().collect::<Vec<_>>();

        assert!(find_non_canonical_bibliography_keys(&scoped_entries).is_empty());
    }

    #[test]
    fn skips_leading_title_articles_for_canonical_keys() {
        let entries = parse_bib_entries(
            Path::new("refs.bib"),
            r"@article{silver2018general,
  title={A general reinforcement learning algorithm that masters chess, shogi, and Go through self-play},
  author={Silver, David},
  journal={Science},
  year={2018}
}",
        );

        assert_eq!(
            canonical_bibliography_key(&entries[0]).as_deref(),
            Some("silver2018general")
        );
    }

    #[test]
    fn skips_leading_on_the_for_canonical_keys() {
        let entries = parse_bib_entries(
            Path::new("refs.bib"),
            r"@inproceedings{pallagani2024prospects,
  title={On the Prospects of Incorporating Large Language Models ({LLMs}) in Automated Planning and Scheduling ({APS})},
  author={Pallagani, Vishal},
  booktitle={Proceedings of the International Conference on Automated Planning and Scheduling},
  year={2024}
}",
        );

        assert_eq!(
            canonical_bibliography_key(&entries[0]).as_deref(),
            Some("pallagani2024prospects")
        );
    }

    #[test]
    fn joins_hyphenated_title_words_for_canonical_keys() {
        let entries = parse_bib_entries(
            Path::new("refs.bib"),
            r"@inproceedings{wang2023selfconsistency,
  title={Self-Consistency Improves Chain of Thought Reasoning in Language Models},
  author={Xuezhi Wang and Jason Wei},
  booktitle={The Eleventh International Conference on Learning Representations},
  year={2023}
}",
        );

        assert_eq!(
            canonical_bibliography_key(&entries[0]).as_deref(),
            Some("wang2023selfconsistency")
        );
    }

    #[test]
    fn accepts_shortened_compound_title_words_for_canonical_keys() {
        let entries = parse_bib_entries(
            Path::new("refs.bib"),
            r"@article{wei2022chain,
  title={Chain-of-Thought Prompting Elicits Reasoning in Large Language Models},
  author={Wei, Jason},
  journal={NeurIPS},
  year={2022}
}
@article{guo2025deepseek,
  title={DeepSeek-R1: Incentivizing Reasoning Capability in LLMs via Reinforcement Learning},
  author={Guo, Daya},
  journal={arXiv},
  year={2025}
}",
        );
        let scoped_entries = entries.iter().collect::<Vec<_>>();

        assert!(find_non_canonical_bibliography_keys(&scoped_entries).is_empty());
    }

    #[test]
    fn skips_canonical_key_check_when_metadata_is_missing() {
        let entries = parse_bib_entries(
            Path::new("refs.bib"),
            r"@article{paper1, author={Jane Smith}, journal={J}, year={2024}}",
        );

        assert!(canonical_bibliography_key(&entries[0]).is_none());
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

    #[test]
    fn validates_bibliography_identifier_syntax() {
        assert!(is_valid_doi("10.1145/1234567.8901234"));
        assert!(is_valid_doi("https://doi.org/10.1000/example"));
        assert!(!is_valid_doi("not-a-doi"));

        assert!(is_valid_url("https://example.com/paper"));
        assert!(is_valid_url("mailto:author@example.com"));
        assert!(!is_valid_url("example dot com"));

        assert!(is_valid_arxiv_id("2401.00001v2"));
        assert!(is_valid_arxiv_id("hep-th/9901001"));
        assert!(!is_valid_arxiv_id("bad-id"));
    }

    #[test]
    fn citation_punctuation_allows_comma_separated_citation_lists() {
        assert!(is_citation_punctuation_exception(
            r"Many works~\cite{first},~",
            ','
        ));
    }

    #[test]
    fn citation_punctuation_allows_introductory_conversely() {
        assert!(is_citation_punctuation_exception(r"Conversely,~", ','));
    }

    #[test]
    fn citation_punctuation_keeps_sentence_comma_warning() {
        assert!(!is_citation_punctuation_exception(r"This claim,~", ','));
    }

    #[test]
    fn skips_large_bib_after_active_keys_are_resolved() {
        let source = BibSource {
            path: PathBuf::from("huge.bib"),
            is_explicit: false,
            size_bytes: Some(LARGE_BIB_FAST_PATH_BYTES + 1),
        };
        let active_keys = HashSet::from(["alpha".to_string(), "beta".to_string()]);
        let resolved_active_keys = HashSet::from(["alpha".to_string(), "beta".to_string()]);

        assert!(should_skip_resolved_large_bib(
            &source,
            Some(&active_keys),
            &resolved_active_keys,
            false,
        ));
    }

    #[test]
    fn does_not_skip_explicit_large_bib_after_active_keys_are_resolved() {
        let source = BibSource {
            path: PathBuf::from("huge.bib"),
            is_explicit: true,
            size_bytes: Some(LARGE_BIB_FAST_PATH_BYTES + 1),
        };
        let active_keys = HashSet::from(["alpha".to_string()]);
        let resolved_active_keys = HashSet::from(["alpha".to_string()]);

        assert!(!should_skip_resolved_large_bib(
            &source,
            Some(&active_keys),
            &resolved_active_keys,
            false,
        ));
    }
}
