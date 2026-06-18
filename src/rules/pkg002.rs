use crate::diagnostic::{Diagnostic, Severity};
use crate::project::ProjectIndex;
use crate::rules::ProjectRule;

pub struct PackageDependencies;

impl ProjectRule for PackageDependencies {
    fn code(&self) -> &'static str {
        "PKG002"
    }

    fn name(&self) -> &'static str {
        "package dependency"
    }

    fn strict_only(&self) -> bool {
        true
    }

    fn check_project(&self, project: &ProjectIndex) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();

        if !project.graphics.is_empty() && !project.uses_package("graphicx") {
            if let Some(graphic) = project.graphics.first() {
                diagnostics.push(
                    Diagnostic::new(
                        self.code(),
                        Severity::Warning,
                        "\\includegraphics is used but package 'graphicx' is not loaded",
                        &graphic.location.file,
                        graphic.location.line,
                        graphic.location.column,
                    )
                    .with_hint("add \\usepackage{graphicx}"),
                );
            }
        }

        for reference in &project.refs {
            if matches!(reference.command.as_str(), "cref" | "Cref")
                && !project.uses_package("cleveref")
            {
                diagnostics.push(
                    Diagnostic::new(
                        self.code(),
                        Severity::Warning,
                        format!(
                            "\\{} is used but package 'cleveref' is not loaded",
                            reference.command
                        ),
                        &reference.location.file,
                        reference.location.line,
                        reference.location.column,
                    )
                    .with_hint("add \\usepackage{cleveref}"),
                );
            }
        }

        for file in &project.files {
            for (line_number, line) in file.content.lines().enumerate() {
                let line_number = line_number + 1;
                if line.trim_start().starts_with('%') {
                    continue;
                }

                for env in amsmath_environments_in_line(line) {
                    if !project.uses_package("amsmath") {
                        diagnostics.push(
                            Diagnostic::new(
                                self.code(),
                                Severity::Warning,
                                format!("environment '{env}' requires package 'amsmath'"),
                                &file.path,
                                line_number,
                                line.find(env).unwrap_or(0) + 1,
                            )
                            .with_hint("add \\usepackage{amsmath}"),
                        );
                    }
                }

                if line.contains("\\begin{algorithmic}")
                    && !project.uses_package("algorithm")
                    && !project.uses_package("algorithmic")
                    && !project.uses_package("algorithmicx")
                {
                    diagnostics.push(
                        Diagnostic::new(
                            self.code(),
                            Severity::Warning,
                            "environment 'algorithmic' requires package 'algorithm' or 'algorithmicx'",
                            &file.path,
                            line_number,
                            line.find("algorithmic").unwrap_or(0) + 1,
                        )
                        .with_hint("add \\usepackage{algorithm} or \\usepackage{algorithmicx}"),
                    );
                }
            }
        }

        if project.uses_package("subcaption") && project.uses_package("subfigure") {
            if let Some(package) = project
                .packages
                .iter()
                .find(|package| package.name.eq_ignore_ascii_case("subcaption"))
            {
                diagnostics.push(
                    Diagnostic::new(
                        self.code(),
                        Severity::Warning,
                        "packages 'subcaption' and 'subfigure' should not be used together",
                        &package.location.file,
                        package.location.line,
                        package.location.column,
                    )
                    .with_hint("use only subcaption for sub-floats"),
                );
            }
        }

        diagnostics
    }
}

fn amsmath_environments_in_line(line: &str) -> Vec<&'static str> {
    const ENVIRONMENTS: &[&str] = &[
        "align",
        "align*",
        "alignat",
        "alignat*",
        "equation",
        "equation*",
        "gather",
        "gather*",
        "multline",
        "multline*",
        "split",
        "cases",
        "matrix",
        "pmatrix",
        "bmatrix",
        "vmatrix",
        "Vmatrix",
    ];

    ENVIRONMENTS
        .iter()
        .copied()
        .filter(|env| line.contains(&format!("\\begin{{{env}}}")))
        .collect()
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::path::PathBuf;

    use crate::project::{ProjectIndex, SourceFile};

    use super::PackageDependencies;
    use crate::rules::ProjectRule;

    #[test]
    fn requires_graphicx_for_includegraphics() {
        let content = "\\documentclass{article}\n\\begin{document}\n\\includegraphics{fig}\n\\end{document}\n";
        let scan = crate::latex::scan::scan_latex("main.tex", content);
        let project = ProjectIndex {
            root: PathBuf::from("."),
            files: vec![SourceFile {
                path: PathBuf::from("main.tex"),
                content: content.to_string(),
            }],
            labels: Vec::new(),
            refs: Vec::new(),
            graphics: scan.graphics,
            graphics_paths: Vec::new(),
            bibliographies: Vec::new(),
            document_classes: Vec::new(),
            packages: scan.packages,
            floats: Vec::new(),
            asset_headers: BTreeMap::new(),
        };

        let diagnostics = PackageDependencies.check_project(&project);
        assert!(diagnostics
            .iter()
            .any(|diag| diag.message.contains("graphicx")));
    }
}
