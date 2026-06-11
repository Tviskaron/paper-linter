use std::collections::{HashMap, HashSet};
use std::fs;
use std::io;
use std::path::PathBuf;

use crate::diagnostic::{Diagnostic, Severity};

mod bibtex;
mod latex;
mod paths;
mod similarity;
mod syntax;

use bibtex::parse_bib_entries;
use latex::{find_bibliographies, find_citations};
pub use paths::explicit_bib_files;
use paths::{bibliography_paths, resolve_bib_path};
use similarity::find_similar_titles;

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

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{bibtex::parse_bib_entries, find_duplicate_keys};

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
}
