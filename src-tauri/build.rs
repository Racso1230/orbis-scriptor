use tauri_build::{Attributes, WindowsAttributes};

fn main() {
    let mut windows = WindowsAttributes::new();

    // On Windows, embed our own app manifest into every linked target (app
    // exe, cdylib and test executables) instead of letting tauri-build embed
    // its manifest into the app exe only. See windows-app.manifest for why.
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let manifest =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("windows-app.manifest");
        println!("cargo:rerun-if-changed=windows-app.manifest");
        println!("cargo:rustc-link-arg=/MANIFEST:EMBED");
        println!("cargo:rustc-link-arg=/MANIFESTINPUT:{}", manifest.display());
        windows = WindowsAttributes::new_without_app_manifest();
    }

    if let Err(error) = tauri_build::try_build(Attributes::new().windows_attributes(windows)) {
        panic!("failed to run tauri-build: {error:#}");
    }
}
