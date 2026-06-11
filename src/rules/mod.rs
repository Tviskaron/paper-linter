mod fmt001;
mod fmt002;
mod sec001;
mod sec002;
mod txt001;
mod txt002;
mod ws001;

use std::path::Path;

use crate::diagnostic::Diagnostic;

pub trait Rule: Sync {
    fn code(&self) -> &'static str;
    fn name(&self) -> &'static str;
    fn check_file(&self, path: &Path, content: &str) -> Vec<Diagnostic>;
}

static FMT001_RULE: fmt001::MissingFinalNewline = fmt001::MissingFinalNewline;
static FMT002_RULE: fmt002::RepeatedBlankLines = fmt002::RepeatedBlankLines;
static SEC001_RULE: sec001::SkippedSectionLevel = sec001::SkippedSectionLevel;
static SEC002_RULE: sec002::EmptySection = sec002::EmptySection;
static TXT001_RULE: txt001::PlaceholderText = txt001::PlaceholderText;
static TXT002_RULE: txt002::RepeatedWords = txt002::RepeatedWords;
static WS001_RULE: ws001::TrailingWhitespace = ws001::TrailingWhitespace;
static RULES: [&dyn Rule; 7] = [
    &FMT001_RULE,
    &FMT002_RULE,
    &SEC001_RULE,
    &SEC002_RULE,
    &TXT001_RULE,
    &TXT002_RULE,
    &WS001_RULE,
];

pub fn all_rules() -> &'static [&'static dyn Rule] {
    &RULES
}

#[cfg(test)]
mod tests {
    use super::{all_rules, Rule};

    #[test]
    fn rule_registry_contains_rules() {
        let codes: Vec<_> = all_rules().iter().map(|rule| rule.code()).collect();
        assert_eq!(
            codes,
            vec!["FMT001", "FMT002", "SEC001", "SEC002", "TXT001", "TXT002", "WS001"]
        );
    }

    fn assert_rule_trait_object(_: &dyn Rule) {}

    #[test]
    fn registry_rules_are_trait_objects() {
        assert_rule_trait_object(all_rules()[0]);
    }
}
