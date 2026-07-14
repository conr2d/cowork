use std::path::{Path, PathBuf};
use std::process::Command;

const FALLBACK_SHA_LEN: usize = 7;

fn main() {
    emit_build_sha();
    tauri_build::build();

    // With the `embed-guest` feature, stage the cross-compiled guest binary
    // (path in COWORK_GUEST_BIN) into OUT_DIR so setup.rs can include_bytes! it.
    // Off by default: dev builds and the CI host job compile without a guest.
    println!("cargo:rerun-if-env-changed=COWORK_GUEST_BIN");
    if std::env::var_os("CARGO_FEATURE_EMBED_GUEST").is_some() {
        let src = std::env::var("COWORK_GUEST_BIN").expect(
            "embed-guest feature enabled but COWORK_GUEST_BIN is not set \
             (point it at the cross-compiled musl guest binary)",
        );
        let out_dir = std::env::var("OUT_DIR").expect("OUT_DIR not set");
        let dest = std::path::Path::new(&out_dir).join("cowork-guest");
        std::fs::copy(&src, &dest)
            .unwrap_or_else(|e| panic!("copy guest binary {src} -> {}: {e}", dest.display()));
        println!("cargo:rerun-if-changed={src}");
    }
}

fn emit_build_sha() {
    println!("cargo:rerun-if-env-changed=GITHUB_SHA");

    let sha = git_checkout_sha()
        .or_else(github_sha)
        .unwrap_or_else(|| "unknown".to_string());
    println!("cargo:rustc-env=COWORK_BUILD_SHA={sha}");
}

fn git_checkout_sha() -> Option<String> {
    let cwd = std::env::current_dir().ok()?;
    let git_dir = git_dir(&cwd)?;
    emit_git_rerun_hints(&git_dir);
    git_output(&cwd, &["rev-parse", "--short", "HEAD"])
}

fn github_sha() -> Option<String> {
    std::env::var("GITHUB_SHA")
        .ok()
        .map(|sha| {
            let sha = sha.trim();
            sha.chars().take(FALLBACK_SHA_LEN).collect()
        })
        .filter(|sha: &String| !sha.is_empty())
}

fn git_dir(cwd: &Path) -> Option<PathBuf> {
    git_output(cwd, &["rev-parse", "--git-dir"]).map(|path| cwd.join(path))
}

fn emit_git_rerun_hints(git_dir: &Path) {
    println!("cargo:rerun-if-changed={}", git_dir.join("HEAD").display());
    println!(
        "cargo:rerun-if-changed={}",
        git_dir.join("packed-refs").display()
    );

    if let Ok(head) = std::fs::read_to_string(git_dir.join("HEAD")) {
        if let Some(reference) = head.strip_prefix("ref:") {
            let reference = reference.trim();
            if !reference.is_empty() {
                println!(
                    "cargo:rerun-if-changed={}",
                    git_dir.join(reference).display()
                );
            }
        }
    }
}

fn git_output(cwd: &Path, args: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(cwd)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let value = String::from_utf8(output.stdout).ok()?;
    let value = value.trim();
    if value.is_empty() {
        None
    } else {
        Some(value.to_string())
    }
}
