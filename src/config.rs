use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::Path;

use crate::diagnostic::Severity;
use crate::latex::scan::ScanAliases;
use crate::rule_policy;

#[derive(Debug, Clone, Default)]
pub struct LinterConfig {
    pub enable: Vec<String>,
    pub disable: Vec<String>,
    pub strict: bool,
    pub thresholds: BTreeMap<String, String>,
    pub severity: BTreeMap<String, Severity>,
    pub aliases: ScanAliases,
    pub bibliography_forbidden_fields: Vec<String>,
}

impl LinterConfig {
    pub fn load(path: &Path) -> io::Result<Self> {
        let content = fs::read_to_string(path)?;
        parse_toml(&content)
    }

    pub fn load_preset(name: &str) -> io::Result<Self> {
        parse_toml(preset_content(name)?)
    }

    pub fn merge_into_options(&self, select: &mut Vec<String>, ignore: &mut Vec<String>) {
        for code in &self.enable {
            if !select.iter().any(|existing| existing.starts_with(code)) {
                select.push(code.clone());
            }
        }
        for code in &self.disable {
            if !ignore.iter().any(|existing| existing.starts_with(code)) {
                ignore.push(code.clone());
            }
        }
    }

    pub fn threshold_usize(&self, key: &str, default: usize) -> usize {
        self.thresholds
            .get(key)
            .and_then(|value| value.parse().ok())
            .unwrap_or(default)
    }
}

fn preset_content(name: &str) -> io::Result<&'static str> {
    match name {
        "essential" => Ok(include_str!("../presets/essential.toml")),
        "standard" => Ok(include_str!("../presets/standard.toml")),
        "strict" => Ok(include_str!("../presets/strict.toml")),
        "polish" => Ok(include_str!("../presets/polish.toml")),
        _ => Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("preset not found: {name}"),
        )),
    }
}

fn parse_toml(content: &str) -> io::Result<LinterConfig> {
    let mut config = LinterConfig::default();
    let mut section = "";

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            section = trimmed.trim_matches(['[', ']']).trim();
            continue;
        }

        let Some((key, value)) = trimmed.split_once('=') else {
            continue;
        };
        let key = key.trim();
        let value = parse_toml_value(value.trim());

        match section {
            "rules" if key == "enable" => {
                config.enable = parse_string_list(&value);
            }
            "rules" if key == "disable" => {
                config.disable = parse_string_list(&value);
            }
            "options" if key == "strict" => {
                config.strict = parse_bool(&value);
            }
            "thresholds" => {
                config.thresholds.insert(key.to_string(), value);
            }
            "severity" => {
                if let Some(severity) = parse_severity(&value) {
                    config.severity.insert(key.to_ascii_uppercase(), severity);
                }
            }
            "aliases" if matches!(key, "cite" | "cites") => {
                config.aliases.cites = parse_string_list(&value);
            }
            "aliases" if matches!(key, "ref" | "refs") => {
                config.aliases.refs = parse_string_list(&value);
            }
            "aliases" if matches!(key, "input" | "inputs") => {
                config.aliases.inputs = parse_string_list(&value);
            }
            "aliases" if matches!(key, "graphic" | "graphics") => {
                config.aliases.graphics = parse_string_list(&value);
            }
            "bibliography" if key == "forbidden_fields" => {
                config.bibliography_forbidden_fields = parse_string_list(&value)
                    .into_iter()
                    .map(|field| field.to_ascii_lowercase())
                    .collect();
            }
            _ => {}
        }
    }

    Ok(config)
}

fn parse_toml_value(value: &str) -> String {
    value.trim_matches('"').to_string()
}

fn parse_string_list(value: &str) -> Vec<String> {
    value
        .trim_matches(['[', ']'])
        .split(',')
        .map(|item| item.trim().trim_matches('"').to_string())
        .filter(|item| !item.is_empty())
        .collect()
}

fn parse_severity(value: &str) -> Option<Severity> {
    match value.to_ascii_lowercase().as_str() {
        "error" => Some(Severity::Error),
        "warning" => Some(Severity::Warning),
        _ => None,
    }
}

