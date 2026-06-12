use std::collections::BTreeMap;

use crate::diagnostic::{Diagnostic, Severity};
use crate::latex::scan::PackageImport;
use crate::project::ProjectIndex;
use crate::rules::ProjectRule;

const CLASH_PRONE_PACKAGES: &[&str] = &[
    "xcolor", "hyperref", "babel", "geometry", "fontenc", "inputenc",
];

pub struct PackageOptionClash;

impl ProjectRule for PackageOptionClash {
    fn code(&self) -> &'static str {
        "PKG001"
    }

    fn name(&self) -> &'static str {
        "package option clash"
    }

    fn check_project(&self, project: &ProjectIndex) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        let mut by_name: BTreeMap<String, Vec<&PackageImport>> = BTreeMap::new();

        for package in &project.packages {
            by_name
                .entry(normalize_package_name(&package.name))
                .or_default()
                .push(package);
        }

        for (name, imports) in &by_name {
            if imports.len() < 2 {
                continue;
            }

            let option_sets: Vec<Vec<String>> = imports
                .iter()
                .map(|import| normalize_options(&import.options))
                .collect();

            if option_sets.iter().all(|options| options.is_empty()) {
                continue;
            }

            let first = &option_sets[0];
            if option_sets.iter().all(|options| options == first) {
                continue;
            }

            let severity = if CLASH_PRONE_PACKAGES.contains(&name.as_str()) {
                Severity::Error
            } else {
                Severity::Warning
            };

            let latest = imports.last().expect("imports");
            diagnostics.push(
                Diagnostic::new(
                    self.code(),
                    severity,
                    format!(
                        "package '{}' is loaded {} times with different options",
                        name,
                        imports.len()
                    ),
                    &latest.location.file,
                    latest.location.line,
                    latest.location.column,
                )
                .with_hint(
                    "merge package options into a single \\usepackage call to avoid option clashes",
                ),
            );
        }

        if let (Some(hyperref), Some(cleveref)) = (
            find_package_import(&project.packages, "hyperref"),
            find_package_import(&project.packages, "cleveref"),
        ) {
            if hyperref.location.line > cleveref.location.line
                || (hyperref.location.line == cleveref.location.line
                    && hyperref.location.column > cleveref.location.column)
            {
                diagnostics.push(
                    Diagnostic::new(
                        self.code(),
                        Severity::Warning,
                        "package 'cleveref' should be loaded after 'hyperref'",
                        &hyperref.location.file,
                        hyperref.location.line,
                        hyperref.location.column,
                    )
                    .with_hint("load cleveref after hyperref, or use cleveref's hyperref option"),
                );
            }
        }

        diagnostics
    }
}

fn normalize_package_name(name: &str) -> String {
    name.trim().to_ascii_lowercase()
}

fn normalize_options(options: &[String]) -> Vec<String> {
    let mut normalized: Vec<String> = options
        .iter()
        .flat_map(|option| option.split(','))
        .map(|option| option.trim().to_ascii_lowercase())
        .filter(|option| !option.is_empty())
        .collect();
    normalized.sort();
    normalized.dedup();
    normalized
}

fn find_package_import<'a>(packages: &'a [PackageImport], name: &str) -> Option<&'a PackageImport> {
    packages
        .iter()
        .find(|package| normalize_package_name(&package.name) == name)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use crate::project::{ProjectIndex, SourceFile};

    use super::PackageOptionClash;
    use crate::rules::ProjectRule;

    fn package_index(content: &str) -> ProjectIndex {
        ProjectIndex {
            root: PathBuf::from("."),
            files: vec![SourceFile {
                path: PathBuf::from("main.tex"),
                content: content.to_string(),
            }],
            labels: Vec::new(),
            refs: Vec::new(),
            graphics: Vec::new(),
            graphics_paths: Vec::new(),
            bibliographies: Vec::new(),
            document_classes: Vec::new(),
            packages: crate::latex::scan::scan_latex("main.tex", content).packages,
            floats: Vec::new(),
        }
    }

    #[test]
    fn flags_xcolor_option_clash() {
        let project = package_index(
            "\\documentclass{article}\n\\usepackage[dvipsnames]{xcolor}\n\\usepackage[table]{xcolor}\n\\begin{document}\n\\end{document}\n",
        );
        let diagnostics = PackageOptionClash.check_project(&project);
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, "PKG001");
    }

    #[test]
    fn ignores_identical_duplicate_loads() {
        let project = package_index(
            "\\documentclass{article}\n\\usepackage{graphicx}\n\\usepackage{graphicx}\n\\begin{document}\n\\end{document}\n",
        );
        let diagnostics = PackageOptionClash.check_project(&project);
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn warns_when_cleveref_loads_before_hyperref() {
        let content = "\\documentclass{article}\n\\usepackage{cleveref}\n\\usepackage{hyperref}\n\\begin{document}\n\\end{document}\n";
        let scan = crate::latex::scan::scan_latex("main.tex", content);
        let project = ProjectIndex {
            root: PathBuf::from("."),
            files: vec![SourceFile {
                path: PathBuf::from("main.tex"),
                content: content.to_string(),
            }],
            labels: Vec::new(),
            refs: Vec::new(),
            graphics: Vec::new(),
            graphics_paths: Vec::new(),
            bibliographies: Vec::new(),
            document_classes: Vec::new(),
            packages: scan.packages,
            floats: Vec::new(),
        };
        let diagnostics = PackageOptionClash.check_project(&project);
        assert!(diagnostics
            .iter()
            .any(|diag| diag.message.contains("hyperref")));
    }
}
