use arbiter::graph::{graph, judge};

const DEMO: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/demo");

#[test]
fn the_fixture_yields_its_known_findings() {
    let (repo, defs, edges) = graph(DEMO).expect("the fixture folder exists");
    let shown: Vec<String> = judge(&defs, &edges, &repo)
        .iter()
        .map(|finding| {
            format!(
                "{}:{} {} {}",
                finding.file.to_string_lossy(),
                finding.line,
                finding.name,
                finding.reason
            )
        })
        .collect();
    assert_eq!(
        shown,
        [
            "libs/dead_py/src/dead/calc.py:1 orphan no user anywhere",
            "libs/testy/src/testy/calc.py:1 helper no user but tests",
            "web/src/index.ts:1 ghost no user anywhere",
        ]
    );
}

#[test]
fn a_used_symbol_keeps_its_resolved_user() {
    let (_, defs, edges) = graph(DEMO).expect("the fixture folder exists");
    let value = defs
        .iter()
        .position(|def| def.name == "value")
        .expect("the fixture defines value");
    assert!(edges
        .iter()
        .any(|edge| edge.def == value && edge.file.to_string_lossy() == "app/src/app/main.py"));
}

#[test]
fn every_manifest_declares_a_unit() {
    let (repo, _, _) = graph(DEMO).expect("the fixture folder exists");
    let paths: Vec<String> = repo
        .units
        .iter()
        .map(|unit| unit.path.to_string_lossy().to_string())
        .collect();
    for expected in ["app", "libs/used_py", "libs/dead_py", "libs/testy", "web"] {
        assert!(
            paths.contains(&expected.to_string()),
            "missing unit {expected}"
        );
    }
}
