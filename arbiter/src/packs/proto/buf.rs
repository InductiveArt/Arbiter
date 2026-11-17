use std::path::Path;

// The pinned buf every contract command runs through.
pub const BUF: [&str; 3] = ["npx", "--yes", "@bufbuild/buf@1.74.0"];

// Success keeps nothing; failure keeps the tool's whole answer.
pub fn run_buf(folder: &Path, args: &[&str]) -> Result<(), String> {
    let finished = std::process::Command::new(BUF[0])
        .args(&BUF[1..])
        .args(args)
        .current_dir(folder)
        .output();
    let Ok(output) = finished else {
        return Err(format!("{} did not start", BUF[0]));
    };
    if output.status.success() {
        return Ok(());
    }
    Err(format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
    .trim()
    .to_string())
}
