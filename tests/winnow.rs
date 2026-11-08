use arbiter::core::graph::graph;
use arbiter::judge::findings::integrity;
use arbiter::judge::findings::winnow;

const DEMO: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/demo");

#[test]
fn the_fixture_yields_its_known_findings() {
    let found = graph(DEMO).expect("the fixture folder exists");
    let shown: Vec<String> = winnow(&found.defs, &found.edges, &found.repo, &Default::default())
        .0
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
            "libs/dead_py/src/dead/calc.py:1 dead-symbol orphan has no user anywhere",
            "libs/testy/src/testy/calc.py:1 tests-only-symbol helper has no user but tests",
            "native/geo.h:5 dead-symbol Shape has no user anywhere",
            "native/geo.h:10 dead-symbol diameter has no user anywhere",
            "native/geo.h:14 dead-symbol perimeter has no user anywhere",
            "proto/billing/invoice.proto:4 dead-symbol Ghost has no user anywhere",
            "proto/billing/invoice.proto:5 dead-symbol Billing has no user anywhere",
            "proto/billing/invoice.proto:6 dead-symbol GetInvoice has no user anywhere",
            "web/src/index.ts:1 dead-symbol ghost has no user anywhere",
        ]
    );
}

#[test]
fn every_finding_names_a_law_the_matrix_drills() {
    let found = graph(DEMO).expect("the fixture folder exists");
    let mut all = winnow(&found.defs, &found.edges, &found.repo, &Default::default()).0;
    all.extend(integrity(&found));
    all.extend(arbiter::judge::ledger::blanket_findings(&found));
    all.extend(arbiter::judge::claims::vanished(&found));
    all.extend(arbiter::judge::brief::findings(&found));
    assert!(!all.is_empty());
    for finding in &all {
        assert!(
            arbiter::core::pack::drillable(&finding.name),
            "{} is a finding name no rule answers",
            finding.name
        );
    }
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
            ".arbiter/units.toml app grants ghost, which never uses it",
            ".arbiter/units.toml app declares libs/facade and never uses it",
            "app/pyproject.toml rich is declared, and nothing in the unit reaches it",
            "app/src/app/alias.py imported as v; a symbol keeps its own name",
            "app/src/app/doors.py Satellite stands where Port is asked and names no \
             such base; a protocol satisfied by shape is still named",
            "app/src/app/ghost_rpc.py app.gen.phantom_pb2 names no contract; \
             a generated import answers for the contract it was generated from",
            "app/src/app/web.py requests is imported, \
             and the unit's manifest does not declare it",
            "app/src/app/web.py imports a module through a computed value; \
             a module is named by a literal",
            "infra/deploy.yaml registry.local/app:1.2.3 is pulled by tag; \
             an image is named by digest, never by a tag alone",
            "infra/deploy.yaml registry.local/cache:latest is pulled by tag; \
             an image is named by digest, never by a tag alone",
            "infra/deploy.yaml a comment names billing_gateway, \
             which nothing in the file defines, uses or quotes",
            "libs/cyc_a units import each other: libs/cyc_a, libs/cyc_b",
            "libs/cyc_a/src/cyc_a/calc.py libs/cyc_a uses libs/cyc_b, \
             which only nothing/* may",
            "libs/dead_py/uv.lock requests is locked at 2.31.0 in libs/dead_py/uv.lock \
             and 2.32.0 in libs/used_py/uv.lock",
            "libs/facade/src/facade/__init__.py __init__.py imports shine; \
             a symbol is imported from the module defining it",
            "libs/facade/src/facade/inner.py a comment names dashboard_frontend, \
             which nothing in the file defines, uses or quotes",
            "libs/testy/tests/test_calc.py asserts True; \
             an assertion compares the system to an expectation",
            "proto/buf.yaml buf.build/demo/never is declared, \
             and nothing in the unit reaches it",
            "rlib/Cargo.toml itoa is declared, and nothing in the unit reaches it",
            "rlib/src/lib.rs names 2 symbols in one import; \
             an import line names one symbol and the module it comes from",
            "rlib/src/lib.rs imported as measured; a symbol keeps its own name",
            "rlib/src/lib.rs re-exports shapes::area; a module offers only its own names",
            "rlib/src/lib.rs asserts 4 equals itself; \
             an assertion compares the system to an expectation",
            "ui/package.json lodash is declared, and nothing in the unit reaches it",
            "ui/pnpm-lock.yaml lodash is locked at 4.17.20 in ui/pnpm-lock.yaml \
             and 4.17.21 in web/package-lock.json",
            "ui/src/index.ts re-exports every name from \"./extra\"; \
             a surface names each symbol it offers",
            "ui/src/kit.ts defines Kit; a surface offers only names defined elsewhere",
            "ui/src/parts/index.ts re-exports from \"./button\"; \
             a module offers only its own names",
            "ui/src/widget.ts enters its own unit at ui/src/index.ts; \
             a unit's own code imports from the defining file",
            "web/src/__tests__/panel.test.ts lodash is imported, \
             and the unit's manifest does not declare it",
            "web/src/__tests__/panel.test.ts asserts 1 equals itself; \
             an assertion compares the system to an expectation",
            "web/src/__tests__/panel.test.ts a test the tree cannot name; \
             a claim speaks a literal sentence",
            "web/src/barrel.ts re-exports from \"./index\"; a module offers only its own names",
            "web/src/boot.ts bootpkg is imported, and the unit's manifest does not declare it",
            "web/src/boot.ts sparkpkg is imported, and the unit's manifest does not declare it",
            "web/src/boot.ts effectpkg is imported, and the unit's manifest does not declare it",
            "web/src/deep.ts reaches ui/src/parts/button.ts inside ui; \
             a unit is entered through the routes its manifest declares",
            "web/src/doorway.ts Dish stands where Beam is asked and names no such \
             base; a protocol satisfied by shape is still named",
            "web/src/ghost_rpc.ts ./gen/phantom_pb names no contract; \
             a generated import answers for the contract it was generated from",
            "web/src/lazy.ts imports a module through a computed value; \
             a module is named by a literal",
            "web/src/panel.ts names 3 symbols in one import; \
             an import line names one symbol and the module it comes from",
            "web/src/panel.ts mockery is imported by shipped code \
             and declared only under devDependencies",
            "web/src/sneak.ts goes into @demo/ui past its routes; \
             a unit is entered through the routes its manifest declares",
        ]
    );
}

