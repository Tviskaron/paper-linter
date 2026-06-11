pub fn enabled_by_default(code: &str) -> bool {
    !matches!(
        code,
        "FMT001" | "FMT002" | "TXT003" | "TXT004" | "TXT005" | "CMT001" | "WS001" | "PRJ005"
    )
}

pub fn strict_only(code: &str) -> bool {
    matches!(
        code,
        "CIT009" | "CIT010" | "CIT011" | "FMT001" | "FMT002" | "TXT005" | "WS001" | "PRJ005"
    )
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
        assert!(!code_is_enabled("FMT001", &[], &[], false));
        assert!(!code_is_enabled("FMT002", &[], &[], false));
        assert!(!code_is_enabled("TXT003", &[], &[], false));
        assert!(!code_is_enabled("TXT004", &[], &[], false));
        assert!(!code_is_enabled("CMT001", &[], &[], false));
        assert!(!code_is_enabled("WS001", &[], &[], false));
    }

    #[test]
    fn strict_only_rules_need_strict_or_select() {
        assert!(!code_is_enabled("CIT009", &[], &[], false));
        assert!(code_is_enabled("CIT009", &[], &[], true));
        assert!(code_is_enabled(
            "CIT009",
            &[String::from("CIT009")],
            &[],
            false
        ));

        assert!(!code_is_enabled("CIT010", &[], &[], false));
        assert!(code_is_enabled("CIT010", &[], &[], true));
        assert!(code_is_enabled(
            "CIT010",
            &[String::from("CIT010")],
            &[],
            false
        ));

        assert!(!code_is_enabled("CIT011", &[], &[], false));
        assert!(code_is_enabled("CIT011", &[], &[], true));
        assert!(code_is_enabled(
            "CIT011",
            &[String::from("CIT011")],
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

        assert!(!code_is_enabled("WS001", &[], &[], false));
        assert!(code_is_enabled("WS001", &[], &[], true));
        assert!(code_is_enabled(
            "WS001",
            &[String::from("WS001")],
            &[],
            false
        ));
    }

    #[test]
    fn prj_rules_enabled_by_default() {
        assert!(code_is_enabled("PRJ001", &[], &[], false));
    }
}
