use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::discovery::discover_tex_files;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormatMode {
    Check,
    Write,
    Diff,
}

#[derive(Debug, Clone)]
pub struct FormatOptions {
    pub paths: Vec<PathBuf>,
    pub mode: FormatMode,
}

#[derive(Debug, Clone)]
pub struct FormatResult {
    pub files_checked: usize,
    pub changes: Vec<FormatChange>,
}

impl FormatResult {
    pub fn changed_count(&self) -> usize {
        self.changes.len()
    }
}

#[derive(Debug, Clone)]
pub struct FormatChange {
    pub path: PathBuf,
    pub original: String,
    pub formatted: String,
}

pub fn run_format(options: &FormatOptions) -> io::Result<FormatResult> {
    let files = discover_format_files(&options.paths)?;
    let mut changes = Vec::new();

    for path in &files {
        let original = fs::read_to_string(path)?;
        let formatted = format_content(&original);
        if formatted == original {
            continue;
        }

        if options.mode == FormatMode::Write {
            fs::write(path, &formatted)?;
        }

        changes.push(FormatChange {
            path: path.clone(),
            original,
            formatted,
        });
    }

    Ok(FormatResult {
        files_checked: files.len(),
        changes,
    })
}

fn discover_format_files(paths: &[PathBuf]) -> io::Result<Vec<PathBuf>> {
    let mut files = discover_tex_files(paths, true)?;
    for path in paths {
        if path.is_file() && is_bib_file(path) && !files.contains(path) {
            files.push(path.clone());
        }
    }
    files.sort();
    files.dedup();
    Ok(files)
}

fn is_bib_file(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("bib"))
}

pub fn render_format_text(result: &FormatResult, mode: FormatMode) -> String {
    let action = match mode {
        FormatMode::Check => "would change",
        FormatMode::Write => "changed",
        FormatMode::Diff => "would change",
    };
    format!(
        "checked {} file(s), {} file(s) {action}\n",
        result.files_checked,
        result.changed_count()
    )
}

pub fn render_format_diff(result: &FormatResult) -> String {
    let mut output = String::new();
    for change in &result.changes {
        output.push_str(&format!("--- {}\n", change.path.display()));
        output.push_str(&format!("+++ {}\n", change.path.display()));
        output.push_str("@@\n");
        for line in diff_lines(&change.original) {
            output.push_str(&format!("-{line}\n"));
        }
        for line in diff_lines(&change.formatted) {
            output.push_str(&format!("+{line}\n"));
        }
    }
    output.push_str(&render_format_text(result, FormatMode::Diff));
    output
}

fn diff_lines(content: &str) -> Vec<&str> {
    if content.is_empty() {
        Vec::new()
    } else {
        content.trim_end_matches(['\r', '\n']).split('\n').collect()
    }
}

fn format_content(content: &str) -> String {
    if content.is_empty() {
        return String::new();
    }

    let newline = if content.contains("\r\n") {
        "\r\n"
    } else {
        "\n"
    };
    let mut lines: Vec<&str> = content.split('\n').collect();
    if content.ends_with('\n') {
        lines.pop();
    }

    let mut formatted_lines = Vec::new();
    let mut blank_run = 0usize;
    for line in lines {
        let line = line.strip_suffix('\r').unwrap_or(line);
        let trimmed = line.trim_end_matches([' ', '\t']);
        if trimmed.is_empty() {
            blank_run += 1;
            if blank_run > 2 {
                continue;
            }
        } else {
            blank_run = 0;
        }
        formatted_lines.push(trimmed);
    }

    let mut formatted = formatted_lines.join(newline);
    formatted.push_str(newline);
    formatted
}

#[cfg(test)]
mod tests {
    use super::format_content;

    #[test]
    fn removes_trailing_whitespace_and_adds_final_newline() {
        assert_eq!(format_content("A  \nB\t"), "A\nB\n");
    }

    #[test]
    fn collapses_more_than_two_blank_lines() {
        assert_eq!(format_content("A\n\n\n\nB\n"), "A\n\n\nB\n");
    }

    #[test]
    fn preserves_crlf_when_present() {
        assert_eq!(format_content("A  \r\nB"), "A\r\nB\r\n");
    }

    #[test]
    fn leaves_empty_file_empty() {
        assert_eq!(format_content(""), "");
    }
}
