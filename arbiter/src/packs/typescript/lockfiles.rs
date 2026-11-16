// The packages a lockfile resolves, each with its version, read in the
// lockfile's own format.
pub fn locked(name: &str, text: &str) -> Vec<(String, String)> {
    if name == "package-lock.json" {
        npm_locked(text)
    } else {
        pnpm_locked(text)
    }
}

// pnpm writes YAML: under `packages:`, each key is name@version, quoted when
// scoped, any peer qualifiers in parentheses; older locks prefix a slash.
fn pnpm_locked(text: &str) -> Vec<(String, String)> {
    let mut found = Vec::new();
    let mut inside = false;
    for line in text.lines() {
        if !line.starts_with(' ') && !line.trim().is_empty() {
            inside = line.trim_end() == "packages:";
            continue;
        }
        if !inside || !line.starts_with("  ") || line.starts_with("   ") {
            continue;
        }
        let Some(key) = line.trim().strip_suffix(':') else {
            continue;
        };
        let key = key.trim_matches(['\'', '"']).trim_start_matches('/');
        let key = key.split('(').next().unwrap_or(key);
        let Some(at) = key.rfind('@') else {
            continue;
        };
        if at == 0 {
            continue;
        }
        found.push((key[..at].to_string(), key[at + 1..].to_string()));
    }
    found
}

// npm writes JSON: under "packages", each key is an install path whose last
// node_modules/ segment names the package; a workspace link carries no version.
// A copy nested inside another package's node_modules is that party's own and
// unreachable from any unit, so only single-segment paths count as resolved.
fn npm_locked(text: &str) -> Vec<(String, String)> {
    let Ok(parsed) = serde_json::from_str::<serde_json::Value>(text) else {
        return Vec::new();
    };
    let Some(packages) = parsed
        .get("packages")
        .and_then(serde_json::Value::as_object)
    else {
        return Vec::new();
    };
    let mut found = Vec::new();
    for (key, entry) in packages {
        if key.matches("node_modules/").count() != 1 {
            continue;
        }
        let Some(position) = key.rfind("node_modules/") else {
            continue;
        };
        if entry
            .get("link")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false)
        {
            continue;
        }
        let Some(version) = entry.get("version").and_then(serde_json::Value::as_str) else {
            continue;
        };
        let name = &key[position + "node_modules/".len()..];
        found.push((name.to_string(), version.to_string()));
    }
    found
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_pnpm_lock_yields_each_package_with_peers_stripped() {
        let lock = "lockfileVersion: '9.0'\n\nimporters:\n  .:\n    dependencies:\n      \
                    lodash:\n        specifier: ^4.0.0\n        version: 4.17.21\n\n\
                    packages:\n\n  '@scope/kit@2.0.1':\n    resolution: {integrity: sha512-a}\n\n  \
                    lodash@4.17.21:\n    resolution: {integrity: sha512-b}\n\n  \
                    react-dom@18.2.0(react@18.2.0):\n    resolution: {integrity: sha512-c}\n";
        assert_eq!(
            super::pnpm_locked(lock),
            [
                ("@scope/kit".to_string(), "2.0.1".to_string()),
                ("lodash".to_string(), "4.17.21".to_string()),
                ("react-dom".to_string(), "18.2.0".to_string()),
            ]
        );
    }

    #[test]
    fn an_npm_lock_keeps_what_units_reach_and_skips_links_and_nested_copies() {
        let lock = r#"{
          "packages": {
            "": { "name": "web", "version": "0.0.0" },
            "node_modules/@demo/ui": { "resolved": "../ui", "link": true },
            "node_modules/lodash": { "version": "4.17.21" },
            "apps/x/node_modules/lodash": { "version": "4.18.0" },
            "node_modules/a/node_modules/lodash": { "version": "4.17.20" }
          }
        }"#;
        assert_eq!(
            super::npm_locked(lock),
            [
                ("lodash".to_string(), "4.18.0".to_string()),
                ("lodash".to_string(), "4.17.21".to_string()),
            ]
        );
    }
}
