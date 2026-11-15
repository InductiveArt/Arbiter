use std::path::Path;
use std::path::PathBuf;

use serde_json::Value;

use crate::wip::trace::outcome::Failure;
use crate::wip_domain::diagnostic::Diagnostic;

// Each error or warning cargo printed that points at a line, told once: a
// lint met in the library and again in its tests is printed twice. A file is
// as cargo writes it, from the root of the workspace it ran in.
pub fn read_cargo_json(printed: &str) -> Vec<Diagnostic> {
    let mut diagnostics: Vec<Diagnostic> = printed
        .lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .filter(|line| line["reason"] == "compiler-message")
        .filter_map(|line| diagnostic(&line["message"]))
        .collect();
    diagnostics.sort();
    diagnostics.dedup();
    diagnostics
}

// The folder holding the manifest cargo names as the root of its workspace,
// from what it prints when asked where that root is.
pub fn workspace_root(located: &str) -> Result<PathBuf, Failure> {
    serde_json::from_str::<Value>(located.trim())
        .ok()
        .and_then(|told| told["root"].as_str().map(PathBuf::from))
        .filter(|manifest| manifest.is_absolute())
        .and_then(|manifest| manifest.parent().map(Path::to_path_buf))
        .ok_or_else(|| Failure::from(format!("cargo named no workspace root: {}", located.trim())))
}

fn diagnostic(message: &Value) -> Option<Diagnostic> {
    let level = message["level"].as_str()?;
    if level != "error" && level != "warning" {
        return None;
    }
    let span = message["spans"]
        .as_array()?
        .iter()
        .find(|span| span["is_primary"] == true)?;
    Some(Diagnostic {
        file: PathBuf::from(span["file_name"].as_str()?),
        line: span["line_start"]
            .as_u64()
            .and_then(|line| u32::try_from(line).ok()),
        code: message["code"]["code"].as_str().map(str::to_string),
        message: message["message"].as_str()?.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::read_cargo_json;
    use super::workspace_root;
    use crate::wip_domain::diagnostic::Diagnostic;

    const LINT: &str = r#"{"reason":"compiler-message","package_id":"path+file:///w#probe@0.1.0","manifest_path":"/w/Cargo.toml","target":{"kind":["lib"],"name":"probe","src_path":"/w/src/lib.rs"},"message":{"rendered":"error: equal expressions as operands to `==`\n","children":[],"code":{"code":"clippy::eq_op","explanation":null},"level":"error","message":"equal expressions as operands to `==`","spans":[{"byte_end":44,"byte_start":38,"column_end":11,"column_start":5,"expansion":null,"file_name":"src/lib.rs","is_primary":true,"label":null,"line_end":2,"line_start":2,"suggested_replacement":null,"suggestion_applicability":null,"text":[]}]}}"#;
    const SUMMARY: &str = r#"{"reason":"compiler-message","package_id":"path+file:///w#probe@0.1.0","manifest_path":"/w/Cargo.toml","target":{"kind":["lib"],"name":"probe","src_path":"/w/src/lib.rs"},"message":{"rendered":"error: aborting due to 1 previous error\n","children":[],"code":null,"level":"error","message":"aborting due to 1 previous error","spans":[]}}"#;
    const ARTIFACT: &str =
        r#"{"reason":"compiler-artifact","package_id":"path+file:///w#probe@0.1.0","fresh":true}"#;
    const FINISHED: &str = r#"{"reason":"build-finished","success":false}"#;

    fn lint(file: &str, line: u32) -> Diagnostic {
        Diagnostic {
            file: PathBuf::from(file),
            line: Some(line),
            code: Some("clippy::eq_op".to_string()),
            message: "equal expressions as operands to `==`".to_string(),
        }
    }

    #[test]
    fn each_message_pointing_at_a_line_is_one_diagnostic_its_file_as_cargo_writes_it() {
        let printed = [ARTIFACT, LINT, SUMMARY, FINISHED].join("\n");
        assert_eq!(read_cargo_json(&printed), [lint("src/lib.rs", 2)]);
    }

    #[test]
    fn a_warning_is_a_diagnostic_as_an_error_is() {
        let warned = LINT.replace(r#""level":"error""#, r#""level":"warning""#);
        assert_eq!(read_cargo_json(&warned), [lint("src/lib.rs", 2)]);
    }

    #[test]
    fn a_message_printed_for_the_library_and_again_for_its_tests_is_one_diagnostic() {
        let printed = [LINT, LINT].join("\n");
        assert_eq!(read_cargo_json(&printed), [lint("src/lib.rs", 2)]);
    }

    #[test]
    fn a_diagnostic_is_placed_where_the_primary_span_points() {
        let spanned = LINT.replace(
            r#""spans":[{"#,
            r#""spans":[{"file_name":"src/other.rs","is_primary":false,"line_start":9},{"#,
        );
        assert_eq!(read_cargo_json(&spanned), [lint("src/lib.rs", 2)]);
    }

    #[test]
    fn a_line_that_is_not_json_and_a_message_with_no_line_are_no_diagnostic() {
        let printed = ["   Compiling probe v0.1.0", LINT, SUMMARY, ""].join("\n");
        assert_eq!(read_cargo_json(&printed), [lint("src/lib.rs", 2)]);
    }

    #[test]
    fn the_workspace_root_is_the_folder_of_the_manifest_cargo_names() {
        assert_eq!(
            workspace_root("{\"root\":\"/w/Cargo.toml\"}\n").expect("cargo named it"),
            PathBuf::from("/w")
        );
    }

    #[test]
    fn a_root_not_named_or_not_absolute_is_refused() {
        for told in [
            "",
            "error: could not find `Cargo.toml`",
            "{\"root\":\"Cargo.toml\"}",
        ] {
            let refused = workspace_root(told).expect_err("no root is named");
            assert_eq!(
                refused.reason,
                format!("cargo named no workspace root: {told}")
            );
        }
    }
}
