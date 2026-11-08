use std::fs;
use std::process::Command;

use arbiter::packs::graphed;

#[test]
fn a_git_ignored_folder_is_not_the_repository() {
    let root = std::env::temp_dir().join(format!("arbiter-scan-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(root.join("scrap")).expect("the folder is writable");
    fs::write(root.join(".gitignore"), "scrap/\n").expect("the folder is writable");
    fs::write(root.join("kept.py"), "value = 1\n").expect("the folder is writable");
    fs::write(root.join("scrap/ghost.py"), "ghost = 1\n").expect("the folder is writable");
    let path = root.to_string_lossy();

    let bare = graphed(&path).expect("the folder exists");
    assert!(
        bare.repo
            .files
            .iter()
            .any(|file| file.ends_with("scrap/ghost.py")),
        "a folder with no git ignores nothing"
    );

    let init = Command::new("git")
        .arg("-C")
        .arg(&root)
        .args(["init", "-q"])
        .status()
        .expect("git runs");
    assert!(init.success(), "git init succeeds");

    let repo = graphed(&path).expect("the folder exists");
    assert!(
        repo.repo.files.iter().any(|file| file.ends_with("kept.py")),
        "a file git would carry is judged"
    );
    assert!(
        !repo
            .repo
            .files
            .iter()
            .any(|file| file.ends_with("scrap/ghost.py")),
        "a file git ignores is not"
    );

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn a_workspace_manifest_with_no_entry_is_a_container() {
    use arbiter::core::pack::Pack;
    let pack = arbiter::packs::typescript::TypeScript::new();
    assert!(pack.container(r#"{"name":"root","workspaces":["libs/a"]}"#));
    assert!(!pack
        .container(r#"{"name":"root","workspaces":["libs/a"],"exports":{".":"./src/index.ts"}}"#));
    assert!(!pack.container(r#"{"name":"lib","main":"./src/index.ts"}"#));
}

#[test]
fn a_published_unit_keeps_its_unused_surface() {
    let root = std::env::temp_dir().join(format!("arbiter-published-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("the folder is writable");
    fs::write(root.join("ghost.py"), "ghost = 1\n").expect("the folder is writable");
    let path = root.to_string_lossy();

    let judged = graphed(&path).expect("the folder exists");
    let (flagged, _) = arbiter::judge::findings::winnow(
        &judged.defs,
        &judged.edges,
        &judged.repo,
        &Default::default(),
    );
    assert_eq!(flagged.len(), 1, "an unused symbol is a finding");

    fs::write(root.join("PUBLISHED"), "").expect("the folder is writable");
    let declared = graphed(&path).expect("the folder exists");
    let (kept, _) = arbiter::judge::findings::winnow(
        &declared.defs,
        &declared.edges,
        &declared.repo,
        &Default::default(),
    );
    assert!(kept.is_empty(), "a published surface rests unjudged");

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn fix_splits_an_import_to_one_per_line() {
    let root = std::env::temp_dir().join(format!("arbiter-mend-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("the folder is writable");
    fs::write(
        root.join("m.ts"),
        "export const a = 1;\nexport type B = number;\n",
    )
    .expect("the folder is writable");
    fs::write(
        root.join("n.ts"),
        "const d = 4;\nexport default d;\nexport const c = 3;\n",
    )
    .expect("the folder is writable");
    fs::write(
        root.join("user.ts"),
        "import { a, type B } from \"./m\";\nimport d, { c } from \"./n\";\n\nexport const sum: B = a + c + d;\n",
    )
    .expect("the folder is writable");

    let judged = graphed(&root.to_string_lossy()).expect("the folder exists");
    let pack = judged
        .packs
        .iter()
        .find(|pack| pack.label() == "typescript")
        .expect("the typescript pack exists");
    let notes = pack.mend(&judged.root, &judged.repo);
    assert_eq!(notes.len(), 1, "one file is rewritten");

    let mended = fs::read_to_string(root.join("user.ts")).expect("the file stays readable");
    assert_eq!(
        mended,
        "import { a } from \"./m\";\nimport type { B } from \"./m\";\nimport d from \"./n\";\nimport { c } from \"./n\";\n\nexport const sum: B = a + c + d;\n"
    );

    let again = graphed(&root.to_string_lossy()).expect("the folder exists");
    let quiet = again
        .packs
        .iter()
        .find(|pack| pack.label() == "typescript")
        .expect("the typescript pack exists")
        .mend(&again.root, &again.repo);
    assert!(quiet.is_empty(), "a split file stays split");

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn the_first_gate_seeds_the_decision_files_and_never_overwrites() {
    let root = std::env::temp_dir().join(format!("arbiter-seed-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("the folder is writable");

    arbiter::judge::verdict::seeded(&root);
    let units = fs::read_to_string(root.join(".arbiter/units.toml")).expect("units.toml seeded");
    assert!(units.starts_with("# The unit graph, declared."));
    assert!(
        units
            .lines()
            .all(|line| line.is_empty() || line.starts_with('#')),
        "a seed declares nothing"
    );
    let licenses =
        fs::read_to_string(root.join(".arbiter/licenses.toml")).expect("licenses.toml seeded");
    assert!(licenses
        .lines()
        .all(|line| line.is_empty() || line.starts_with('#')));

    fs::write(
        root.join(".arbiter/units.toml"),
        "[units.\"libs/x\"]\ndeps = []\n",
    )
    .expect("the folder is writable");
    arbiter::judge::verdict::seeded(&root);
    let kept = fs::read_to_string(root.join(".arbiter/units.toml")).expect("still readable");
    assert!(
        kept.starts_with("[units."),
        "a written decision is never overwritten"
    );

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn a_warm_key_follows_the_dependency_cone() {
    let root = std::env::temp_dir().join(format!("arbiter-warm-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    let demo = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/demo");
    let copied = std::process::Command::new("cp")
        .args(["-R", demo, root.to_string_lossy().as_ref()])
        .status()
        .expect("cp runs");
    assert!(copied.success());

    let keyed = |root: &std::path::Path| {
        let found = graphed(&root.to_string_lossy()).expect("the folder exists");
        let defs_units: Vec<usize> = found.defs.iter().map(|def| def.unit).collect();
        let warmth = arbiter::core::warm::warmth(
            &found.root,
            &found.repo,
            &found.packs,
            &found.edges,
            &defs_units,
        );
        let mut keys = std::collections::HashMap::new();
        for (index, unit) in found.repo.units.iter().enumerate() {
            if let Some(Some(key)) = warmth.keys.get(index) {
                keys.insert(unit.path.to_string_lossy().to_string(), key.clone());
            }
        }
        keys
    };

    let before = keyed(&root);
    fs::write(
        root.join("ui/src/button.ts"),
        "export function Button(): string {\n  return \"pressed\";\n}\n",
    )
    .expect("the folder is writable");
    let after = keyed(&root);

    assert_ne!(before["ui"], after["ui"], "the edited unit colds");
    assert_ne!(
        before["web"], after["web"],
        "a user of the edited unit colds"
    );
    assert_eq!(before["app"], after["app"], "a stranger to the edit stays");

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn the_store_answers_the_key_and_forgets_a_failure() {
    let root = std::env::temp_dir().join(format!("arbiter-store-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("the folder is writable");
    fs::create_dir_all(root.join("libs/x/src")).expect("the folder is writable");
    fs::write(
        root.join("libs/x/pyproject.toml"),
        "[project]\nname = \"x\"\n",
    )
    .expect("the folder is writable");
    fs::write(root.join("libs/x/src/x.py"), "value = 1\n").expect("the folder is writable");

    let found = graphed(&root.to_string_lossy()).expect("the folder exists");
    let defs_units: Vec<usize> = found.defs.iter().map(|def| def.unit).collect();
    let warmth = arbiter::core::warm::warmth(
        &found.root,
        &found.repo,
        &found.packs,
        &found.edges,
        &defs_units,
    );
    let index = found
        .repo
        .units
        .iter()
        .position(|unit| unit.path.to_string_lossy() == "libs/x")
        .expect("the unit exists");
    let unit = std::path::Path::new("libs/x");

    assert!(
        warmth.stored(index, unit).is_none(),
        "an empty store is cold"
    );
    warmth.record(
        index,
        unit,
        &[arbiter::core::warm::StoredCheck {
            check: "lint".to_string(),
            pack: "python".to_string(),
            seconds: 1.5,
        }],
    );
    let stored = warmth.stored(index, unit).expect("a green run is stored");
    assert_eq!(stored.len(), 1);
    assert_eq!(stored[0].check, "lint");
    warmth.invalidate(unit);
    assert!(warmth.stored(index, unit).is_none(), "a failure forgets");

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn fix_splits_a_use_to_one_per_line() {
    let root = std::env::temp_dir().join(format!("arbiter-mend-rs-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(root.join("src")).expect("the folder is writable");
    fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"mendy\"\nversion = \"0.0.1\"\n",
    )
    .expect("the folder is writable");
    fs::write(
        root.join("src/lib.rs"),
        "use std::collections::{BTreeMap, BTreeSet};\nuse std::fmt::{self, Write as _};\n\npub fn hinge() {}\n",
    )
    .expect("the folder is writable");

    let judged = graphed(&root.to_string_lossy()).expect("the folder exists");
    let pack = judged
        .packs
        .iter()
        .find(|pack| pack.label() == "rust")
        .expect("the rust pack exists");
    let notes = pack.mend(&judged.root, &judged.repo);
    assert_eq!(notes.len(), 1, "one file is rewritten");

    let mended = fs::read_to_string(root.join("src/lib.rs")).expect("the file stays readable");
    assert_eq!(
        mended,
        "use std::collections::BTreeMap;\nuse std::collections::BTreeSet;\nuse std::fmt;\nuse std::fmt::Write as _;\n\npub fn hinge() {}\n"
    );

    let again = graphed(&root.to_string_lossy()).expect("the folder exists");
    let quiet = again
        .packs
        .iter()
        .find(|pack| pack.label() == "rust")
        .expect("the rust pack exists")
        .integrity(&again.root, &again.repo);
    assert!(
        quiet.iter().all(|finding| finding.name != "crowded-import"),
        "the split leaves no crowded use"
    );

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn a_computed_python_import_is_named_and_a_literal_reaches() {
    let root = std::env::temp_dir().join(format!("arbiter-load-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(root.join("src/demo")).expect("the folder is writable");
    fs::write(
        root.join("pyproject.toml"),
        "[project]\nname = \"demo\"\nversion = \"0\"\n",
    )
    .expect("the folder is writable");
    fs::write(
        root.join("src/demo/load.py"),
        "import importlib\n\nimportlib.import_module(\"requests\")\nimportlib.import_module(\"demo.\" + \"kind\")\n",
    )
    .expect("the folder is writable");

    let judged = graphed(&root.to_string_lossy()).expect("the folder exists");
    let shown: Vec<String> = judged
        .packs
        .iter()
        .find(|pack| pack.label() == "python")
        .expect("the python pack exists")
        .integrity(&judged.root, &judged.repo)
        .iter()
        .map(|finding| finding.reason.clone())
        .collect();
    assert!(
        shown
            .iter()
            .any(|reason| reason
                == "requests is imported, and the unit's manifest does not declare it"),
        "a literal run-time import reaches the dependency law: {shown:?}"
    );
    assert!(
        shown.iter().any(|reason| reason
            == "imports a module through a computed value; a module is named by a literal"),
        "a computed run-time import is named: {shown:?}"
    );

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn a_contracts_import_outside_its_module_needs_a_declared_dependency() {
    let root = std::env::temp_dir().join(format!("arbiter-buf-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("the folder is writable");
    fs::write(
        root.join("buf.yaml"),
        "version: v2\nlint:\n  use:\n    - STANDARD\n",
    )
    .expect("the folder is writable");
    fs::write(
        root.join("ping.proto"),
        "syntax = \"proto3\";\npackage acme;\nimport \"vendor/thing.proto\";\nmessage Pinger { string id = 1; }\n",
    )
    .expect("the folder is writable");

    let judged = graphed(&root.to_string_lossy()).expect("the folder exists");
    let shown: Vec<String> = judged
        .packs
        .iter()
        .find(|pack| pack.label() == "proto")
        .expect("the proto pack exists")
        .integrity(&judged.root, &judged.repo)
        .iter()
        .map(|finding| finding.reason.clone())
        .collect();
    assert!(
        shown.iter().any(|reason| reason
            == "vendor/thing.proto is imported, and the unit's manifest does not declare it"),
        "an external import without a dep is named: {shown:?}"
    );

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn a_buf_workspace_bounds_its_modules_without_being_judged() {
    let packs = arbiter::packs::all();
    let pack = packs
        .iter()
        .find(|pack| pack.label() == "proto")
        .expect("the proto pack exists");
    assert!(pack.container("version: v2\nmodules:\n  - path: billing\n"));
    assert!(!pack.container("version: v2\nlint:\n  use:\n    - STANDARD\n"));
}

#[test]
fn a_cargo_workspace_without_a_package_is_a_container() {
    let packs = arbiter::packs::all();
    let pack = packs
        .iter()
        .find(|pack| pack.label() == "rust")
        .expect("the rust pack exists");
    assert!(pack.container("[workspace]\nmembers = [\"a\", \"b\"]\n"));
    assert!(!pack.container("[package]\nname = \"solo\"\nversion = \"0.0.1\"\n"));
    assert!(!pack.container("[workspace]\nmembers = [\"a\"]\n\n[package]\nname = \"rooted\"\n"));
}

#[test]
fn a_symbol_reached_only_by_commissioned_tests_is_nascent_not_dead() {
    let root = std::env::temp_dir().join(format!("arbiter-nascent-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(root.join("src/demo")).expect("the folder is writable");
    fs::create_dir_all(root.join("tests")).expect("the folder is writable");
    fs::create_dir_all(root.join(".arbiter")).expect("the folder is writable");
    fs::write(
        root.join("pyproject.toml"),
        "[project]\nname = \"demo\"\nversion = \"0\"\n",
    )
    .expect("the folder is writable");
    fs::write(
        root.join("src/demo/forge.py"),
        "def forged() -> int:\n    return 9\n",
    )
    .expect("the folder is writable");
    fs::write(
        root.join("tests/test_forge.py"),
        "from demo.forge import forged\n\n\ndef test_forged_turns_metal() -> None:\n    assert forged() == 9\n",
    )
    .expect("the folder is writable");
    fs::write(
        root.join(".arbiter/brief.toml"),
        "[[case]]\nname = \"forging\"\nstory = \"metal is forged and poured\"\n\n  [[case.need]]\n  need = \"forged turns metal\"\n  claims = [\". |  | forged turns metal\"]\n\n  [[case.need]]\n  need = \"a consumer pours the metal\"\n  planned = true\n",
    )
    .expect("the folder is writable");

    let judged = graphed(&root.to_string_lossy()).expect("the folder exists");
    let commissioned = arbiter::judge::brief::commissioned(&judged);
    let (findings, nascent) =
        arbiter::judge::findings::winnow(&judged.defs, &judged.edges, &judged.repo, &commissioned);
    assert!(
        findings
            .iter()
            .all(|finding| !finding.reason.contains("forged")),
        "a commissioned symbol is no finding"
    );
    assert_eq!(nascent.len(), 1, "the symbol is named nascent: {nascent:?}");
    assert!(nascent[0].contains("forged"));

    fs::write(
        root.join(".arbiter/brief.toml"),
        "[[case]]\nname = \"forging\"\nstory = \"metal is forged\"\n\n  [[case.need]]\n  need = \"forged turns metal\"\n  claims = [\". |  | forged turns metal\"]\n",
    )
    .expect("the folder is writable");
    let again = graphed(&root.to_string_lossy()).expect("the folder exists");
    let commissioned = arbiter::judge::brief::commissioned(&again);
    let (findings, nascent) =
        arbiter::judge::findings::winnow(&again.defs, &again.edges, &again.repo, &commissioned);
    assert!(
        findings
            .iter()
            .any(|finding| finding.name == "tests-only-symbol"
                && finding.reason == "forged has no user but tests"),
        "without a planned need the symbol is dead wood again"
    );
    assert!(nascent.is_empty());

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn a_class_standing_where_a_protocol_is_asked_names_it() {
    let root = std::env::temp_dir().join(format!("arbiter-doors-py-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(root.join("src/demo")).expect("the folder is writable");
    fs::write(
        root.join("pyproject.toml"),
        "[project]\nname = \"demo\"\nversion = \"0\"\n",
    )
    .expect("the folder is writable");
    fs::write(
        root.join("src/demo/core.py"),
        "from typing import Protocol\n\n\nclass Completer(Protocol):\n    def complete(self, prompt: str) -> str: ...\n\n\nclass Anthropic:\n    def complete(self, prompt: str) -> str:\n        return prompt\n\n\nclass Named(Completer):\n    def complete(self, prompt: str) -> str:\n        return prompt\n\n\ndef asked(prompt: str, completer: Completer) -> str:\n    return completer.complete(prompt)\n\n\nprint(asked(\"hi\", Anthropic()))\nprint(asked(\"hi\", Named()))\n",
    )
    .expect("the folder is writable");

    let judged = graphed(&root.to_string_lossy()).expect("the folder exists");
    let shown: Vec<String> = judged
        .packs
        .iter()
        .find(|pack| pack.label() == "python")
        .expect("the python pack exists")
        .integrity(&judged.root, &judged.repo)
        .iter()
        .filter(|finding| finding.name == "undeclared-protocol")
        .map(|finding| finding.reason.clone())
        .collect();
    assert_eq!(
        shown,
        [
            "Anthropic stands where Completer is asked and names no such base; \
          a protocol satisfied by shape is still named"
        ]
    );

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn a_class_standing_where_an_interface_is_asked_names_it() {
    let root = std::env::temp_dir().join(format!("arbiter-doors-ts-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(root.join("src")).expect("the folder is writable");
    fs::write(
        root.join("package.json"),
        "{\n  \"name\": \"demo\",\n  \"version\": \"0.0.0\"\n}\n",
    )
    .expect("the folder is writable");
    fs::write(
        root.join("src/core.ts"),
        "interface Completer {\n  complete(prompt: string): string;\n}\n\nclass Anthropic {\n  complete(prompt: string): string {\n    return prompt;\n  }\n}\n\nclass Named implements Completer {\n  complete(prompt: string): string {\n    return prompt;\n  }\n}\n\nfunction asked(prompt: string, completer: Completer): string {\n  return completer.complete(prompt);\n}\n\nasked(\"hi\", new Anthropic());\nasked(\"hi\", new Named());\n",
    )
    .expect("the folder is writable");

    let judged = graphed(&root.to_string_lossy()).expect("the folder exists");
    let shown: Vec<String> = judged
        .packs
        .iter()
        .find(|pack| pack.label() == "typescript")
        .expect("the typescript pack exists")
        .integrity(&judged.root, &judged.repo)
        .iter()
        .filter(|finding| finding.name == "undeclared-protocol")
        .map(|finding| finding.reason.clone())
        .collect();
    assert_eq!(
        shown,
        [
            "Anthropic stands where Completer is asked and names no such base; \
          a protocol satisfied by shape is still named"
        ]
    );

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn a_broken_unit_declaration_is_named_never_read_as_virgin() {
    let root = std::env::temp_dir().join(format!("arbiter-units-record-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(root.join(".arbiter")).expect("the folder is writable");
    fs::write(root.join(".arbiter/units.toml"), "[units. not toml")
        .expect("the folder is writable");
    let found = graphed(&root.to_string_lossy()).expect("the folder exists");
    let findings = arbiter::judge::findings::integrity(&found);
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].name, "unparsable-user-record");
    assert!(findings[0]
        .reason
        .contains("the declared boundaries cannot be read"));
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn a_broken_reference_is_named_never_read_as_virgin() {
    let root = std::env::temp_dir().join(format!("arbiter-record-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(root.join(".arbiter")).expect("the folder is writable");
    fs::write(root.join(".arbiter/reference.json"), "{ not json").expect("the folder is writable");
    let broken = arbiter::judge::ledger::broken_record(&root);
    assert_eq!(broken.len(), 1);
    assert_eq!(broken[0].name, "unparsable-user-record");
    assert!(broken[0].reason.contains("the signature cannot be read"));
    fs::write(root.join(".arbiter/reference.json"), "{}").expect("the folder is writable");
    assert!(arbiter::judge::ledger::broken_record(&root).is_empty());
    let _ = fs::remove_dir_all(&root);
}
