// Optional: embed wintun.dll at build time.
//
// Downloads wintun from wintun.net. If the download fails (no network,
// firewall, etc.), the build continues and the runtime falls back to
// binary-adjacent dll or system PATH.

#[cfg(target_os = "windows")]
fn main() {
    use std::env;
    use std::fs;
    use std::io;
    use std::io::Read;
    use std::path::PathBuf;

    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let dll_path = out_dir.join("wintun.dll");

    if dll_path.exists() {
        println!("cargo:rerun-if-changed=build.rs");
        return;
    }

    let arch = if cfg!(target_arch = "x86_64") {
        "amd64"
    } else if cfg!(target_arch = "aarch64") {
        "arm64"
    } else {
        "x86"
    };

    let url = format!("https://www.wintun.net/builds/wintun-0.14.1.zip");

    match download_wintun(&url, arch, &dll_path) {
        Ok(()) => {
            println!("cargo:warning=wintun.dll embedded ({arch})");
            println!("cargo:rustc-cfg=wintun_embedded");
        }
        Err(e) => {
            println!("cargo:warning=wintun.dll skipped: {e}");
            println!("cargo:warning=falling back to binary-adjacent or PATH dll");
        }
    }
}

#[cfg(target_os = "windows")]
fn download_wintun(url: &str, arch: &str, dll_path: &std::path::Path) -> Result<(), String> {
    use std::io::{self, Read};
    use std::fs;

    // Download.
    let resp = ureq::get(url).call().map_err(|e| format!("download: {e}"))?;
    let mut body = Vec::new();
    resp.into_reader()
        .read_to_end(&mut body)
        .map_err(|e| format!("read: {e}"))?;

    // Unzip.
    let reader = io::Cursor::new(&body);
    let mut archive =
        zip::ZipArchive::new(reader).map_err(|e| format!("unzip: {e}"))?;

    // Try exact path: wintun/bin/{arch}/wintun.dll
    let entry_name = format!("wintun/bin/{arch}/wintun.dll");
    if let Ok(mut entry) = archive.by_name(&entry_name) {
        let mut out = fs::File::create(dll_path).map_err(|e| format!("create: {e}"))?;
        io::copy(&mut entry, &mut out).map_err(|e| format!("write: {e}"))?;
        return Ok(());
    }

    // Fallback: search all entries.
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).map_err(|e| format!("zip: {e}"))?;
        if entry.name().contains("wintun.dll") {
            let mut out = fs::File::create(dll_path).map_err(|e| format!("create: {e}"))?;
            io::copy(&mut entry, &mut out).map_err(|e| format!("write: {e}"))?;
            return Ok(());
        }
    }

    Err(format!("wintun.dll not found for {arch}"))
}

#[cfg(not(target_os = "windows"))]
fn main() {}
