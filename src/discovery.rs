use std::fs;
use std::io;
use std::path::{Path, PathBuf};

pub fn discover_tex_files(paths: &[PathBuf], all_tex: bool) -> io::Result<Vec<PathBuf>> {
    let mut files = Vec::new();

    for path in paths {
        if path.is_file() {
            if is_tex_file(path) {
                if all_tex {
                    files.push(path.clone());
                } else {
                    collect_reachable_tex_files(path, &mut files)?;
                }
            }
        } else if path.is_dir() {
            if all_tex {
                collect_tex_files(path, &mut files)?;
            } else {
                collect_project_tex_files(path, &mut files)?;
            }
        } else {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("path does not exist: {}", path.display()),
            ));
        }
    }

    files.sort();
    files.dedup();
    Ok(files)
}

fn collect_project_tex_files(dir: &Path, files: &mut Vec<PathBuf>) -> io::Result<()> {
    let mut candidates = Vec::new();
    collect_tex_files(dir, &mut candidates)?;

    let roots = primary_roots(dir, &candidates)?;
    if roots.is_empty() {
        files.extend(candidates);
        return Ok(());
    }

    for root in roots {
        collect_reachable_tex_files(&root, files)?;
    }

    Ok(())
}

fn collect_tex_files(dir: &Path, files: &mut Vec<PathBuf>) -> io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_dir() {
            collect_tex_files(&path, files)?;
        } else if path.is_file() && is_tex_file(&path) {
            files.push(path);
        }
    }

    Ok(())
}

fn is_tex_file(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("tex"))
}

fn primary_roots(dir: &Path, candidates: &[PathBuf]) -> io::Result<Vec<PathBuf>> {
    let mut document_roots = Vec::new();
    for candidate in candidates {
        let content = fs::read_to_string(candidate)?;
        if declares_document(&content) {
            document_roots.push(candidate.clone());
        }
    }

    if document_roots.is_empty() {
        return Ok(Vec::new());
    }

    let roots_with_bbl: Vec<_> = document_roots
        .iter()
        .filter(|path| matching_bbl_path(path).is_file())
        .cloned()
        .collect();
    if !roots_with_bbl.is_empty() {
        return Ok(roots_with_bbl);
    }

    let main_like: Vec<_> = document_roots
        .iter()
        .filter(|path| {
            path.parent() == Some(dir)
                && path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(is_main_like_name)
        })
        .cloned()
        .collect();
    if !main_like.is_empty() {
        return Ok(main_like);
    }

    let top_level: Vec<_> = document_roots
        .iter()
        .filter(|path| path.parent() == Some(dir))
        .cloned()
        .collect();
    if !top_level.is_empty() {
        return Ok(top_level);
    }

    Ok(document_roots)
}

fn matching_bbl_path(path: &Path) -> PathBuf {
    let mut bbl = path.to_path_buf();
    bbl.set_extension("bbl");
    bbl
}

fn collect_reachable_tex_files(root: &Path, files: &mut Vec<PathBuf>) -> io::Result<()> {
    if files.iter().any(|file| file == root) {
        return Ok(());
    }

    let content = fs::read_to_string(root)?;
    files.push(root.to_path_buf());

    for input in find_input_paths(&content) {
        let path = resolve_tex_input(root, &input);
        if path.is_file() {
            collect_reachable_tex_files(&path, files)?;
        }
    }

    Ok(())
}

fn declares_document(content: &str) -> bool {
    content.contains("\\documentclass") || content.contains("\\begin{document}")
}

fn is_main_like_name(name: &str) -> bool {
    matches!(
        name.to_ascii_lowercase().as_str(),
        "main.tex" | "paper.tex" | "article.tex" | "manuscript.tex" | "ms.tex"
    )
}

fn find_input_paths(content: &str) -> Vec<String> {
    let mut paths = Vec::new();
    let mut offset = 0;

    while let Some(relative) = content[offset..].find('\\') {
        let start = offset + relative;
        if is_commented_position(content, start) {
            offset = start + 1;
            continue;
        }

        let Some((name, after_name)) = read_command_name(content, start) else {
            offset = start + 1;
            continue;
        };

        if !matches!(name, "input" | "include" | "subfile") {
            offset = after_name;
            continue;
        }

        let Some((body_start, body_end)) = read_required_argument(content, after_name) else {
            offset = after_name;
            continue;
        };

        let input = content[body_start..body_end].trim();
        if !input.is_empty() {
            paths.push(input.to_string());
        }

        offset = body_end + 1;
    }

    paths
}

fn resolve_tex_input(root: &Path, input: &str) -> PathBuf {
    let mut path = root
        .parent()
        .unwrap_or_else(|| Path::new(""))
        .join(input.trim());
    if path.extension().is_none() {
        path.set_extension("tex");
    }
    path
}

fn read_command_name(content: &str, slash_index: usize) -> Option<(&str, usize)> {
    let command_start = slash_index + 1;
    let mut command_end = command_start;

    for (index, character) in content[command_start..].char_indices() {
        if character.is_ascii_alphabetic() {
            command_end = command_start + index + character.len_utf8();
        } else {
            break;
        }
    }

    if command_end == command_start {
        None
    } else {
        Some((&content[command_start..command_end], command_end))
    }
}

fn read_required_argument(content: &str, mut offset: usize) -> Option<(usize, usize)> {
    offset = skip_ascii_whitespace(content, offset);
    if !content[offset..].starts_with('{') {
        return None;
    }

    let end = balanced_group_end(content, offset, '{', '}')?;
    Some((offset + 1, end))
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
