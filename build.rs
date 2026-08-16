fn main() {
    // Kopieer de gebruikershandleiding naast de executable
    copy_manual();

    // Windows icoon embedden in de executable
    #[cfg(target_os = "windows")]
    build_icon();
}

/// Kopieer MANUAL.md naar de target-map (naast de executable), zodat de
/// gebruiker de handleiding altijd vindt naast jukeboks.exe.
fn copy_manual() {
    let manifest = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let profile = std::env::var("PROFILE").unwrap();
    let target =
        std::env::var("CARGO_TARGET_DIR").unwrap_or_else(|_| format!("{}/target", manifest));
    let src = std::path::Path::new(&manifest).join("MANUAL.md");
    let dest = std::path::Path::new(&target)
        .join(&profile)
        .join("MANUAL.md");

    println!("cargo:rerun-if-changed=MANUAL.md");

    if !src.exists() {
        return;
    }
    if let Some(parent) = dest.parent() {
        if let Err(e) = std::fs::create_dir_all(parent) {
            println!(
                "cargo:warning=Kon map '{}' niet aanmaken: {}",
                parent.display(),
                e
            );
            return;
        }
    }
    if let Err(e) = std::fs::copy(&src, &dest) {
        println!(
            "cargo:warning=Kon MANUAL.md niet kopiëren naar '{}': {}",
            dest.display(),
            e
        );
    }
}

#[cfg(target_os = "windows")]
fn build_icon() {
    let manifest = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let ico_path = std::path::Path::new(&manifest).join("assets/jukeboks.ico");

    if ico_path.exists() {
        println!("cargo:rerun-if-changed={}", ico_path.display());
        let inc = format!("/i{}", manifest.replace("/", "\\"));
        let _ = embed_resource::compile("app.rc", &[&inc as &str] as &[&str]);
    }
}
