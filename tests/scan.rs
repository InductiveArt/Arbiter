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
