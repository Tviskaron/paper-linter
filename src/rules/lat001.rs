use crate::diagnostic::{Diagnostic, Severity};
use crate::latex::commands::commands_in_line;
use crate::latex::environments::{environment_events, EnvironmentEventKind};
use crate::project::ProjectIndex;
use crate::rules::ProjectRule;

pub struct LegacyLatex;

impl ProjectRule for LegacyLatex {
    fn code(&self) -> &'static str {
        "LAT001"
    }

    fn name(&self) -> &'static str {
        "legacy latex"
    }

    fn strict_only(&self) -> bool {
        true
    }

    fn check_project(&self, project: &ProjectIndex) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();

        for file in &project.files {
            for (index, line) in file.content.lines().enumerate() {
                let line_number = index + 1;
                for command in commands_in_line(line, line_number) {
                    if let Some(hint) = legacy_command_hint(&command.name) {
                        diagnostics.push(
                            Diagnostic::new(
                                self.code(),
                                Severity::Warning,
                                format!("legacy command '\\{}' is deprecated", command.name),
                                &file.path,
                                command.line,
                                command.column,
                            )
                            .with_hint(hint),
                        );
                    }
                }
            }

            for event in environment_events(&file.content) {
                if event.kind != EnvironmentEventKind::Begin {
                    continue;
                }

                let env_name = event.name.trim_end_matches('*');
                if let Some(hint) = legacy_environment_hint(env_name) {
                    diagnostics.push(
                        Diagnostic::new(
                            self.code(),
                            Severity::Warning,
                            format!("legacy environment '{}' is deprecated", event.name),
                            &file.path,
                            event.line,
                            event.column,
                        )
                        .with_hint(hint),
                    );
                }
            }
        }

        for package in &project.packages {
            if let Some(hint) = deprecated_package_hint(&package.name) {
                diagnostics.push(
                    Diagnostic::new(
                        self.code(),
                        Severity::Warning,
                        format!("deprecated package '{}' is discouraged", package.name),
                        &package.location.file,
                        package.location.line,
                        package.location.column,
                    )
                    .with_hint(hint),
                );
            }
        }

        diagnostics
    }
}

fn legacy_command_hint(command: &str) -> Option<&'static str> {
    match command {
        "bf" => Some("use \\textbf{...} or scoped \\bfseries"),
        "it" => Some("use \\textit{...} or scoped \\itshape"),
        _ => None,
    }
}

fn legacy_environment_hint(environment: &str) -> Option<&'static str> {
    match environment {
        "eqnarray" => Some("use align or equation from amsmath"),
        "displaymath" => Some("use \\[...\\] or equation"),
        _ => None,
    }
}

fn deprecated_package_hint(package: &str) -> Option<&'static str> {
    match package {
        "epsfig" => Some("use graphicx"),
        "times" => {
            Some("use a modern font package such as newtxtext/newtxmath or the venue template")
        }
        "subfigure" => Some("use subcaption when the venue allows it"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{deprecated_package_hint, legacy_command_hint, legacy_environment_hint};

    #[test]
    fn recognizes_legacy_commands() {
        assert!(legacy_command_hint("bf").is_some());
        assert!(legacy_command_hint("it").is_some());
        assert!(legacy_command_hint("bfseries").is_none());
    }

    #[test]
    fn recognizes_legacy_environments() {
        assert!(legacy_environment_hint("eqnarray").is_some());
        assert!(legacy_environment_hint("displaymath").is_some());
        assert!(legacy_environment_hint("equation").is_none());
    }

    #[test]
    fn recognizes_deprecated_packages() {
        assert!(deprecated_package_hint("epsfig").is_some());
        assert!(deprecated_package_hint("times").is_some());
        assert!(deprecated_package_hint("subfigure").is_some());
        assert!(deprecated_package_hint("graphicx").is_none());
    }
}
