use std::fs;
use std::io;
use std::path::{Path, PathBuf};

pub fn discover_tex_files(paths: &[PathBuf]) -> io::Result<Vec<PathBuf>> {
    let mut files = Vec::new();

    for path in paths {
        if path.is_file() {
            if is_tex_file(path) {
                files.push(path.clone());
            }
        } else if path.is_dir() {
            collect_tex_files(path, &mut files)?;
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
