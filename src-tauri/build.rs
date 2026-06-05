fn main() {
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
