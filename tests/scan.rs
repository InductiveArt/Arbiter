use std::fs;
use std::process::Command;

use arbiter::core::graph::graph;

#[test]
fn a_git_ignored_folder_is_not_the_repository() {
    let root = std::env::temp_dir().join(format!("arbiter-scan-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(root.join("scrap")).expect("the folder is writable");
    fs::write(root.join(".gitignore"), "scrap/\n").expect("the folder is writable");
    fs::write(root.join("kept.py"), "value = 1\n").expect("the folder is writable");
    fs::write(root.join("scrap/ghost.py"), "ghost = 1\n").expect("the folder is writable");
    let path = root.to_string_lossy();

    let bare = graph(&path).expect("the folder exists");
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

    let repo = graph(&path).expect("the folder exists");
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

    let judged = graph(&path).expect("the folder exists");
    let flagged = arbiter::judge::findings::winnow(&judged.defs, &judged.edges, &judged.repo);
    assert_eq!(flagged.len(), 1, "an unused symbol is a finding");

    fs::write(root.join("PUBLISHED"), "").expect("the folder is writable");
    let declared = graph(&path).expect("the folder exists");
    let kept = arbiter::judge::findings::winnow(&declared.defs, &declared.edges, &declared.repo);
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

    let judged = graph(&root.to_string_lossy()).expect("the folder exists");
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

    let again = graph(&root.to_string_lossy()).expect("the folder exists");
    let quiet = again
        .packs
        .iter()
        .find(|pack| pack.label() == "typescript")
        .expect("the typescript pack exists")
        .mend(&again.root, &again.repo);
    assert!(quiet.is_empty(), "a split file stays split");

    let _ = fs::remove_dir_all(&root);
}
