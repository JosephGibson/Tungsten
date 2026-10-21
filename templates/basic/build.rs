//! Links the Windows icon, `windows/icon.res`, into the game's executables
//! for a Windows MSVC target, whose `link.exe` takes a `.res` as an input;
//! other targets get nothing. `tools/icon.py` makes the file.

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=windows/icon.res");
    let target = |key: &str| std::env::var(key).unwrap_or_default();
    if target("CARGO_CFG_TARGET_OS") == "windows" && target("CARGO_CFG_TARGET_ENV") == "msvc" {
        let manifest = std::path::PathBuf::from(target("CARGO_MANIFEST_DIR"));
        let res = manifest.join("windows").join("icon.res");
        println!("cargo:rustc-link-arg-bins={}", res.display());
    }
}
