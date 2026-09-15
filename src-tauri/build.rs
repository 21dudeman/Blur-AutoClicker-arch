fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    if cfg!(target_os = "linux") {
        let has_x11 = pkg_config::probe_library("x11").is_ok();
        if !has_x11 {
            panic!("libX11 development files not found. Install x11 headers (e.g. pacman -S libx11)");
        }
        let _ = pkg_config::probe_library("xtst");
        let _ = pkg_config::probe_library("xinerama");
        println!("cargo:rustc-link-lib=X11");
        println!("cargo:rustc-link-lib=Xtst");
        println!("cargo:rustc-link-lib=Xinerama");
    }
    println!("cargo:rerun-if-env-changed=CRASHPAD_HANDLER_PATH");
    if std::env::var_os("CARGO_FEATURE_CRASHPAD").is_some() {
        use std::path::PathBuf;
        let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
        let committed_path = manifest.join("crashpad_handler.sha256");
        let committed_hash = std::fs::read_to_string(&committed_path)
            .ok()
            .and_then(|s| {
                for line in s.lines() {
                    let t = line.trim();
                    if t.is_empty() || t.starts_with('#') {
                        continue;
                    }
                    // first 64 hex chars on line
                    let hex: String = t.chars().filter(|c| c.is_ascii_hexdigit()).collect();
                    if hex.len() >= 64 {
                        return Some(hex[..64].to_lowercase());
                    }
                    if !t.is_empty() {
                        return Some(t.to_lowercase());
                    }
                }
                None
            })
            .filter(|s| !s.is_empty());
        println!("cargo:rerun-if-changed={}", committed_path.display());
        let profile = std::env::var("PROFILE").unwrap_or_else(|_| "release".into());
        let root = std::env::var("CARGO_TARGET_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("target"));
        let triple = std::env::var("TARGET").unwrap_or_default();
        let mut candidates = vec![
            root.join(&profile).join("crashpad_handler.exe"),
            root.join(&triple)
                .join(&profile)
                .join("crashpad_handler.exe"),
        ];
        if let Ok(out_dir) = std::env::var("OUT_DIR").map(PathBuf::from) {
            candidates.push(out_dir.join("crashpad_handler.exe"));
        }
        if let Ok(env_path) = std::env::var("CRASHPAD_HANDLER_PATH") {
            if !env_path.is_empty() {
                candidates.insert(0, PathBuf::from(env_path));
            }
        }
        let mut found = false;
        for handler in &candidates {
            if let Ok(bytes) = std::fs::read(handler) {
                use sha2::{Digest, Sha256};
                let digest = hex::encode(Sha256::digest(&bytes));
                if let Some(expected) = &committed_hash {
                    if digest.to_lowercase() != *expected {
                        panic!(
                            "crashpad_handler.exe hash mismatch: expected {}, got {} — possible upstream tamper or stale committed hash. Update crashpad_handler.sha256 if this is a legitimate upstream update.",
                            expected, digest
                        );
                    }
                }
                println!("cargo:rustc-env=CRASHPAD_HANDLER_SHA256={}", digest);
                println!("cargo:rerun-if-changed={}", handler.display());
                found = true;
                break;
            }
        }
        if !found {
            if committed_hash.is_some() {
                println!(
                    "cargo:warning=crashpad_handler.exe not found but crashpad_handler.sha256 is committed — verification will fail at runtime until handler is built"
                );
            } else {
                println!(
                    "cargo:warning=crashpad_handler.exe not found for hash pin, verification will be no-op in this build (add crashpad_handler.sha256 to enforce pin)"
                );
            }
        }
    }
    tauri_build::build()
}
