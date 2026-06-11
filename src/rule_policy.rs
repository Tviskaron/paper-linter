pub fn enabled_by_default(code: &str) -> bool {
    !matches!(code, "TXT003" | "TXT004" | "TXT005" | "CMT001" | "PRJ005")
}

pub fn strict_only(code: &str) -> bool {
    matches!(code, "CIT010" | "TXT005" | "PRJ005")
}

pub fn never_promote_to_error(code: &str) -> bool {
    matches!(code, "TXT005")
}

pub fn code_is_enabled(code: &str, select: &[String], ignore: &[String], strict: bool) -> bool {
    if strict_only(code) {
        if select.is_empty() && !strict {
            return false;
        }
    } else if select.is_empty() && !enabled_by_default(code) {
        return false;
    }

    let selected = select.is_empty() || select.iter().any(|pattern| code.starts_with(pattern));
    let ignored = ignore.iter().any(|pattern| code.starts_with(pattern));
    selected && !ignored
}

#[cfg(test)]
mod tests {
    use super::code_is_enabled;

    #[test]
    fn opt_in_rules_disabled_by_default() {
        assert!(!code_is_enabled("TXT003", &[], &[], false));
        assert!(!code_is_enabled("TXT004", &[], &[], false));
        assert!(!code_is_enabled("CMT001", &[], &[], false));
    }

    #[test]
    fn strict_only_rules_need_strict_or_select() {
        assert!(!code_is_enabled("CIT010", &[], &[], false));
        assert!(code_is_enabled("CIT010", &[], &[], true));
        assert!(code_is_enabled(
            "CIT010",
            &[String::from("CIT010")],
            &[],
            false
        ));

        assert!(!code_is_enabled("TXT005", &[], &[], false));
        assert!(code_is_enabled("TXT005", &[], &[], true));
        assert!(code_is_enabled(
            "TXT005",
            &[String::from("TXT005")],
            &[],
            false
        ));
    }

    #[test]
    fn prj_rules_enabled_by_default() {
        assert!(code_is_enabled("PRJ001", &[], &[], false));
    }
}
