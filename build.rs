fn main() {
    // Windows icoon embedden in de executable
    #[cfg(target_os = "windows")]
    build_icon();
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
