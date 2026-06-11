pub mod citations;
mod ws001;

use std::path::Path;

use crate::diagnostic::Diagnostic;

pub trait Rule: Sync {
    fn code(&self) -> &'static str;
    fn name(&self) -> &'static str;
    fn check_file(&self, path: &Path, content: &str) -> Vec<Diagnostic>;
}

static WS001_RULE: ws001::TrailingWhitespace = ws001::TrailingWhitespace;
static RULES: [&dyn Rule; 1] = [&WS001_RULE];

pub fn all_rules() -> &'static [&'static dyn Rule] {
    &RULES
}

#[cfg(test)]
mod tests {
    use super::{all_rules, Rule};

    #[test]
    fn rule_registry_contains_ws001() {
        let codes: Vec<_> = all_rules().iter().map(|rule| rule.code()).collect();
        assert_eq!(codes, vec!["WS001"]);
    }

    fn assert_rule_trait_object(_: &dyn Rule) {}

    #[test]
    fn registry_rules_are_trait_objects() {
        assert_rule_trait_object(all_rules()[0]);
    }
}
