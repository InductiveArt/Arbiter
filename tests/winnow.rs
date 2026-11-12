use arbiter::laws::economy::winnow;
use arbiter::laws::judged;
use arbiter::packs::graphed;

const DEMO: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/demo");

#[test]
fn the_fixture_yields_its_known_findings() {
    let found = graphed(DEMO).expect("the fixture folder exists");
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
    let found = graphed(DEMO).expect("the fixture folder exists");
    let mut all = winnow(&found.defs, &found.edges, &found.repo, &Default::default()).0;
    all.extend(judged(&found));
    all.extend(arbiter::ledger::blanket_findings(&found));
    all.extend(arbiter::core::claims::vanished(&found));
    all.extend(arbiter::records::brief::findings(&found));
    assert!(!all.is_empty());
    for finding in &all {
        assert!(
            arbiter::laws::drillable(&finding.name),
            "{} is a finding name no rule answers",
            finding.name
        );
    }
}

#[test]
fn integrity_names_the_cycle_and_each_reexport() {
    let found = graphed(DEMO).expect("the fixture folder exists");
    let shown: Vec<String> = judged(&found)
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
            "libs/dead_py/src/dead/calc.py # type: ignore quiets more than one named rule",
            "libs/dead_py/uv.lock requests is locked at 2.31.0 in libs/dead_py/uv.lock \
             and 2.32.0 in libs/used_py/uv.lock",
            "libs/facade/src/facade/__init__.py __init__.py imports shine; \
             a symbol is imported from the module defining it",
            "libs/facade/src/facade/inner.py a comment names dashboard_frontend, \
             which nothing in the file defines, uses or quotes",
            "libs/testy/tests/test_calc.py asserts True; \
             an assertion compares the system to an expectation",
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
    let found = graphed(DEMO).expect("the fixture folder exists");
    let shown: Vec<String> = judged(&found)
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
        judged(&bare)
            .iter()
            .any(|finding| finding.name == "unused-dependency"
                && finding.reason.starts_with("coverage is declared")),
        "an uncovered package nothing imports is unused"
    );

    fs::write(
        root.join(".arbiter/suppressions.toml"),
        "[[suppression]]\nrule = \"unused-dependency\"\nlanguage = \"python\"\n\
         packages = [\"coverage\"]\nunits = [\".\"]\n",
    )
    .expect("the folder is writable");
    let quieted = graphed(&root.to_string_lossy()).expect("the folder exists");
    let findings = judged(&quieted);
    assert!(
        !findings
            .iter()
            .any(|finding| finding.name == "unused-dependency"),
        "a suppressed package is quiet"
    );
    assert!(
        !findings
            .iter()
            .any(|finding| finding.name == "stale-suppression"),
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
    let stale: Vec<String> = judged(&found)
        .iter()
        .filter(|finding| finding.name == "stale-suppression")
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
        .map(|finding| finding.name.as_str())
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
        judged(found)
            .iter()
            .filter(|finding| finding.name.contains("folder"))
            .map(|finding| (finding.name.clone(), finding.reason.clone()))
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
        !judged(&consenting)
            .iter()
            .any(|finding| finding.name.contains("folder") || finding.name.contains("unit-")),
        "the unit consents and the folder consents through the root marker"
    );

    fs::write(
        root.join(".arbiter/units.toml"),
        "[units.\"app\"]\ndeps = [\"lib\"]\n\n\
         [units.\"app\".inner.\"src/app\"]\ndeps = [\"src/other\"]\n",
    )
    .expect("writable");
    let tightened = graphed(&root.to_string_lossy()).expect("the folder exists");
    let reasons: Vec<String> = judged(&tightened)
        .iter()
        .filter(|finding| finding.name == "undeclared-folder-use")
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
        !judged(&door)
            .iter()
            .any(|finding| finding.name.contains("folder")),
        "the named file is the one door and the handler holds its key"
    );

    fs::write(
        root.join(".arbiter/units.toml"),
        "[units.\".\".inner.\"infra/db.py\"]\nvisibility = [\"api/other.py\"]\n",
    )
    .expect("the folder is writable");
    let barred = graphed(&root.to_string_lossy()).expect("the folder exists");
    let mut names: Vec<String> = judged(&barred)
        .iter()
        .filter(|finding| finding.name.contains("folder"))
        .map(|finding| finding.name.clone())
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
    let cycles: Vec<String> = judged(&found)
        .iter()
        .filter(|finding| finding.name == "sibling-cycle")
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
    let findings = judged(&signed);
    assert!(
        !findings
            .iter()
            .any(|finding| finding.name == "sibling-cycle"),
        "the exact set is quiet"
    );
    assert!(
        !findings
            .iter()
            .any(|finding| finding.name == "stale-suppression"),
        "a quieting entry is alive"
    );

    fs::write(root.join("infra/b.py"), "value = 1\n").expect("the folder is writable");
    let dissolved = graphed(&root.to_string_lossy()).expect("the folder exists");
    let stale: Vec<String> = judged(&dissolved)
        .iter()
        .filter(|finding| finding.name == "stale-suppression")
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
