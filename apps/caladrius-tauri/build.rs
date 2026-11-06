//! Builds the shell. The app manifest is present, so Tauri refuses every command of the app that a
//! capability does not allow: `capabilities/default.json` lists the permission `allow-engine-execute`
//! (`permissions/engine.toml`) and nothing else.

fn main() {
    // The test executables need the common controls v6 to load the webview library on Windows (the
    // executable of the shell gets its manifest from tauri-build).
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_default();
        println!("cargo:rustc-link-arg-tests=/MANIFEST:EMBED");
        println!("cargo:rustc-link-arg-tests=/MANIFESTINPUT:{dir}/test-manifest.xml");
        println!("cargo:rerun-if-changed=test-manifest.xml");
    }
    let attributes = tauri_build::Attributes::new().app_manifest(tauri_build::AppManifest::new());
    if let Err(error) = tauri_build::try_build(attributes) {
        // A build script reports its failure on the standard error and exits with a code.
        eprintln!("tauri-build failed: {error:#}");
        std::process::exit(1);
    }
}
