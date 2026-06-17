use std::process::Command;

fn main() {
    // Embed build timestamp using the `time` crate for correct formatting.
    let now = time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_else(|_| "unknown".to_owned());
    println!("cargo:rustc-env=ZERO_BUILD_TIME={now}");

    // Embed git commit hash if available.
    if let Ok(output) = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
    {
        if output.status.success() {
            let hash = String::from_utf8_lossy(&output.stdout).trim().to_owned();
            println!("cargo:rustc-env=ZERO_GIT_HASH={hash}");
        }
    }

    // Embed git tag if available.
    if let Ok(output) = Command::new("git")
        .args(["describe", "--tags", "--always"])
        .output()
    {
        if output.status.success() {
            let tag = String::from_utf8_lossy(&output.stdout).trim().to_owned();
            println!("cargo:rustc-env=ZERO_GIT_DESCRIBE={tag}");
        }
    }
}
