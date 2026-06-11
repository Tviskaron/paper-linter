mod cap001;
pub(crate) mod citations;
mod env001;
mod fig001;
mod fig002;
mod fmt001;
mod fmt002;
mod lbl001;
mod sec001;
mod sec002;
mod tab001;
mod tex001;
mod txt001;
mod txt002;
mod ws001;

use std::path::Path;

use crate::diagnostic::Diagnostic;
use crate::project::ProjectIndex;

pub trait Rule: Sync {
    fn code(&self) -> &'static str;
    fn name(&self) -> &'static str;
    fn check_file(&self, path: &Path, content: &str) -> Vec<Diagnostic>;
}

pub trait ProjectRule: Sync {
    fn code(&self) -> &'static str;
    fn name(&self) -> &'static str;
    fn check_project(&self, project: &ProjectIndex) -> Vec<Diagnostic>;
}

static ENV001_RULE: env001::EnvironmentMismatch = env001::EnvironmentMismatch;
static FMT001_RULE: fmt001::MissingFinalNewline = fmt001::MissingFinalNewline;
static FMT002_RULE: fmt002::RepeatedBlankLines = fmt002::RepeatedBlankLines;
static SEC001_RULE: sec001::SkippedSectionLevel = sec001::SkippedSectionLevel;
static SEC002_RULE: sec002::EmptySection = sec002::EmptySection;
static TEX001_RULE: tex001::MissingNonBreakingSpace = tex001::MissingNonBreakingSpace;
static TXT001_RULE: txt001::PlaceholderText = txt001::PlaceholderText;
static TXT002_RULE: txt002::RepeatedWords = txt002::RepeatedWords;
static WS001_RULE: ws001::TrailingWhitespace = ws001::TrailingWhitespace;
static RULES: [&dyn Rule; 9] = [
    &ENV001_RULE,
    &FMT001_RULE,
    &FMT002_RULE,
    &SEC001_RULE,
    &SEC002_RULE,
    &TEX001_RULE,
    &TXT001_RULE,
    &TXT002_RULE,
    &WS001_RULE,
];

static FIG001_RULE: fig001::MissingAsset = fig001::MissingAsset;
static CAP001_RULE: cap001::MissingCaption = cap001::MissingCaption;
static FIG002_RULE: fig002::OrphanFigure = fig002::OrphanFigure;
static TAB001_RULE: tab001::OrphanTable = tab001::OrphanTable;
static LBL001_RULE: lbl001::UnusedLabel = lbl001::UnusedLabel;
static PROJECT_RULES: [&dyn ProjectRule; 5] = [
    &FIG001_RULE,
    &CAP001_RULE,
    &FIG002_RULE,
    &TAB001_RULE,
    &LBL001_RULE,
];

pub fn all_rules() -> &'static [&'static dyn Rule] {
    &RULES
}

pub fn all_project_rules() -> &'static [&'static dyn ProjectRule] {
    &PROJECT_RULES
}

#[cfg(test)]
mod tests {
    use super::{all_project_rules, all_rules, ProjectRule, Rule};

    #[test]
    fn rule_registry_contains_rules() {
        let codes: Vec<_> = all_rules().iter().map(|rule| rule.code()).collect();
        assert_eq!(
            codes,
            vec![
                "ENV001", "FMT001", "FMT002", "SEC001", "SEC002", "TEX001", "TXT001", "TXT002",
                "WS001"
            ]
        );
    }

    #[test]
    fn project_rule_registry_contains_figures_and_tables_rules() {
        let codes: Vec<_> = all_project_rules().iter().map(|rule| rule.code()).collect();
        assert_eq!(
            codes,
            vec!["FIG001", "CAP001", "FIG002", "TAB001", "LBL001"]
        );
    }

    fn assert_rule_trait_object(_: &dyn Rule) {}

    fn assert_project_rule_trait_object(_: &dyn ProjectRule) {}

    #[test]
    fn registry_rules_are_trait_objects() {
        assert_rule_trait_object(all_rules()[0]);
        assert_project_rule_trait_object(all_project_rules()[0]);
    }
}
