use crate::wip::scope::Scope;
use crate::wip::task::Task;
use crate::wip::trace::outcome::Failure;
use crate::wip_domain::ecosystem::Config;
use crate::wip_domain::ecosystem::Ecosystem;

// A record as this binary was built with it, beside the config files its
// tools are pointed at.
struct Record {
    path: &'static str,
    text: &'static str,
    configs: &'static [(&'static str, &'static str)],
}

const RECORDS: [Record; 4] = [
    Record {
        path: "packs/proto/ecosystem.toml",
        text: include_str!("../../packs/proto/ecosystem.toml"),
        configs: &[],
    },
    Record {
        path: "packs/python/ecosystem.toml",
        text: include_str!("../../packs/python/ecosystem.toml"),
        configs: &[("ruff.toml", include_str!("../../packs/python/ruff.toml"))],
    },
    Record {
        path: "packs/rust/ecosystem.toml",
        text: include_str!("../../packs/rust/ecosystem.toml"),
        configs: &[],
    },
    Record {
        path: "packs/typescript/ecosystem.toml",
        text: include_str!("../../packs/typescript/ecosystem.toml"),
        configs: &[(
            "biome.jsonc",
            include_str!("../../packs/typescript/biome.jsonc"),
        )],
    },
];

pub fn load_ecosystems_task() -> Task {
    Task::new("load_ecosystems", |scope: &mut Scope, (): ()| {
        let ecosystems = load_ecosystems()?;
        let tools: usize = ecosystems
            .iter()
            .map(|ecosystem| ecosystem.tools.len())
            .sum();
        scope.measure("ecosystems", ecosystems.len() as f64);
        scope.measure("tools", tools as f64);
        Ok(ecosystems)
    })
    .line(
        "every ecosystem this binary was built with, as its record declares it: its unit \
         marker, its members-only rule, its tools and the config files they are pointed at",
    )
}

// In the order of their names.
pub fn load_ecosystems() -> Result<Vec<Ecosystem>, Failure> {
    let mut ecosystems = Vec::with_capacity(RECORDS.len());
    for record in &RECORDS {
        let mut ecosystem: Ecosystem = toml::from_str(record.text).map_err(|error| {
            let message: Vec<&str> = error.message().lines().collect();
            Failure::from(format!(
                "the built-in record {} does not read: {}",
                record.path,
                message.join(", ")
            ))
        })?;
        ecosystem.configs = record
            .configs
            .iter()
            .map(|(name, content)| Config {
                name: name.to_string(),
                content: content.to_string(),
            })
            .collect();
        ecosystems.push(ecosystem);
    }
    Ok(ecosystems)
}

#[cfg(test)]
mod tests {
    use std::env;

    use super::load_ecosystems;
    use super::load_ecosystems_task;
    use crate::wip::job::cancel::Cancel;
    use crate::wip::job::id::JobId;
    use crate::wip::job::id::RunId;
    use crate::wip::layout::Site;
    use crate::wip::scope::Scope;
    use crate::wip::trace::outcome::Outcome;
    use crate::wip_domain::ecosystem::Ecosystem;
    use crate::wip_domain::ecosystem::Reader;

    #[test]
    fn every_record_built_in_loads_its_name_marker_and_rule_as_written() {
        let loaded = load_ecosystems().expect("the records are read");
        let told: Vec<(&str, &str, String)> = loaded
            .iter()
            .map(|ecosystem| {
                let rule = match &ecosystem.members_only {
                    None => "never".to_string(),
                    Some(rule) => format!(
                        "has {}; lacks {}",
                        rule.has.join(", "),
                        rule.lacks.join(", ")
                    ),
                };
                (ecosystem.name.as_str(), ecosystem.marker.as_str(), rule)
            })
            .collect();
        assert_eq!(
            told,
            [
                ("proto", "buf.yaml", "never".to_string()),
                (
                    "python",
                    "pyproject.toml",
                    "has tool.uv.workspace; lacks project".to_string()
                ),
                (
                    "rust",
                    "Cargo.toml",
                    "has workspace; lacks package".to_string()
                ),
                (
                    "typescript",
                    "package.json",
                    "has workspaces; lacks main, exports, bin".to_string()
                ),
            ]
        );
    }