fn parse_bool(value: &str) -> bool {
    matches!(
        value.to_ascii_lowercase().as_str(),
        "true" | "1" | "yes" | "on"
    )
}

pub fn append_missing(target: &mut Vec<String>, source: &[String]) {
    for item in source {
        if !target.contains(item) {
            target.push(item.clone());
        }
    }
}

pub fn apply_preset(
    preset: Option<&str>,
    select: &mut Vec<String>,
    ignore: &mut Vec<String>,
) -> io::Result<Option<LinterConfig>> {
    if let Some(name) = preset {
        let config = LinterConfig::load_preset(name)?;
        config.merge_into_options(select, ignore);
        return Ok(Some(config));
    }
    Ok(None)
}

pub fn is_enabled_with_config(
    code: &str,
    select: &[String],
    ignore: &[String],
    strict: bool,
    config: &LinterConfig,
) -> bool {
    if config
        .disable
        .iter()
        .any(|pattern| code.starts_with(pattern))
    {
        return false;
    }
    if config
        .enable
        .iter()
        .any(|pattern| code.starts_with(pattern))
    {
        return rule_policy::code_is_enabled(code, select, ignore, strict) || select.is_empty();
    }
    rule_policy::code_is_enabled(code, select, ignore, strict)
}

#[cfg(test)]
mod tests {
    use super::parse_toml;

    #[test]
    fn parses_enable_rules() {
        let config =
            parse_toml("[rules]\nenable = [\"TXT003\", \"TXT004\"]\ndisable = [\"CIT002\"]\n")
                .expect("parse");
        assert_eq!(config.enable, vec!["TXT003", "TXT004"]);
        assert_eq!(config.disable, vec!["CIT002"]);
    }

    #[test]
    fn parses_command_aliases() {
        let config = parse_toml(
            "[aliases]\ncite = [\"mycite\"]\nref = [\"figref\"]\ninput = [\"subfile\"]\ngraphic = [\"plotfile\"]\n",
        )
        .expect("parse");

        assert_eq!(config.aliases.cites, vec!["mycite"]);
        assert_eq!(config.aliases.refs, vec!["figref"]);
        assert_eq!(config.aliases.inputs, vec!["subfile"]);
        assert_eq!(config.aliases.graphics, vec!["plotfile"]);
    }

    #[test]
    fn parses_preset_options() {
        let config = parse_toml("[options]\nstrict = true\n").expect("parse");

        assert!(config.strict);
    }

    #[test]
    fn loads_essential_preset() {
        let config = super::LinterConfig::load_preset("essential").expect("preset");
        assert!(config.enable.iter().any(|code| code == "CIT012"));
        assert!(config.enable.iter().any(|code| code == "PKG001"));
    }

    #[test]
    fn loads_strict_and_standard_presets() {
        let strict = super::LinterConfig::load_preset("strict").expect("strict");
        let standard = super::LinterConfig::load_preset("standard").expect("standard");
        assert!(strict.strict);
        assert!(strict.enable.iter().any(|code| code == "PKG002"));
        assert!(standard.enable.iter().any(|code| code == "TEX002"));
    }

    #[test]
    fn loads_polish_preset() {
        let polish = super::LinterConfig::load_preset("polish").expect("polish");
        assert!(polish.enable.iter().any(|code| code == "TXT001"));
        assert!(!polish.enable.iter().any(|code| code == "CMT001"));
    }

    #[test]
    fn parses_bibliography_forbidden_fields() {
        let config =
            parse_toml("[bibliography]\nforbidden_fields = [\"abstract\", \"Keywords\"]\n")
                .expect("parse");

        assert_eq!(
            config.bibliography_forbidden_fields,
            vec!["abstract", "keywords"]
        );
    }

    #[test]
    fn parses_severity_overrides() {
        let config =
            parse_toml("[severity]\ntxt = \"error\"\nref001 = \"warning\"\n").expect("parse");

        assert_eq!(
            config.severity.get("TXT"),
            Some(&crate::diagnostic::Severity::Error)
        );
        assert_eq!(
            config.severity.get("REF001"),
            Some(&crate::diagnostic::Severity::Warning)
        );
    }
}
