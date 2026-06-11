use serde::Serialize;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Error,
    Warning,
}

impl Severity {
    pub fn as_str(self) -> &'static str {
        match self {
            Severity::Error => "error",
            Severity::Warning => "warning",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Diagnostic {
    pub code: &'static str,
    pub severity: Severity,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hint: Option<String>,
    pub file: PathBuf,
    pub line: usize,
    pub column: usize,
}

impl Diagnostic {
    pub fn new(
        code: &'static str,
        severity: Severity,
        message: impl Into<String>,
        file: impl Into<PathBuf>,
        line: usize,
        column: usize,
    ) -> Self {
        Self {
            code,
            severity,
            message: message.into(),
            hint: None,
            file: file.into(),
            line,
            column,
        }
    }

    pub fn with_hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = Some(hint.into());
        self
    }
}