    #[test]
    fn every_record_names_each_of_its_tools_in_the_order_written() {
        let loaded = load_ecosystems().expect("the records are read");
        let told: Vec<(&str, Vec<&str>)> = loaded
            .iter()
            .map(|ecosystem| {
                let tools = ecosystem
                    .tools
                    .iter()
                    .map(|tool| tool.name.as_str())
                    .collect();
                (ecosystem.name.as_str(), tools)
            })
            .collect();
        assert_eq!(
            told,
            [
                ("proto", vec!["format_proto", "lint_proto"]),
                (
                    "python",
                    vec![
                        "format_python",
                        "lint_python",
                        "check_python_types",
                        "test_python"
                    ]
                ),
                ("rust", vec!["format_rust", "lint_rust", "test_rust"]),
                (
                    "typescript",
                    vec![
                        "lint_typescript",
                        "check_typescript_types",
                        "test_typescript"
                    ]
                ),
            ]
        );
        assert_eq!(loaded[2].tools[0].run, ["cargo", "fmt", "--check"]);
    }

    #[test]
    fn every_record_carries_the_config_files_its_tools_are_pointed_at() {
        let loaded = load_ecosystems().expect("the records are read");
        let told: Vec<(&str, Vec<&str>)> = loaded
            .iter()
            .map(|ecosystem| {
                let configs = ecosystem
                    .configs
                    .iter()
                    .map(|config| config.name.as_str())
                    .collect();
                (ecosystem.name.as_str(), configs)
            })
            .collect();
        assert_eq!(
            told,
            [
                ("proto", Vec::new()),
                ("python", vec!["ruff.toml"]),
                ("rust", Vec::new()),
                ("typescript", vec!["biome.jsonc"]),
            ]
        );
        for ecosystem in &loaded {
            let pointed = ecosystem
                .tools
                .iter()
                .any(|tool| tool.run.iter().any(|word| word.contains("{pack}")));
            assert_eq!(
                pointed,
                !ecosystem.configs.is_empty(),
                "{} points its tools at config files exactly when it carries some",
                ecosystem.name
            );
            assert!(ecosystem
                .configs
                .iter()
                .all(|config| !config.content.is_empty()));
        }
    }

    #[test]
    fn only_the_rust_lint_has_its_findings_read_and_from_the_json_cargo_prints() {
        let loaded = load_ecosystems().expect("the records are read");
        let read: Vec<(&str, Reader, String, Option<String>)> = loaded
            .iter()
            .flat_map(|ecosystem| ecosystem.tools.iter())
            .filter_map(|tool| {
                tool.findings.as_ref().map(|readable| {
                    (
                        tool.name.as_str(),
                        readable.reader,
                        readable.run.join(" "),
                        readable.root.as_ref().map(|root| root.join(" ")),
                    )
                })
            })
            .collect();
        assert_eq!(
            read,
            [(
                "lint_rust",
                Reader::CargoJson,
                "cargo clippy --all-targets --quiet --message-format=json -- -D warnings"
                    .to_string(),
                Some("cargo locate-project --workspace --message-format json".to_string())
            )]
        );
    }

    #[test]
    fn the_task_reads_nothing_leaves_the_ecosystems_and_counts_them() {
        let task = load_ecosystems_task();
        assert!(task.reads.is_empty());
        let site = Site {
            dir: env::temp_dir(),
            store: env::temp_dir(),
        };
        let mut scope = Scope::new(
            JobId::new(RunId::next(), "load_ecosystems", 1),
            Cancel::new(),
            site,
        );
        let bytes = task
            .perform(&mut scope, Vec::new())
            .expect("the records are read");
        let left: Vec<Ecosystem> = serde_json::from_slice(&bytes).expect("the product decodes");
        assert_eq!(left, load_ecosystems().expect("the records are read"));
        let step = scope.finished("load_ecosystems", Outcome::Ok);
        assert_eq!(step.measures.get("ecosystems"), Some(&4.0));
        assert_eq!(step.measures.get("tools"), Some(&12.0));
    }
}
