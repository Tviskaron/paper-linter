use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::project_graph::ProjectGraph;

pub fn discover_tex_files(paths: &[PathBuf], all_tex: bool) -> io::Result<Vec<PathBuf>> {
    let mut files = Vec::new();

    for path in paths {
        if path.is_file() {
            if is_tex_file(path) {
                if all_tex {
                    files.push(path.clone());
                } else if let Some(root) = magic_root_path(path)? {
                    let project_root = root.parent().unwrap_or_else(|| Path::new(""));
                    collect_reachable_tex_files(project_root, &root, &mut files)?;
                } else {
                    let project_root = path.parent().unwrap_or_else(|| Path::new(""));
                    collect_reachable_tex_files(project_root, path, &mut files)?;
                }
            }
        } else if path.is_dir() {
            if all_tex {
                collect_tex_files(path, &mut files)?;
            } else {
                let graph = ProjectGraph::analyze(path)?;
                files.extend(graph.tex_files_for_lint(false));
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

fn magic_root_path(path: &Path) -> io::Result<Option<PathBuf>> {
    let content = fs::read_to_string(path)?;
    let Some(raw_root) = find_magic_root(&content) else {
        return Ok(None);
    };

    let mut root = path
        .parent()
        .unwrap_or_else(|| Path::new(""))
        .join(raw_root.trim());
    if root.extension().is_none() {
        root.set_extension("tex");
    }

    if root.is_file() {
        return root.canonicalize().map(Some);
    }

    Ok(None)
}

fn collect_reachable_tex_files(
    project_root: &Path,
    current_file: &Path,
    files: &mut Vec<PathBuf>,
) -> io::Result<()> {
    if files.iter().any(|file| file == current_file) {
        return Ok(());
    }

    let content = fs::read_to_string(current_file)?;
    files.push(current_file.to_path_buf());

    for input in find_input_paths(&content) {
        let path = resolve_tex_input(project_root, current_file, &input);
        if path.is_file() {
            collect_reachable_tex_files(project_root, &path, files)?;
        }
    }

    Ok(())
}

fn find_magic_root(content: &str) -> Option<String> {
    for line in content.lines() {
        let trimmed = line.trim_start();
        let Some(comment) = trimmed.strip_prefix('%') else {
            continue;
        };
        let comment = comment.trim_start();
        let comment = comment.strip_prefix('!').unwrap_or(comment).trim_start();
        let lower = comment.to_ascii_lowercase();
        if !lower.starts_with("tex root") {
            continue;
        }

        let (_, value) = comment.split_once('=')?;
        let value = value.trim().trim_matches('"').trim_matches('\'');
        if !value.is_empty() {
            return Some(value.to_string());
        }
    }

    None
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

        let Some((input, end)) = read_input_argument(content, after_name) else {
            offset = after_name;
            continue;
        };

        if !input.is_empty() {
            paths.push(input);
        }

        offset = end;
    }

    paths
}

fn resolve_tex_input(project_root: &Path, current_file: &Path, input: &str) -> PathBuf {
    let raw = Path::new(input.trim());
    let bases = [
        current_file
            .parent()
            .unwrap_or_else(|| Path::new(""))
            .to_path_buf(),
        project_root.to_path_buf(),
    ];

    for base in bases {
        for candidate in tex_input_candidates(&base, raw) {
            if candidate.is_file() {
                return candidate;
            }
        }
    }

    PathBuf::new()
}

fn tex_input_candidates(base: &Path, raw: &Path) -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    let raw_path = if raw.is_absolute() {
        raw.to_path_buf()
    } else {
        base.join(raw)
    };

    if raw
        .extension()
        .and_then(|extension| extension.to_str())
        .is_none_or(|extension| extension.eq_ignore_ascii_case("tex"))
    {
        candidates.push(raw_path.clone());
    }

    if !raw_path
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("tex"))
    {
        candidates.push(PathBuf::from(format!("{}.tex", raw_path.display())));
    }

    candidates
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

fn read_input_argument(content: &str, offset: usize) -> Option<(String, usize)> {
    if let Some((body_start, body_end)) = read_required_argument(content, offset) {
        return Some((
            content[body_start..body_end].trim().to_string(),
            body_end + 1,
        ));
    }

    let bytes = content.as_bytes();
    let path_start = skip_ascii_whitespace(content, offset);
    let mut path_end = path_start;

    while path_end < bytes.len() {
        let byte = bytes[path_end];
        if byte.is_ascii_whitespace() || matches!(byte, b'%' | b'{' | b'}' | b'\\') {
            break;
        }
        path_end += 1;
    }

    (path_end > path_start).then(|| (content[path_start..path_end].trim().to_string(), path_end))
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
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::discover_tex_files;
    use super::find_magic_root;
    use super::magic_root_path;

    fn temp_project(name: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time before unix epoch")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("paper-linter-discovery-{name}-{nonce}"));
        fs::create_dir_all(&dir).expect("failed to create temp project");
        dir
    }

    fn write(path: &Path, content: &str) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("failed to create parent");
        }
        fs::write(path, content).expect("failed to write fixture");
    }

    fn canonical(path: &Path) -> PathBuf {
        path.canonicalize().expect("canonical path")
    }

    #[test]
    fn finds_tex_root_magic_comments() {
        assert_eq!(
            find_magic_root("%! TeX root = ../main.tex\n"),
            Some("../main.tex".to_string())
        );
        assert_eq!(
            find_magic_root("% !TEX root = paper\n"),
            Some("paper".to_string())
        );
        assert_eq!(
            find_magic_root("%! TEX root = \"manuscript.tex\"\n"),
            Some("manuscript.tex".to_string())
        );
    }

    #[test]
    fn ignores_non_root_comments() {
        assert_eq!(find_magic_root("% TODO root = main.tex\n"), None);
    }

    #[test]
    fn resolves_magic_root_relative_to_current_file() {
        let dir = temp_project("magic-root");
        let section = dir.join("sections/method.tex");
        let root = dir.join("main.tex");
        write(&root, "\\documentclass{article}\n");
        write(&section, "%! TeX root = ../main.tex\n");

        assert_eq!(
            magic_root_path(&section).expect("root should parse"),
            root.canonicalize().ok()
        );
    }

    #[test]
    fn reachable_files_ignore_explicit_non_tex_inputs() {
        let dir = temp_project("pgf-input");
        let main = dir.join("main.tex");
        let plot = dir.join("plot.pgf");
        write(
            &main,
            "\\documentclass{article}\n\\begin{document}\n\\input{plot.pgf}\n\\end{document}\n",
        );
        write(&plot, "\\input{nested}\n");
        write(&dir.join("nested.tex"), "\\label{nested}\n");

        let files =
            discover_tex_files(std::slice::from_ref(&dir), false).expect("should discover files");

        assert_eq!(files, vec![canonical(&main)]);
    }

    #[test]
    fn reachable_files_follow_bare_input_paths() {
        let dir = temp_project("bare-input");
        let main = dir.join("main.tex");
        let section = dir.join("sections/method.tex");
        write(&main, "\\documentclass{article}\n\\input sections/method\n");
        write(&section, "\\label{sec:method}\n");

        let files =
            discover_tex_files(std::slice::from_ref(&dir), false).expect("should discover files");

        assert_eq!(files, vec![canonical(&main), canonical(&section)]);
    }

    #[test]
    fn reachable_files_follow_root_relative_inputs_from_nested_files() {
        let dir = temp_project("root-relative-input");
        let main = dir.join("main.tex");
        let supp = dir.join("src/supp/supp.tex");
        let method = dir.join("src/supp/method.tex");
        write(&main, "\\documentclass{article}\n\\input src/supp/supp\n");
        write(&supp, "\\input src/supp/method\n");
        write(&method, "\\label{sec:method}\n");

        let files =
            discover_tex_files(std::slice::from_ref(&dir), false).expect("should discover files");

        assert_eq!(
            files,
            vec![canonical(&main), canonical(&method), canonical(&supp)]
        );
    }

    #[test]
    fn project_roots_prefer_top_level_bbl() {
        let dir = temp_project("duplicate-source-tree");
        let main = dir.join("main.tex");
        let nested = dir.join("copy/main.tex");
        write(&main, "\\documentclass{article}\n");
        write(&dir.join("main.bbl"), "\\bibitem{top}\n");
        write(&nested, "\\documentclass{article}\n");
        write(&dir.join("copy/main.bbl"), "\\bibitem{nested}\n");

        let files =
            discover_tex_files(std::slice::from_ref(&dir), false).expect("should discover files");

        assert_eq!(files, vec![canonical(&main)]);
    }
}
