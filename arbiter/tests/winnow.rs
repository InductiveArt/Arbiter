use arbiter::packs::graphed;
use arbiter::tasks::laws;
use arbiter::tasks::laws::economy::winnow;

const DEMO: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/demo");

#[test]
fn the_fixture_yields_its_known_findings() {
    let found = graphed(DEMO).expect("the fixture folder exists");
    let shown: Vec<String> = winnow(&found)
        .iter()
        .map(|finding| {
            format!(
                "{}:{} {} {}",
                finding.file.to_string_lossy(),
                finding.line.unwrap_or(0),
                finding.rule.as_deref().unwrap_or(""),
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
fn a_graph_written_out_and_read_back_finds_what_it_found() {
    use arbiter::core::graph_data::GraphData;
    let found = graphed(DEMO).expect("the fixture folder exists");
    let text = serde_json::to_string(&GraphData::of(&found).expect("the graph writes out"))
        .expect("the graph is json");
    let back: GraphData = serde_json::from_str(&text).expect("the json reads back");
    let again = back
        .into_graph(arbiter::packs::all())
        .expect("each language takes its readings back");
    let sorted = |graph: &arbiter::core::graph::Graph| {
        let mut all: Vec<(String, Option<u32>, Option<String>, String)> = laws::findings(graph)
            .into_iter()
            .map(|finding| {
                (
                    finding.file.to_string_lossy().into_owned(),
                    finding.line,
                    finding.rule,
                    finding.reason,
                )
            })
            .collect();
        all.sort();
        all
    };
    assert_eq!(
        sorted(&again),
        sorted(&found),
        "the same laws find the same"
    );
    let counted = |graph: &arbiter::core::graph::Graph| -> Vec<String> {
        arbiter::ledger::counted(graph)
            .iter()
            .map(|suppression| suppression.text.clone())
            .collect()
    };
    assert_eq!(
        counted(&again),
        counted(&found),
        "the same suppressions count"
    );
    assert_eq!(
        serde_json::to_string(&GraphData::of(&again).expect("the graph writes out again"))
            .expect("the graph is json"),
        text,
        "a graph read back writes out byte for byte as it was"
    );
}

#[test]
fn every_finding_names_a_law_the_matrix_drills() {
    let found = graphed(DEMO).expect("the fixture folder exists");
    let mut all = laws::findings(&found);
    all.extend(arbiter::ledger::blanket_findings(&found));
    assert!(!all.is_empty());
    for finding in &all {
        assert!(
            arbiter::tasks::laws::drillable(finding.rule.as_deref().unwrap_or("")),
            "{} is a finding name no rule answers",
            finding.rule.as_deref().unwrap_or("")
        );
    }
}

#[test]
fn integrity_names_the_cycle_and_each_reexport() {
    let found = graphed(DEMO).expect("the fixture folder exists");
    let shown: Vec<String> = laws::findings(&found)
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
            "libs/dead_py/src/dead/calc.py orphan has no user anywhere",
            "libs/dead_py/src/dead/calc.py # type: ignore quiets more than one named rule",
            "libs/dead_py/uv.lock requests is locked at 2.31.0 in libs/dead_py/uv.lock \
             and 2.32.0 in libs/used_py/uv.lock",
            "libs/facade/src/facade/__init__.py __init__.py imports shine; \
             a symbol is imported from the module defining it",
            "libs/facade/src/facade/inner.py a comment names dashboard_frontend, \
             which nothing in the file defines, uses or quotes",
            "libs/testy/src/testy/calc.py helper has no user but tests",
            "libs/testy/tests/test_calc.py asserts True; \
             an assertion compares the system to an expectation",
            "native/geo.h Shape has no user anywhere",
            "native/geo.h diameter has no user anywhere",
            "native/geo.h perimeter has no user anywhere",
            "proto/billing/invoice.proto Ghost has no user anywhere",
            "proto/billing/invoice.proto Billing has no user anywhere",
            "proto/billing/invoice.proto GetInvoice has no user anywhere",
            "proto/billing/invoice.proto buf:lint:ignore quiets more than one named rule",
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
            "web/src/index.ts ghost has no user anywhere",
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
    let found = graphed(DEMO).expect("the fixture folder exists");
    let shown: Vec<String> = laws::findings(&found)
        .iter()
        .filter(|finding| finding.rule.as_deref() == Some("unused-dependency"))
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
    let found = graphed(DEMO).expect("the fixture folder exists");
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
    let found = graphed(DEMO).expect("the fixture folder exists");
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
    let found = graphed(DEMO).expect("the fixture folder exists");
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
    let found = graphed(DEMO).expect("the fixture folder exists");
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
    let found = graphed(DEMO).expect("the fixture folder exists");
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
    let found = graphed(DEMO).expect("the fixture folder exists");
    let counted = arbiter::ledger::counted(&found);
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
    let blankets = arbiter::ledger::blankets(&found);
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
    let found = graphed(DEMO).expect("the fixture folder exists");
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
fn a_record_suppression_quiets_the_package_it_names() {
    use std::fs;
    let root = std::env::temp_dir().join(format!("arbiter-quiet-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(root.join(".arbiter")).expect("the folder is writable");
    fs::write(
        root.join("pyproject.toml"),
        "[project]\nname = \"demo\"\nversion = \"0\"\ndependencies = [\"coverage>=7\"]\n",
    )
    .expect("the folder is writable");
    fs::write(root.join("app.py"), "value = 1\n").expect("the folder is writable");

    let bare = graphed(&root.to_string_lossy()).expect("the folder exists");
    assert!(
        laws::findings(&bare)
            .iter()
            .any(
                |finding| finding.rule.as_deref() == Some("unused-dependency")
                    && finding.reason.starts_with("coverage is declared")
            ),
        "an uncovered package nothing imports is unused"
    );

    fs::write(
        root.join(".arbiter/suppressions.toml"),
        "[[suppression]]\nrule = \"unused-dependency\"\nlanguage = \"python\"\n\
         packages = [\"coverage\"]\nunits = [\".\"]\n",
    )
    .expect("the folder is writable");
    let quieted = graphed(&root.to_string_lossy()).expect("the folder exists");
    let findings = laws::findings(&quieted);
    assert!(
        !findings
            .iter()
            .any(|finding| finding.rule.as_deref() == Some("unused-dependency")),
        "a suppressed package is quiet"
    );
    assert!(
        !findings
            .iter()
            .any(|finding| finding.rule.as_deref() == Some("stale-suppression")),
        "a suppression that quiets a finding is alive"
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn a_suppression_that_quiets_nothing_is_stale() {
    use std::fs;
    let root = std::env::temp_dir().join(format!("arbiter-stale-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(root.join(".arbiter")).expect("the folder is writable");
    fs::write(
        root.join("pyproject.toml"),
        "[project]\nname = \"demo\"\nversion = \"0\"\n",
    )
    .expect("the folder is writable");
    fs::write(root.join("app.py"), "value = 1\n").expect("the folder is writable");
    fs::write(
        root.join(".arbiter/suppressions.toml"),
        "[[suppression]]\nrule = \"unused-dependency\"\nlanguage = \"python\"\n\
         packages = [\"ghost-package\"]\nunits = [\"*\"]\n",
    )
    .expect("the folder is writable");

    let found = graphed(&root.to_string_lossy()).expect("the folder exists");
    let stale: Vec<String> = laws::findings(&found)
        .iter()
        .filter(|finding| finding.rule.as_deref() == Some("stale-suppression"))
        .map(|finding| finding.reason.clone())
        .collect();
    assert_eq!(
        stale,
        ["the suppression of unused-dependency on ghost-package quiets nothing"]
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn a_record_blanket_is_refused_never_recorded() {
    use std::fs;
    let root = std::env::temp_dir().join(format!("arbiter-blanket-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(root.join(".arbiter")).expect("the folder is writable");
    fs::write(
        root.join(".arbiter/suppressions.toml"),
        "[[suppression]]\nrule = \"dead-symbol\"\nlanguage = \"python\"\n\
         packages = [\"x\"]\nunits = [\"*\"]\n\n\
         [[suppression]]\nrule = \"unused-dependency\"\nlanguage = \"python\"\n\
         packages = [\"*\"]\nunits = [\"*\"]\n",
    )
    .expect("the folder is writable");

    let (subjects, findings) = arbiter::core::record::read(&root);
    assert!(subjects.is_empty(), "a refused entry records nothing");
    let named: Vec<&str> = findings
        .iter()
        .map(|finding| finding.rule.as_deref().unwrap_or(""))
        .collect();
    assert_eq!(named, ["blanket-suppression", "blanket-suppression"]);
    assert!(findings[0]
        .reason
        .contains("dead-symbol is fixed, never recorded"));
    assert!(findings[1].reason.contains("covers every package"));
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn a_record_suppression_is_counted_in_the_account() {
    use std::fs;
    let root = std::env::temp_dir().join(format!("arbiter-account-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(root.join(".arbiter")).expect("the folder is writable");
    fs::write(
        root.join("pyproject.toml"),
        "[project]\nname = \"demo\"\nversion = \"0\"\ndependencies = [\"coverage>=7\"]\n",
    )
    .expect("the folder is writable");
    fs::write(root.join("app.py"), "value = 1\n").expect("the folder is writable");
    fs::write(
        root.join(".arbiter/suppressions.toml"),
        "[[suppression]]\nrule = \"unused-dependency\"\nlanguage = \"python\"\n\
         packages = [\"coverage\"]\nunits = [\".\"]\n",
    )
    .expect("the folder is writable");

    let found = graphed(&root.to_string_lossy()).expect("the folder exists");
    let counted = arbiter::ledger::counted(&found);
    assert_eq!(counted.len(), 1);
    assert_eq!(counted[0].text, "unused-dependency python coverage .");
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn the_resolved_graph_draws_itself_declared_against_real() {
    use arbiter::core::measures::drawn;
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

    let bare = graphed(&root.to_string_lossy()).expect("the folder exists");
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
    let refused = graphed(&root.to_string_lossy()).expect("the folder exists");
    assert_eq!(
        drawn(&refused),
        [("app".to_string(), "lib".to_string(), true)],
        "an edge the declarations refuse is drawn as a break"
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn a_declared_folder_consents_both_ways_inside_its_unit() {
    use std::fs;
    let root = std::env::temp_dir().join(format!("arbiter-folder-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    for folder in ["api", "infra", ".arbiter"] {
        fs::create_dir_all(root.join(folder)).expect("the folder is writable");
    }
    fs::write(
        root.join("pyproject.toml"),
        "[project]\nname = \"demo\"\nversion = \"0\"\n",
    )
    .expect("the folder is writable");
    fs::write(
        root.join("infra/db.py"),
        "def connect() -> int:\n    return 1\n",
    )
    .expect("the folder is writable");
    fs::write(
        root.join("api/handler.py"),
        "from infra.db import connect\n\nprint(connect())\n",
    )
    .expect("the folder is writable");
    fs::write(
        root.join(".arbiter/units.toml"),
        "[units.\".\".inner.\"infra\"]\nvisibility = [\"api\"]\n\n\
         [units.\".\".inner.\"api\"]\ndeps = [\"infra\"]\n",
    )
    .expect("the folder is writable");

    let consenting = graphed(&root.to_string_lossy()).expect("the folder exists");
    let folder_findings = |found: &arbiter::core::graph::Graph| -> Vec<(String, String)> {
        laws::findings(found)
            .iter()
            .filter(|finding| finding.rule.as_deref().unwrap_or("").contains("folder"))
            .map(|finding| {
                (
                    finding.rule.clone().unwrap_or_default(),
                    finding.reason.clone(),
                )
            })
            .collect()
    };
    assert_eq!(
        folder_findings(&consenting),
        [],
        "a granted entry and a declared dep are quiet"
    );

    fs::write(
        root.join(".arbiter/units.toml"),
        "[units.\".\".inner.\"infra\"]\nvisibility = [\"migrations\"]\n\n\
         [units.\".\".inner.\"api\"]\ndeps = [\"domain\"]\n",
    )
    .expect("the folder is writable");
    let refused = graphed(&root.to_string_lossy()).expect("the folder exists");
    let found = folder_findings(&refused);
    let names: Vec<&str> = found.iter().map(|(name, _)| name.as_str()).collect();
    assert!(names.contains(&"undeclared-folder-use"));
    assert!(names.contains(&"uninvited-folder-use"));
    assert!(names.contains(&"stale-folder-grant"));
    assert!(names.contains(&"stale-folder-declaration"));
    assert!(
        found
            .iter()
            .any(|(_, reason)| reason
                == "api/handler.py uses infra/db.py, which api does not declare")
    );
    assert!(found
        .iter()
        .any(|(_, reason)| reason == "api/handler.py uses infra/db.py, which only migrations may"));
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn a_child_folder_only_tightens_what_the_unit_allows() {
    use std::fs;
    let root = std::env::temp_dir().join(format!("arbiter-tighten-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    for unit in ["app", "lib"] {
        fs::create_dir_all(root.join(unit).join("src").join(unit)).expect("writable");
        fs::write(
            root.join(unit).join("pyproject.toml"),
            format!("[project]\nname = \"{unit}\"\nversion = \"0\"\n"),
        )
        .expect("writable");
    }
    fs::create_dir_all(root.join(".arbiter")).expect("writable");
    fs::write(root.join("lib/src/lib/core.py"), "value = 1\n").expect("writable");
    fs::write(
        root.join("app/src/app/main.py"),
        "from lib.core import value\n\nprint(value)\n",
    )
    .expect("writable");

    fs::write(
        root.join(".arbiter/units.toml"),
        "[units.\"app\"]\ndeps = [\"lib\"]\n\n\
         [units.\"app\".inner.\"src/app\"]\ndeps = [\"//lib\"]\n",
    )
    .expect("writable");
    let consenting = graphed(&root.to_string_lossy()).expect("the folder exists");
    assert!(
        !laws::findings(&consenting).iter().any(|finding| finding
            .rule
            .as_deref()
            .unwrap_or("")
            .contains("folder")
            || finding.rule.as_deref().unwrap_or("").contains("unit-")),
        "the unit consents and the folder consents through the root marker"
    );

    fs::write(
        root.join(".arbiter/units.toml"),
        "[units.\"app\"]\ndeps = [\"lib\"]\n\n\
         [units.\"app\".inner.\"src/app\"]\ndeps = [\"src/other\"]\n",
    )
    .expect("writable");
    let tightened = graphed(&root.to_string_lossy()).expect("the folder exists");
    let reasons: Vec<String> = laws::findings(&tightened)
        .iter()
        .filter(|finding| finding.rule.as_deref() == Some("undeclared-folder-use"))
        .map(|finding| finding.reason.clone())
        .collect();
    assert_eq!(
        reasons,
        ["src/app/main.py uses //lib/src/lib/core.py, which src/app does not declare"],
        "the child refuses what the unit allowed: conjunctive, tightening only"
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn a_file_declares_as_a_leaf_of_the_folder_tree() {
    use std::fs;
    let root = std::env::temp_dir().join(format!("arbiter-leaf-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    for folder in ["api", "infra", ".arbiter"] {
        fs::create_dir_all(root.join(folder)).expect("the folder is writable");
    }
    fs::write(
        root.join("pyproject.toml"),
        "[project]\nname = \"demo\"\nversion = \"0\"\n",
    )
    .expect("the folder is writable");
    fs::write(
        root.join("infra/db.py"),
        "def connect() -> int:\n    return 1\n",
    )
    .expect("the folder is writable");
    fs::write(
        root.join("api/handler.py"),
        "from infra.db import connect\n\nprint(connect())\n",
    )
    .expect("the folder is writable");

    fs::write(
        root.join(".arbiter/units.toml"),
        "[units.\".\".inner.\"infra/db.py\"]\nvisibility = [\"api/handler.py\"]\n",
    )
    .expect("the folder is writable");
    let door = graphed(&root.to_string_lossy()).expect("the folder exists");
    assert!(
        !laws::findings(&door).iter().any(|finding| finding
            .rule
            .as_deref()
            .unwrap_or("")
            .contains("folder")),
        "the named file is the one door and the handler holds its key"
    );

    fs::write(
        root.join(".arbiter/units.toml"),
        "[units.\".\".inner.\"infra/db.py\"]\nvisibility = [\"api/other.py\"]\n",
    )
    .expect("the folder is writable");
    let barred = graphed(&root.to_string_lossy()).expect("the folder exists");
    let mut names: Vec<String> = laws::findings(&barred)
        .iter()
        .filter(|finding| finding.rule.as_deref().unwrap_or("").contains("folder"))
        .map(|finding| finding.rule.clone().unwrap_or_default())
        .collect();
    names.sort();
    assert_eq!(names, ["stale-folder-grant", "uninvited-folder-use"]);
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn a_folder_whose_children_cycle_is_named() {
    use std::fs;
    let root = std::env::temp_dir().join(format!("arbiter-sibling-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    for folder in ["api", "infra"] {
        fs::create_dir_all(root.join(folder)).expect("the folder is writable");
    }
    fs::write(
        root.join("pyproject.toml"),
        "[project]\nname = \"demo\"\nversion = \"0\"\n",
    )
    .expect("the folder is writable");
    fs::write(
        root.join("api/a.py"),
        "from infra.b import value\n\nmirror = value\n",
    )
    .expect("the folder is writable");
    fs::write(
        root.join("infra/b.py"),
        "from api.a import mirror\n\nvalue = mirror\n",
    )
    .expect("the folder is writable");

    let found = graphed(&root.to_string_lossy()).expect("the folder exists");
    let cycles: Vec<String> = laws::findings(&found)
        .iter()
        .filter(|finding| finding.rule.as_deref() == Some("sibling-cycle"))
        .map(|finding| finding.reason.clone())
        .collect();
    assert_eq!(cycles, ["the children of . form a cycle: api <-> infra"]);
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn an_exact_member_set_quiets_one_cycle_and_stales_when_it_dissolves() {
    use std::fs;
    let root = std::env::temp_dir().join(format!("arbiter-signedcycle-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    for folder in ["api", "infra", ".arbiter"] {
        fs::create_dir_all(root.join(folder)).expect("the folder is writable");
    }
    fs::write(
        root.join("pyproject.toml"),
        "[project]\nname = \"demo\"\nversion = \"0\"\n",
    )
    .expect("the folder is writable");
    fs::write(
        root.join("api/a.py"),
        "from infra.b import value\n\nmirror = value\n",
    )
    .expect("the folder is writable");
    fs::write(
        root.join("infra/b.py"),
        "from api.a import mirror\n\nvalue = mirror\n",
    )
    .expect("the folder is writable");
    fs::write(
        root.join(".arbiter/suppressions.toml"),
        "[[suppression]]\nrule = \"sibling-cycle\"\nunit = \".\"\nmembers = [\"api\", \"infra\"]\n",
    )
    .expect("the folder is writable");

    let signed = graphed(&root.to_string_lossy()).expect("the folder exists");
    let findings = laws::findings(&signed);
    assert!(
        !findings
            .iter()
            .any(|finding| finding.rule.as_deref() == Some("sibling-cycle")),
        "the exact set is quiet"
    );
    assert!(
        !findings
            .iter()
            .any(|finding| finding.rule.as_deref() == Some("stale-suppression")),
        "a quieting entry is alive"
    );

    fs::write(root.join("infra/b.py"), "value = 1\n").expect("the folder is writable");
    let dissolved = graphed(&root.to_string_lossy()).expect("the folder exists");
    let stale: Vec<String> = laws::findings(&dissolved)
        .iter()
        .filter(|finding| finding.rule.as_deref() == Some("stale-suppression"))
        .map(|finding| finding.reason.clone())
        .collect();
    assert_eq!(
        stale,
        ["the suppression of sibling-cycle in . quiets nothing"]
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn the_tangles_read_lists_entangled_siblings_largest_first() {
    use arbiter::core::measures::tangles;
    use std::fs;
    let root = std::env::temp_dir().join(format!("arbiter-tangles-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    for folder in ["api", "infra"] {
        fs::create_dir_all(root.join(folder)).expect("the folder is writable");
    }
    fs::write(
        root.join("pyproject.toml"),
        "[project]\nname = \"demo\"\nversion = \"0\"\n",
    )
    .expect("the folder is writable");
    fs::write(
        root.join("api/a.py"),
        "from infra.b import value\n\nmirror = value\n",
    )
    .expect("the folder is writable");
    fs::write(
        root.join("infra/b.py"),
        "from api.a import mirror\n\nvalue = mirror\n",
    )
    .expect("the folder is writable");

    let found = graphed(&root.to_string_lossy()).expect("the folder exists");
    assert_eq!(
        tangles(&found),
        [(
            ".".to_string(),
            vec!["api".to_string(), "infra".to_string()]
        )]
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn the_families_of_every_folder_travel_with_the_snapshot() {
    use arbiter::core::measures::families;
    use std::fs;
    let root = std::env::temp_dir().join(format!("arbiter-families-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    for folder in ["api", "infra"] {
        fs::create_dir_all(root.join(folder)).expect("the folder is writable");
    }
    fs::write(
        root.join("pyproject.toml"),
        "[project]\nname = \"demo\"\nversion = \"0\"\n",
    )
    .expect("the folder is writable");
    fs::write(
        root.join("infra/db.py"),
        "def connect() -> int:\n    return 1\n",
    )
    .expect("the folder is writable");
    fs::write(
        root.join("api/handler.py"),
        "from infra.db import connect\n\nprint(connect())\n",
    )
    .expect("the folder is writable");

    let found = graphed(&root.to_string_lossy()).expect("the folder exists");
    let told = families(&found);
    assert!(told.iter().any(|family| family.unit == "."
        && family.place == "."
        && family
            .edges
            .contains(&("api".to_string(), "infra".to_string()))));
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn a_rust_path_reaches_only_the_definition_its_crate_and_modules_name() {
    use std::fs;
    let root = std::env::temp_dir().join(format!("arbiter-paths-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    let manifest = |name: &str, deps: &str| {
        format!("[package]\nname = \"{name}\"\nversion = \"0.1.0\"\nedition = \"2021\"\n{deps}")
    };
    // Two crates whose files and names repeat each other: a path must find
    // its own crate's definition and never the lookalike in the other.
    for (file, content) in [
        ("runner/Cargo.toml", manifest("runner", "")),
        (
            "runner/src/lib.rs",
            "pub mod declaration;\npub mod gate;\npub mod plan;\npub mod test_tasks;\npub mod trees;\n"
                .to_string(),
        ),
        ("runner/src/declaration/mod.rs", "pub mod task;\n".to_string()),
        ("runner/src/declaration/task.rs", "pub struct Task;\n".to_string()),
        ("runner/src/gate.rs", "pub fn gate() -> u8 {\n    3\n}\n".to_string()),
        (
            "runner/src/plan.rs",
            "use crate::declaration::task::Task;\nuse crate::trees;\n\n\
             pub fn planned() -> Task {\n    trees::parse();\n    Task\n}\n"
                .to_string(),
        ),
        ("runner/src/test_tasks/mod.rs", "pub mod tasks;\n".to_string()),
        ("runner/src/test_tasks/tasks.rs", "pub fn tools() -> u8 {\n    0\n}\n".to_string()),
        ("runner/src/trees.rs", "pub fn parse() -> u8 {\n    2\n}\n".to_string()),
        (
            "app/Cargo.toml",
            manifest("app", "\n[dependencies]\nrunner = { path = \"../runner\" }\n"),
        ),
        (
            "app/src/lib.rs",
            "pub mod core;\npub mod gate;\npub mod run;\npub mod tasks;\n\n\
             pub fn entry() -> u8 {\n    gate::gate()\n}\n"
                .to_string(),
        ),
        ("app/src/core/mod.rs", "pub mod task;\npub mod trees;\n".to_string()),
        ("app/src/core/task.rs", "pub struct Task;\n".to_string()),
        ("app/src/core/trees.rs", "pub fn parse() -> u8 {\n    2\n}\n".to_string()),
        (
            "app/src/gate.rs",
            "use crate::core::trees;\n\npub fn gate() -> u8 {\n    trees::parse();\n    \
             crate::tasks::tools::checked()\n}\n"
                .to_string(),
        ),
        (
            "app/src/run.rs",
            "use runner::declaration::task::Task;\n\npub fn ran() -> Task {\n    Task\n}\n"
                .to_string(),
        ),
        (
            "app/src/tasks/mod.rs",
            "pub mod tools;\n\nuse crate::core::task::Task;\n\npub fn listed() -> Task {\n    Task\n}\n"
                .to_string(),
        ),
        ("app/src/tasks/tools.rs", "pub fn checked() -> u8 {\n    1\n}\n".to_string()),
    ] {
        let at = root.join(file);
        fs::create_dir_all(at.parent().expect("a file sits in a folder")).expect("writable");
        fs::write(at, content).expect("writable");
    }

    let found = graphed(&root.to_string_lossy()).expect("the folder exists");
    // Every file reaching the definition of a name in a file, sorted.
    let users = |name: &str, file: &str| -> Vec<String> {
        let def = found
            .defs
            .iter()
            .position(|def| def.name == name && def.file.to_string_lossy() == file)
            .unwrap_or_else(|| panic!("{file} defines {name}"));
        let mut users: Vec<String> = found
            .edges
            .iter()
            .filter(|edge| edge.def == def)
            .map(|edge| edge.file.to_string_lossy().into_owned())
            .collect();
        users.sort();
        users.dedup();
        users
    };
    assert_eq!(
        users("Task", "runner/src/declaration/task.rs"),
        ["app/src/run.rs", "runner/src/plan.rs"],
        "crate names the file's own crate, and a crate's name names that crate"
    );
    assert_eq!(
        users("Task", "app/src/core/task.rs"),
        ["app/src/tasks/mod.rs"],
        "the same name in a file of the same name elsewhere is another definition"
    );
    assert_eq!(
        users("checked", "app/src/tasks/tools.rs"),
        ["app/src/gate.rs"],
        "a path written in full reaches the function it ends with"
    );
    assert_eq!(
        users("tools", "runner/src/test_tasks/tasks.rs"),
        Vec::<String>::new(),
        "a module named inside a path is no function of another crate"
    );
    assert_eq!(
        users("parse", "app/src/core/trees.rs"),
        ["app/src/gate.rs"],
        "a module the file imports stands for the path it was imported from"
    );
    assert_eq!(
        users("parse", "runner/src/trees.rs"),
        ["runner/src/plan.rs"],
        "each crate's import reaches its own module"
    );
    assert_eq!(
        users("gate", "app/src/gate.rs"),
        ["app/src/lib.rs"],
        "a module declared beside the file is found beside it"
    );
    assert_eq!(
        users("gate", "runner/src/gate.rs"),
        Vec::<String>::new(),
        "and never in another crate"
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn a_path_inside_a_macro_and_a_name_through_a_glob_reach_their_definitions() {
    use std::fs;
    let root = std::env::temp_dir().join(format!("arbiter-macros-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    for (file, content) in [
        (
            "app/Cargo.toml",
            "[package]\nname = \"app\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
        ),
        (
            "app/src/lib.rs",
            "pub mod deps;\npub mod globbed;\npub mod macros;\npub mod registry;\n",
        ),
        (
            "app/src/deps.rs",
            "pub fn in_vec() -> u8 {\n    0\n}\n\npub fn in_assert() -> u8 {\n    1\n}\n\n\
             pub fn in_macro() -> u8 {\n    2\n}\n\npub fn starred() -> u8 {\n    3\n}\n\n\
             pub fn untouched() -> u8 {\n    4\n}\n",
        ),
        (
            "app/src/registry.rs",
            "pub struct Task {\n    pub run: fn() -> u8,\n}\n\npub fn tasks() -> Vec<Task> {\n    \
             vec![Task { run: crate::deps::in_vec }]\n}\n\npub fn checked() {\n    \
             assert_eq!(crate::deps::in_assert(), 1);\n}\n",
        ),
        (
            "app/src/macros.rs",
            "#[macro_export]\nmacro_rules! counted {\n    () => {\n        \
             $crate::deps::in_macro()\n    };\n}\n",
        ),
        (
            "app/src/globbed.rs",
            "use crate::deps::*;\n\npub fn through() -> u8 {\n    starred()\n}\n",
        ),
    ] {
        let at = root.join(file);
        fs::create_dir_all(at.parent().expect("a file sits in a folder")).expect("writable");
        fs::write(at, content).expect("writable");
    }

    let found = graphed(&root.to_string_lossy()).expect("the folder exists");
    // Every file reaching the definition of a name in deps.rs, sorted.
    let users = |name: &str| -> Vec<String> {
        let def = found
            .defs
            .iter()
            .position(|def| def.name == name && def.file.to_string_lossy() == "app/src/deps.rs")
            .unwrap_or_else(|| panic!("deps.rs defines {name}"));
        let mut users: Vec<String> = found
            .edges
            .iter()
            .filter(|edge| edge.def == def)
            .map(|edge| edge.file.to_string_lossy().into_owned())
            .collect();
        users.sort();
        users.dedup();
        users
    };
    assert_eq!(
        users("in_vec"),
        ["app/src/registry.rs"],
        "a path among a macro's tokens is a path"
    );
    assert_eq!(users("in_assert"), ["app/src/registry.rs"]);
    assert_eq!(
        users("in_macro"),
        ["app/src/macros.rs"],
        "$crate in a macro's body is the crate defining it"
    );
    assert_eq!(
        users("starred"),
        ["app/src/globbed.rs"],
        "a glob import brings every name the file then uses"
    );
    assert_eq!(
        users("untouched"),
        Vec::<String>::new(),
        "and none the file never names"
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn an_undeclared_crate_is_named_even_where_another_crate_has_a_module_of_its_name() {
    use std::fs;
    let root = std::env::temp_dir().join(format!("arbiter-undeclared-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    let manifest = |name: &str| {
        format!("[package]\nname = \"{name}\"\nversion = \"0.1.0\"\nedition = \"2021\"\n")
    };
    for (file, content) in [
        ("app/Cargo.toml", manifest("app")),
        (
            "app/src/lib.rs",
            "pub mod shapes;\n\nmod inline {\n    pub fn helper() -> u8 {\n        0\n    }\n}\n\n\
             use inline::helper;\nuse regex::Regex;\nuse shapes::area;\n"
                .to_string(),
        ),
        (
            "app/src/shapes.rs",
            "pub fn area() -> f64 {\n    1.0\n}\n".to_string(),
        ),
        ("tools/Cargo.toml", manifest("tools")),
        ("tools/src/lib.rs", "pub mod regex;\n".to_string()),
        (
            "tools/src/regex.rs",
            "pub fn compile() -> u8 {\n    0\n}\n".to_string(),
        ),
    ] {
        let at = root.join(file);
        fs::create_dir_all(at.parent().expect("a file sits in a folder")).expect("writable");
        fs::write(at, content).expect("writable");
    }

    let found = graphed(&root.to_string_lossy()).expect("the folder exists");
    let shown: Vec<String> = laws::findings(&found)
        .iter()
        .filter(|finding| finding.rule.as_deref() == Some("undeclared-dependency"))
        .map(|finding| format!("{} {}", finding.file.to_string_lossy(), finding.reason))
        .collect();
    assert_eq!(
        shown,
        ["app/src/lib.rs regex is imported, and the unit's manifest does not declare it"],
        "a module the file declares, inline or in its own file, is no crate, and a \
         lookalike module in another crate hides nothing"
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn a_name_through_a_python_star_import_reaches_its_definition() {
    use std::fs;
    let root = std::env::temp_dir().join(format!("arbiter-star-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    for (file, content) in [
        (
            "app/pyproject.toml",
            "[project]\nname = \"app\"\nversion = \"0\"\n",
        ),
        (
            "app/src/app/deps.py",
            "def starred() -> int:\n    return 1\n\n\ndef untouched() -> int:\n    return 2\n",
        ),
        (
            "app/src/app/main.py",
            "from app.deps import *\n\nprint(starred())\n",
        ),
    ] {
        let at = root.join(file);
        fs::create_dir_all(at.parent().expect("a file sits in a folder")).expect("writable");
        fs::write(at, content).expect("writable");
    }

    let found = graphed(&root.to_string_lossy()).expect("the folder exists");
    let users = |name: &str| -> Vec<String> {
        let def = found
            .defs
            .iter()
            .position(|def| def.name == name)
            .unwrap_or_else(|| panic!("the fixture defines {name}"));
        let mut users: Vec<String> = found
            .edges
            .iter()
            .filter(|edge| edge.def == def)
            .map(|edge| edge.file.to_string_lossy().into_owned())
            .collect();
        users.dedup();
        users
    };
    assert_eq!(
        users("starred"),
        ["app/src/app/main.py"],
        "a star import brings every name the file then uses"
    );
    assert_eq!(
        users("untouched"),
        Vec::<String>::new(),
        "and none the file never names"
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn a_crate_taken_from_a_registry_is_a_release_and_only_a_path_reaches_the_repository_s_unit() {
    use std::fs;
    let root = std::env::temp_dir().join(format!("arbiter-release-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    let manifest = |name: &str, deps: &str| {
        format!("[package]\nname = \"{name}\"\nversion = \"0.1.0\"\nedition = \"2021\"\n{deps}")
    };
    for (file, content) in [
        ("lib/Cargo.toml", manifest("lib", "")),
        ("lib/src/lib.rs", "pub mod core;\n".to_string()),
        ("lib/src/core.rs", "pub struct Thing;\n".to_string()),
        (
            "released/Cargo.toml",
            manifest(
                "released",
                "\n[dependencies]\nlib = { version = \"0.1.0\", registry = \"released\" }\n",
            ),
        ),
        (
            "released/src/lib.rs",
            "use lib::core::Thing;\n\npub fn made() -> Thing {\n    Thing\n}\n".to_string(),
        ),
        (
            "local/Cargo.toml",
            manifest("local", "\n[dependencies]\nlib = { path = \"../lib\" }\n"),
        ),
        (
            "local/src/lib.rs",
            "use lib::core::Thing;\n\npub fn made() -> Thing {\n    Thing\n}\n".to_string(),
        ),
    ] {
        let at = root.join(file);
        fs::create_dir_all(at.parent().expect("a file sits in a folder")).expect("writable");
        fs::write(at, content).expect("writable");
    }

    let found = graphed(&root.to_string_lossy()).expect("the folder exists");
    let thing = found
        .defs
        .iter()
        .position(|def| def.name == "Thing")
        .expect("lib defines Thing");
    let mut users: Vec<String> = found
        .edges
        .iter()
        .filter(|edge| edge.def == thing)
        .map(|edge| edge.file.to_string_lossy().into_owned())
        .collect();
    users.sort();
    users.dedup();
    assert_eq!(
        users,
        ["local/src/lib.rs"],
        "a crate taken from a registry is compiled from its release, never from the \
         repository's source of the same name"
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn a_rust_file_marked_generated_is_read_for_what_it_uses_and_holds_no_finding() {
    use std::fs;
    let root = std::env::temp_dir().join(format!("arbiter-generated-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    for (file, content) in [
        (
            "app/Cargo.toml",
            "[package]\nname = \"app\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
        ),
        (
            "app/src/lib.rs",
            "pub mod contract;\npub mod deps;\n\n#[allow(clippy::too_many_arguments)]\npub fn hand() {}\n",
        ),
        (
            "app/src/deps.rs",
            "pub fn helped() -> u8 {\n    1\n}\n",
        ),
        ("app/src/contract/mod.rs", "include!(\"app.v1.rs\");\n"),
        (
            "app/src/contract/app.v1.rs",
            "// @generated\nuse std::collections::{BTreeMap, HashMap};\n\n#[allow(deprecated)]\n\
             pub struct Unused;\n\n#[allow(clippy::enum_variant_names)]\npub fn uses() -> u8 {\n    \
             crate::deps::helped()\n}\n",
        ),
    ] {
        let at = root.join(file);
        fs::create_dir_all(at.parent().expect("a file sits in a folder")).expect("writable");
        fs::write(at, content).expect("writable");
    }

    let found = graphed(&root.to_string_lossy()).expect("the folder exists");
    let counted: Vec<String> = arbiter::ledger::counted(&found)
        .iter()
        .map(|suppression| suppression.file.to_string_lossy().into_owned())
        .collect();
    assert_eq!(
        counted,
        ["app/src/lib.rs"],
        "a generated file's suppressions are its generator's, never counted"
    );
    let about_generated: Vec<String> = laws::findings(&found)
        .iter()
        .filter(|finding| finding.file.to_string_lossy().ends_with("app.v1.rs"))
        .map(|finding| finding.reason.clone())
        .collect();
    assert_eq!(
        about_generated,
        Vec::<String>::new(),
        "nothing a generator wrote is a finding"
    );
    let helped = found
        .defs
        .iter()
        .position(|def| def.name == "helped")
        .expect("deps.rs defines helped");
    assert!(
        found.edges.iter().any(|edge| edge.def == helped
            && edge.file.to_string_lossy() == "app/src/contract/app.v1.rs"),
        "what a generated file uses is still used"
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn a_path_from_the_root_namespace_reaches_the_crate_it_names() {
    use std::fs;
    let root = std::env::temp_dir().join(format!("arbiter-rooted-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    for (file, content) in [
        (
            "app/Cargo.toml",
            "[package]\nname = \"app\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n\
             [dependencies]\nnamed = \"1\"\nqualified = \"1\"\n",
        ),
        (
            "app/src/lib.rs",
            "use ::ghost::Haunt;\nuse ::named::Thing;\n\npub fn made() -> Thing {\n    \
             ::qualified::value();\n    Thing\n}\n",
        ),
    ] {
        let at = root.join(file);
        fs::create_dir_all(at.parent().expect("a file sits in a folder")).expect("writable");
        fs::write(at, content).expect("writable");
    }

    let found = graphed(&root.to_string_lossy()).expect("the folder exists");
    let shown: Vec<String> = laws::findings(&found)
        .iter()
        .filter(|finding| {
            finding.rule.as_deref() == Some("unused-dependency")
                || finding.rule.as_deref() == Some("undeclared-dependency")
        })
        .map(|finding| {
            format!(
                "{} {}",
                finding.rule.as_deref().unwrap_or(""),
                finding.reason
            )
        })
        .collect();
    assert_eq!(
        shown,
        ["undeclared-dependency ghost is imported, and the unit's manifest does not declare it"],
        "a leading :: names the crate after it, in a use and in a qualified path"
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn a_crate_split_within_one_major_is_named_and_two_majors_stand_side_by_side() {
    use std::fs;
    let root = std::env::temp_dir().join(format!("arbiter-majors-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    let manifest = |name: &str| {
        format!("[package]\nname = \"{name}\"\nversion = \"0.1.0\"\nedition = \"2021\"\n")
    };
    let lock = |packages: &[(&str, &str)]| {
        packages
            .iter()
            .map(|(name, version)| {
                format!("[[package]]\nname = \"{name}\"\nversion = \"{version}\"\n\n")
            })
            .collect::<String>()
    };
    for (file, content) in [
        ("one/Cargo.toml", manifest("one")),
        (
            "one/Cargo.lock",
            lock(&[("prost", "0.14.6"), ("syn", "2.0.119"), ("syn", "3.0.9")]),
        ),
        ("two/Cargo.toml", manifest("two")),
        (
            "two/Cargo.lock",
            lock(&[("prost", "0.13.5"), ("syn", "2.0.100")]),
        ),
    ] {
        let at = root.join(file);
        fs::create_dir_all(at.parent().expect("a file sits in a folder")).expect("writable");
        fs::write(at, content).expect("writable");
    }

    let found = graphed(&root.to_string_lossy()).expect("the folder exists");
    let shown: Vec<String> = laws::findings(&found)
        .iter()
        .filter(|finding| finding.rule.as_deref() == Some("split-version"))
        .map(|finding| finding.reason.clone())
        .collect();
    assert_eq!(
        shown,
        ["syn is locked at 2.0.100 in two/Cargo.lock and 2.0.119 in one/Cargo.lock"],
        "Cargo builds two majors side by side, and a 0.x minor is a major of its own"
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn a_rust_name_from_generated_code_reaches_the_contract_it_was_generated_from() {
    use std::fs;
    let root = std::env::temp_dir().join(format!("arbiter-contract-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    let manifest = |name: &str, deps: &str| {
        format!("[package]\nname = \"{name}\"\nversion = \"0.1.0\"\nedition = \"2021\"\n{deps}")
    };
    for (file, content) in [
        ("app/Cargo.toml", manifest("app", "")),
        (
            "app/proto/app/v1/thing.proto",
            "syntax = \"proto3\";\n\npackage app.v1;\n\nmessage Thing {\n  string name = 1;\n}\n"
                .to_string(),
        ),
        (
            "app/src/lib.rs",
            "pub mod contract;\npub mod user;\n".to_string(),
        ),
        (
            "app/src/contract/mod.rs",
            "include!(\"app/v1/app.v1.rs\");\n".to_string(),
        ),
        (
            "app/src/contract/app/v1/app.v1.rs",
            "// @generated\npub struct Thing {\n    pub name: String,\n}\n".to_string(),
        ),
        (
            "app/src/user.rs",
            "use crate::contract::Thing;\n\npub fn made() -> Thing {\n    Thing {\n        \
             name: String::new(),\n    }\n}\n"
                .to_string(),
        ),
        (
            "web/Cargo.toml",
            manifest("web", "\n[dependencies]\napp = { path = \"../app\" }\n"),
        ),
        (
            "web/src/lib.rs",
            "use app::contract::Thing;\n\npub fn shown(thing: Thing) -> String {\n    \
             thing.name\n}\n"
                .to_string(),
        ),
    ] {
        let at = root.join(file);
        fs::create_dir_all(at.parent().expect("a file sits in a folder")).expect("writable");
        fs::write(at, content).expect("writable");
    }

    let found = graphed(&root.to_string_lossy()).expect("the folder exists");
    let thing = found
        .defs
        .iter()
        .position(|def| def.name == "Thing" && def.file.to_string_lossy().ends_with(".proto"))
        .expect("the contract declares Thing");
    let mut users: Vec<String> = found
        .edges
        .iter()
        .filter(|edge| edge.def == thing)
        .map(|edge| edge.file.to_string_lossy().into_owned())
        .collect();
    users.sort();
    users.dedup();
    assert_eq!(
        users,
        ["app/src/user.rs", "web/src/lib.rs"],
        "a module including generated code answers for the contract's package, \
         inside its crate and from another"
    );
    assert!(
        found
            .defs
            .iter()
            .all(|def| !def.file.to_string_lossy().ends_with("app.v1.rs")),
        "the generated file itself defines nothing"
    );
    let _ = fs::remove_dir_all(&root);
}