#[test]
fn a_third_party_declared_and_never_reached_is_named() {
    let found = graph(DEMO).expect("the fixture folder exists");
    let shown: Vec<String> = integrity(&found)
        .iter()
        .filter(|finding| finding.name == "unused-dependency")
        .map(|finding| format!("{} {}", finding.file.to_string_lossy(), finding.reason))
        .collect();
    assert_eq!(
        shown,
        [
            "app/pyproject.toml rich is declared, and nothing in the unit reaches it",
            "proto/buf.yaml buf.build/demo/never is declared, \
             and nothing in the unit reaches it",
            "rlib/Cargo.toml itoa is declared, and nothing in the unit reaches it",
            "ui/package.json lodash is declared, and nothing in the unit reaches it",
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
fn a_name_crosses_units_through_the_route_and_its_barrels() {
    let found = graph(DEMO).expect("the fixture folder exists");
    for name in ["Button", "Chip"] {
        let def = found
            .defs
            .iter()
            .position(|def| def.name == name)
            .expect("the fixture defines the name");
        assert!(
            found
                .edges
                .iter()
                .any(|edge| edge.def == def && edge.file.to_string_lossy() == "web/src/panel.ts"),
            "missing edge for {name}"
        );
    }
}

#[test]
fn a_namespace_import_keeps_the_whole_surface() {
    let found = graph(DEMO).expect("the fixture folder exists");
    let badge = found
        .defs
        .iter()
        .position(|def| def.name == "Badge")
        .expect("the fixture defines Badge");
    assert!(found
        .edges
        .iter()
        .any(|edge| edge.def == badge && edge.file.to_string_lossy() == "web/src/boot.ts"));
}

#[test]
fn the_ledger_counts_a_named_suppression_and_refuses_a_blanket() {
    let found = graph(DEMO).expect("the fixture folder exists");
    let counted = arbiter::judge::ledger::counted(&found);
    let texts: Vec<&str> = counted
        .iter()
        .map(|suppression| suppression.text.as_str())
        .collect();
    assert_eq!(counted.len(), 5);
    assert!(texts.iter().any(|text| text.contains("noqa: PLR2004")));
    assert!(texts.contains(&"@pytest.mark.skip(reason=\"a rogue skip\")"));
    assert!(texts.contains(&"#[ignore]"));
    assert!(texts.contains(&"it.skip"));
    assert!(texts.contains(&"buf:lint:ignore ENUM_VALUE_PREFIX"));
    let blankets = arbiter::judge::ledger::blankets(&found);
    assert_eq!(blankets.len(), 2);
    assert!(blankets
        .iter()
        .any(|suppression| suppression.text.contains("type: ignore")));
    assert!(blankets
        .iter()
        .any(|suppression| suppression.text == "buf:lint:ignore"));
}

#[test]
fn a_contract_is_reached_through_generated_code_in_both_languages() {
    let found = graph(DEMO).expect("the fixture folder exists");
    let invoice = found
        .defs
        .iter()
        .position(|def| def.name == "Invoice" && def.file.to_string_lossy().ends_with(".proto"))
        .expect("the fixture declares Invoice");
    for user in [
        "app/src/app/rpc.py",
        "app/src/app/deep_rpc.py",
        "web/src/rpc.ts",
    ] {
        assert!(
            found
                .edges
                .iter()
                .any(|edge| edge.def == invoice && edge.file.to_string_lossy() == user),
            "missing user {user}"
        );
    }
}

#[test]
fn a_named_tool_is_reached_by_invocation_never_unused() {
    use std::fs;
    let root = std::env::temp_dir().join(format!("arbiter-tool-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(root.join(".arbiter")).expect("the folder is writable");
    fs::write(
        root.join("pyproject.toml"),
        "[project]\nname = \"demo\"\nversion = \"0\"\ndependencies = [\"coverage>=7\"]\n",
    )
    .expect("the folder is writable");
    fs::write(root.join("app.py"), "value = 1\n").expect("the folder is writable");

    let bare = graph(&root.to_string_lossy()).expect("the folder exists");
    assert!(
        integrity(&bare)
            .iter()
            .any(|finding| finding.name == "unused-dependency"
                && finding.reason.starts_with("coverage is declared")),
        "an invoked tool nothing imports is unused until the user names it"
    );

    fs::write(
        root.join(".arbiter/units.toml"),
        "[units.\".\"]\ntools = [\"coverage\"]\n",
    )
    .expect("the folder is writable");
    let named = graph(&root.to_string_lossy()).expect("the folder exists");
    let findings = integrity(&named);
    assert!(
        !findings
            .iter()
            .any(|finding| finding.name == "unused-dependency"),
        "a named tool is reached"
    );
    assert!(
        !findings.iter().any(|finding| finding.name == "stale-tool"),
        "a declared tool is not stale"
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn a_rotten_tool_entry_is_named() {
    use std::fs;
    let root = std::env::temp_dir().join(format!("arbiter-rot-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(root.join(".arbiter")).expect("the folder is writable");
    fs::write(
        root.join("pyproject.toml"),
        "[project]\nname = \"demo\"\nversion = \"0\"\n",
    )
    .expect("the folder is writable");
    fs::write(root.join("app.py"), "value = 1\n").expect("the folder is writable");
    fs::write(
        root.join(".arbiter/units.toml"),
        "[units.\".\"]\ntools = [\"ghost-tool\"]\n",
    )
    .expect("the folder is writable");

    let found = graph(&root.to_string_lossy()).expect("the folder exists");
    let rotten: Vec<String> = integrity(&found)
        .iter()
        .filter(|finding| finding.name == "stale-tool")
        .map(|finding| finding.reason.clone())
        .collect();
    assert_eq!(
        rotten,
        ["ghost-tool is named a tool, and pyproject.toml does not declare it"]
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn the_resolved_graph_draws_itself_declared_against_real() {
    use arbiter::judge::findings::drawn;
    use std::fs;
    let root = std::env::temp_dir().join(format!("arbiter-drawn-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    for unit in ["app", "lib"] {
        fs::create_dir_all(root.join(unit).join("src").join(unit)).expect("writable");
        fs::write(
            root.join(unit).join("pyproject.toml"),
            format!("[project]\nname = \"{unit}\"\nversion = \"0\"\n"),
        )
        .expect("writable");
    }
    fs::write(root.join("lib/src/lib/core.py"), "value = 1\n").expect("writable");
    fs::write(
        root.join("app/src/app/main.py"),
        "from lib.core import value\n\nprint(value)\n",
    )
    .expect("writable");

    let bare = graph(&root.to_string_lossy()).expect("the folder exists");
    assert_eq!(
        drawn(&bare),
        [("app".to_string(), "lib".to_string(), false)],
        "an undeclared repository draws its facts unbroken"
    );

    fs::create_dir_all(root.join(".arbiter")).expect("writable");
    fs::write(
        root.join(".arbiter/units.toml"),
        "[units.\"lib\"]\nvisibility = [\"web\"]\n",
    )
    .expect("writable");
    let refused = graph(&root.to_string_lossy()).expect("the folder exists");
    assert_eq!(
        drawn(&refused),
        [("app".to_string(), "lib".to_string(), true)],
        "an edge the declarations refuse is drawn as a break"
    );
    let _ = fs::remove_dir_all(&root);
}
