// The declared unit graph, the user's: which units may use which.
pub const DECLARATIONS: &str = ".arbiter/units.toml";

// An exact unit path, or a folder and star covering every unit below it.
pub fn fits(pattern: &str, unit: &str) -> bool {
    if pattern == "*" {
        return true;
    }
    match pattern.strip_suffix("/*") {
        Some(base) => unit == base || unit.starts_with(&format!("{base}/")),
        None => unit == pattern,
    }
}
