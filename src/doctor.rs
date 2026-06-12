use std::io::{self, Write};
use std::path::Path;

use crate::project::ProjectIndex;
use crate::project_graph::ProjectGraph;

pub fn run_doctor(path: &Path, json: bool) -> io::Result<()> {
    let graph = ProjectGraph::analyze(path)?;
    let discovered = graph.reachable.iter().cloned().collect::<Vec<_>>();
    let index = ProjectIndex::build_project_files(&[path.to_path_buf()], &discovered).ok();

    if json {
        let payload = serde_json::json!({
            "paper_dir": graph.paper_dir,
            "root": graph.root,
            "root_method": graph.root_method.as_str(),
            "reachable_count": graph.reachable.len(),
            "all_tex_count": graph.all_tex.len(),
            "missing_includes": graph.missing_includes.len(),
            "orphans": graph.all_tex.iter().filter(|path| !graph.reachable.contains(*path)).collect::<Vec<_>>(),
            "labels": index.as_ref().map(|project| project.labels.len()).unwrap_or(0),
            "refs": index.as_ref().map(|project| project.refs.len()).unwrap_or(0),
        });
        writeln!(io::stdout(), "{}", serde_json::to_string_pretty(&payload)?)?;
        return Ok(());
    }

    writeln!(io::stdout(), "Project doctor for {}", path.display())?;
    writeln!(
        io::stdout(),
        "  root: {}",
        graph
            .root
            .as_ref()
            .map(|root| root.display().to_string())
            .unwrap_or_else(|| "(unresolved)".to_string())
    )?;
    writeln!(
        io::stdout(),
        "  root method: {}",
        graph.root_method.as_str()
    )?;
    writeln!(
        io::stdout(),
        "  reachable .tex files: {}",
        graph.reachable.len()
    )?;
    writeln!(io::stdout(), "  all .tex files: {}", graph.all_tex.len())?;
    writeln!(
        io::stdout(),
        "  missing includes: {}",
        graph.missing_includes.len()
    )?;

    let orphans: Vec<_> = graph
        .all_tex
        .iter()
        .filter(|path| !graph.reachable.contains(*path))
        .collect();
    if orphans.is_empty() {
        writeln!(io::stdout(), "  orphan .tex files: none")?;
    } else {
        writeln!(io::stdout(), "  orphan .tex files:")?;
        for orphan in orphans {
            writeln!(io::stdout(), "    - {}", orphan.display())?;
        }
    }

    Ok(())
}
