use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;

const RECORDS: [&str; 4] = [
    include_str!("../../packs/proto/ecosystem.toml"),
    include_str!("../../packs/python/ecosystem.toml"),
    include_str!("../../packs/rust/ecosystem.toml"),
    include_str!("../../packs/typescript/ecosystem.toml"),
];

// A language with the tools around it, as its record declares it: the name it
// goes by, the file whose presence in a folder makes that folder one of its
// units, and how to tell such a file that only lists members.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Ecosystem {
    pub name: String,
    pub marker: String,
    pub members_only: Option<MembersOnly>,
}

// A marker only lists member units, holding no code of its own, when it has
// every key named first and none named after. A key inside a table is
// written by its path, with dots.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MembersOnly {
    pub has: Vec<String>,
    pub lacks: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Ecosystems {
    pub ecosystems: Vec<Ecosystem>,
}

// Every ecosystem this binary was built with, in the order of their names.
pub fn load_ecosystems() -> Result<Ecosystems, toml::de::Error> {
    let ecosystems = RECORDS
        .iter()
        .map(|record| toml::from_str(record))
        .collect::<Result<Vec<Ecosystem>, _>>()?;
    Ok(Ecosystems { ecosystems })
}

impl MembersOnly {
    pub fn fits(&self, marker: &Value) -> bool {
        self.has.iter().all(|key| holds(marker, key))
            && !self.lacks.iter().any(|key| holds(marker, key))
    }
}

fn holds(document: &Value, key: &str) -> bool {
    key.split('.')
        .try_fold(document, |at, name| at.get(name))
        .is_some()
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::load_ecosystems;
    use super::Ecosystem;
    use super::MembersOnly;

    fn rule(has: &[&str], lacks: &[&str]) -> MembersOnly {
        MembersOnly {
            has: has.iter().map(|key| key.to_string()).collect(),
            lacks: lacks.iter().map(|key| key.to_string()).collect(),
        }
    }

    fn ecosystem(name: &str, marker: &str, members_only: Option<MembersOnly>) -> Ecosystem {
        Ecosystem {
            name: name.to_string(),
            marker: marker.to_string(),
            members_only,
        }
    }

    #[test]
    fn every_record_built_in_loads_as_it_is_written() {
        let loaded = load_ecosystems().expect("the records are read");
        assert_eq!(
            loaded.ecosystems,
            [
                ecosystem("proto", "buf.yaml", None),
                ecosystem(
                    "python",
                    "pyproject.toml",
                    Some(rule(&["tool.uv.workspace"], &["project"]))
                ),
                ecosystem(
                    "rust",
                    "Cargo.toml",
                    Some(rule(&["workspace"], &["package"]))
                ),
                ecosystem(
                    "typescript",
                    "package.json",
                    Some(rule(&["workspaces"], &["main", "exports", "bin"]))
                ),
            ]
        );
    }

    #[test]
    fn a_rule_fits_a_marker_holding_every_key_it_asks_for_and_none_it_bars() {
        let rule = rule(&["workspace"], &["package"]);
        assert!(rule.fits(&json!({"workspace": {"members": ["a"]}})));
        assert!(
            !rule.fits(&json!({"workspace": {}, "package": {"name": "a"}})),
            "a barred key is held"
        );
        assert!(
            !rule.fits(&json!({"package": {"name": "a"}})),
            "the key asked for is missing"
        );
        assert!(!rule.fits(&json!({})));
    }

    #[test]
    fn a_rule_barring_nothing_asks_only_for_its_keys() {
        let rule = rule(&["modules"], &[]);
        assert!(rule.fits(&json!({"modules": [], "package": {}})));
        assert!(!rule.fits(&json!({"package": {}})));
    }

    #[test]
    fn a_key_inside_a_table_is_reached_by_its_path() {
        let rule = rule(&["tool.uv.workspace"], &["project"]);
        assert!(rule.fits(&json!({"tool": {"uv": {"workspace": {"members": []}}}})));
        assert!(!rule.fits(&json!({"tool": {"uv": {"sources": {}}}})));
        assert!(
            !rule.fits(&json!({"tool": "uv"})),
            "a value that is no table holds no key"
        );
        assert!(
            !rule.fits(&json!({"tool.uv.workspace": {}})),
            "one key spelled with dots is another key"
        );
    }
}
