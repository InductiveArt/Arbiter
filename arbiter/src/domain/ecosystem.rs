use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;

// A language with the tools around it, as its record declares it: the name it
// goes by, the file whose presence in a folder makes that folder one of its
// units, how to tell such a file that only lists members, and the tools its
// units are run through, in the order written.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Ecosystem {
    pub name: String,
    pub marker: String,
    pub members_only: Option<MembersOnly>,
    // A record writes each tool as one `check` entry.
    #[serde(default, alias = "check")]
    pub tools: Vec<Tool>,
    // Never written in the record: the files sit beside it.
    #[serde(default)]
    pub configs: Vec<Config>,
}

// A marker only lists member units, holding no code of its own, when it has
// every key named first and none named after. A key inside a table is
// written by its path, with dots.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MembersOnly {
    pub has: Vec<String>,
    pub lacks: Vec<String>,
}

// One program a unit is run through, as the words that start it, the words
// that repair what it finds when it can, and how its findings are read. A
// word holding `{pack}` is given the folder the config files are written to.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Tool {
    #[serde(rename = "tool")]
    pub name: String,
    pub run: Vec<String>,
    pub fix: Option<Vec<String>>,
    pub findings: Option<ReadableRun>,
}

// The words that make a tool print what one of arbiter's readers reads into
// findings, that reader, and the words printing the root the paths it prints
// run from, when that is not the folder the tool is run in. A word holding
// `{pack}` is given the config folder here too.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ReadableRun {
    pub reader: Reader,
    pub run: Vec<String>,
    pub root: Option<Vec<String>>,
}

// A format of output arbiter reads into findings.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Reader {
    // The JSON lines cargo prints with `--message-format=json`, and the root
    // as `cargo locate-project --message-format json` prints it.
    CargoJson,
}

// A file the tools of an ecosystem read their settings from.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Config {
    pub name: String,
    pub content: String,
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

    use super::Config;
    use super::Ecosystem;
    use super::MembersOnly;
    use super::ReadableRun;
    use super::Reader;
    use super::Tool;

    fn rule(has: &[&str], lacks: &[&str]) -> MembersOnly {
        MembersOnly {
            has: has.iter().map(|key| key.to_string()).collect(),
            lacks: lacks.iter().map(|key| key.to_string()).collect(),
        }
    }

    #[test]
    fn a_record_reads_as_an_ecosystem_with_each_check_entry_one_of_its_tools() {
        let record = "name = \"rust\"\nmarker = \"Cargo.toml\"\n\
                      [members_only]\nhas = [\"workspace\"]\nlacks = [\"package\"]\n\
                      [[check]]\ntool = \"format_rust\"\n\
                      run = [\"cargo\", \"fmt\", \"--check\"]\nfix = [\"cargo\", \"fmt\"]\n";
        let read: Ecosystem = toml::from_str(record).expect("the record is read");
        assert_eq!(
            read,
            Ecosystem {
                name: "rust".to_string(),
                marker: "Cargo.toml".to_string(),
                members_only: Some(rule(&["workspace"], &["package"])),
                tools: vec![Tool {
                    name: "format_rust".to_string(),
                    run: vec![
                        "cargo".to_string(),
                        "fmt".to_string(),
                        "--check".to_string()
                    ],
                    fix: Some(vec!["cargo".to_string(), "fmt".to_string()]),
                    findings: None,
                }],
                configs: Vec::new(),
            }
        );
    }

    #[test]
    fn a_check_entry_may_name_the_run_its_findings_are_read_from_its_reader_and_its_root() {
        let record = "name = \"rust\"\nmarker = \"Cargo.toml\"\n\
                      [[check]]\ntool = \"lint_rust\"\n\
                      run = [\"cargo\", \"clippy\"]\n\
                      findings = { reader = \"cargo_json\", \
                      run = [\"cargo\", \"clippy\", \"--message-format=json\"], \
                      root = [\"cargo\", \"locate-project\"] }\n";
        let read: Ecosystem = toml::from_str(record).expect("the record is read");
        assert_eq!(
            read.tools[0].findings,
            Some(ReadableRun {
                reader: Reader::CargoJson,
                run: vec![
                    "cargo".to_string(),
                    "clippy".to_string(),
                    "--message-format=json".to_string()
                ],
                root: Some(vec!["cargo".to_string(), "locate-project".to_string()]),
            })
        );
    }

    #[test]
    fn a_reading_may_leave_its_root_out() {
        let record = "name = \"x\"\nmarker = \"x.toml\"\n[[check]]\ntool = \"lint_x\"\n\
                      run = [\"x\"]\nfindings = { reader = \"cargo_json\", run = [\"x\"] }\n";
        let read: Ecosystem = toml::from_str(record).expect("the record is read");
        assert_eq!(
            read.tools[0]
                .findings
                .as_ref()
                .map(|readable| &readable.root),
            Some(&None)
        );
    }

    #[test]
    fn a_reader_arbiter_does_not_have_does_not_read() {
        let record = "name = \"x\"\nmarker = \"x.toml\"\n[[check]]\ntool = \"lint_x\"\n\
                      run = [\"x\"]\nfindings = { reader = \"x_json\", run = [\"x\"] }\n";
        let refused = toml::from_str::<Ecosystem>(record).expect_err("no such reader");
        assert!(
            refused.message().contains("x_json"),
            "the refusal names the reader: {}",
            refused.message()
        );
    }

    #[test]
    fn a_check_entry_naming_no_tool_does_not_read() {
        let record = "name = \"x\"\nmarker = \"x.toml\"\n[[check]]\nrun = [\"x\"]\n";
        let refused = toml::from_str::<Ecosystem>(record).expect_err("the entry names no tool");
        assert!(
            refused.message().contains("tool"),
            "the refusal names what is missing: {}",
            refused.message()
        );
    }

    #[test]
    fn the_ecosystems_written_for_a_run_read_back_equal() {
        let ecosystems = vec![Ecosystem {
            name: "python".to_string(),
            marker: "pyproject.toml".to_string(),
            members_only: None,
            tools: vec![Tool {
                name: "lint_python".to_string(),
                run: vec!["ruff".to_string(), "check".to_string()],
                fix: None,
                findings: None,
            }],
            configs: vec![Config {
                name: "ruff.toml".to_string(),
                content: "line-length = 100\n".to_string(),
            }],
        }];
        let written = serde_json::to_vec(&ecosystems).expect("the ecosystems encode");
        let back: Vec<Ecosystem> = serde_json::from_slice(&written).expect("the ecosystems decode");
        assert_eq!(back, ecosystems);
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
