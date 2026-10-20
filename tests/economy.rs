use arbiter::graph::{graph, integrity, judge};

const DEMO: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/demo");

#[test]
fn the_fixture_yields_its_known_findings() {
    let found = graph(DEMO).expect("the fixture folder exists");
    let shown: Vec<String> = judge(&found.defs, &found.edges, &found.repo)
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
            "proto/billing/invoice.proto:4 Ghost no user anywhere",
            "proto/billing/invoice.proto:5 Billing no user anywhere",
            "proto/billing/invoice.proto:6 GetInvoice no user anywhere",
            "web/src/index.ts:1 ghost no user anywhere",
        ]
    );
}

#[test]
fn integrity_names_the_cycle_and_each_reexport() {
    let found = graph(DEMO).expect("the fixture folder exists");
    let shown: Vec<String> = integrity(&found)
        .iter()
        .map(|finding| format!("{} {}", finding.file.to_string_lossy(), finding.reason))
        .collect();
    assert_eq!(
        shown,
        [
            ".arbiter/units.toml app declares libs/facade and never uses it",
            "app/src/app/alias.py imported as v; a symbol keeps its own name",
            "app/src/app/web.py requests is imported, and no manifest declares it",
            "libs/cyc_a units import each other: libs/cyc_a, libs/cyc_b",
            "libs/cyc_a/src/cyc_a/calc.py libs/cyc_a uses libs/cyc_b, \
             which only nothing/* may",
            "libs/dead_py/uv.lock requests is locked at 2.31.0 in libs/dead_py/uv.lock \
             and 2.32.0 in libs/used_py/uv.lock",
            "libs/facade/src/facade/__init__.py __init__.py imports shine; \
             a symbol is imported from the module defining it",
            "libs/facade/src/facade/inner.py a comment names dashboard_frontend, \
             which nothing in the file defines, uses or quotes",
            "web/src/barrel.ts re-exports from \"./index\"; a module offers only its own names",
        ]
    );
}

#[test]
fn a_used_symbol_keeps_its_resolved_user() {
    let found = graph(DEMO).expect("the fixture folder exists");
    let value = found
        .defs
        .iter()
        .position(|def| def.name == "value")
        .expect("the fixture defines value");
    assert!(found
        .edges
        .iter()
        .any(|edge| edge.def == value && edge.file.to_string_lossy() == "app/src/app/main.py"));
}

#[test]
fn every_manifest_declares_a_unit() {
    let found = graph(DEMO).expect("the fixture folder exists");
    let paths: Vec<String> = found
        .repo
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

#[test]
fn a_whole_module_import_reaches_the_name_through_its_chain() {
    let found = graph(DEMO).expect("the fixture folder exists");
    let value = found
        .defs
        .iter()
        .position(|def| def.name == "value")
        .expect("the fixture defines value");
    assert!(found
        .edges
        .iter()
        .any(|edge| edge.def == value && edge.file.to_string_lossy() == "app/src/app/plain.py"));
}

#[test]
fn the_ledger_counts_a_named_suppression_and_refuses_a_blanket() {
    let found = graph(DEMO).expect("the fixture folder exists");
    let counted = arbiter::ledger::counted(&found);
    assert_eq!(counted.len(), 1);
    assert!(counted[0].text.contains("noqa: PLR2004"));
    let blankets = arbiter::ledger::blankets(&found);
    assert_eq!(blankets.len(), 1);
    assert!(blankets[0].text.contains("type: ignore"));
}

#[test]
fn a_contract_is_reached_through_generated_code_in_both_languages() {
    let found = graph(DEMO).expect("the fixture folder exists");
    let invoice = found
        .defs
        .iter()
        .position(|def| def.name == "Invoice" && def.file.to_string_lossy().ends_with(".proto"))
        .expect("the fixture declares Invoice");
    for user in ["app/src/app/rpc.py", "web/src/rpc.ts"] {
        assert!(
            found
                .edges
                .iter()
                .any(|edge| edge.def == invoice && edge.file.to_string_lossy() == user),
            "missing user {user}"
        );
    }
}
