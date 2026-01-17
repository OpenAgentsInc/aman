//! Region normalization helpers.

pub fn normalize_region_input(raw: &str) -> String {
    let cleaned: String = raw
        .trim()
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '+' || c == '-' || c == ' ' {
                c.to_ascii_lowercase()
            } else {
                ' '
            }
        })
        .collect();

    cleaned
        .split_whitespace()
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn canonicalize_region(cleaned: &str) -> String {
    match cleaned {
        "ir" | "iran" | "iranian" | "islamic republic of iran" => "iran".to_string(),
        "ug" | "uganda" => "uganda".to_string(),
        "ve" | "venezuela" => "venezuela".to_string(),
        "sy" | "syria" => "syria".to_string(),
        "lb" | "lebanon" => "lebanon".to_string(),
        "vpn iran" | "iran vpn" | "vpn+iran" | "vpn-iran" => "vpn+iran".to_string(),
        other => other.to_string(),
    }
}
