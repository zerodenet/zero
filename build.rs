use std::process::Command;

fn main() {
    // Embed build timestamp from current time.
    let now = chrono_or_manual();
    println!("cargo:rustc-env=ZERO_BUILD_TIME={now}");

    // Embed git commit hash if available.
    if let Ok(output) = Command::new("git").args(["rev-parse", "--short", "HEAD"]).output() {
        if output.status.success() {
            let hash = String::from_utf8_lossy(&output.stdout).trim().to_owned();
            println!("cargo:rustc-env=ZERO_GIT_HASH={hash}");
        }
    }

    // Embed git tag if available.
    if let Ok(output) = Command::new("git").args(["describe", "--tags", "--always"]).output() {
        if output.status.success() {
            let tag = String::from_utf8_lossy(&output.stdout).trim().to_owned();
            println!("cargo:rustc-env=ZERO_GIT_DESCRIBE={tag}");
        }
    }
}

fn chrono_or_manual() -> String {
    // Try chrono for proper formatting.
    // Fallback: just use the build timestamp from git or current time.
    if let Ok(now) = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
        let secs = now.as_secs();
        // Simple ISO-like format
        let days_since_epoch = secs / 86400;
        let time_of_day = secs % 86400;
        let hours = time_of_day / 3600;
        let minutes = (time_of_day % 3600) / 60;
        let seconds = time_of_day % 60;
        // Calculate date from epoch (approximate, good enough for build timestamp)
        let (y, m, d) = epoch_to_date(days_since_epoch as i64);
        return format!("{y:04}-{m:02}-{d:02}T{hours:02}:{minutes:02}:{seconds:02}Z");
    }
    "unknown".to_owned()
}

fn epoch_to_date(days: i64) -> (i64, u32, u32) {
    // Algorithm from http://howardhinnant.github.io/date_algorithms.html
    let z = days + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = (z - era * 146097) as u32;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
}
