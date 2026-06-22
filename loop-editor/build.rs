fn main() {
    let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let rubberband_root = manifest_dir.join("vendor/rubberband");
    let source = rubberband_root.join("single/RubberBandSingle.cpp");

    println!("cargo:rerun-if-changed={}", source.display());
    println!("cargo:rerun-if-changed={}", rubberband_root.display());

    cc::Build::new()
        .cpp(true)
        .define("NOMINMAX", None)
        .define("WIN32_LEAN_AND_MEAN", None)
        .file(&source)
        .include(&rubberband_root)
        .include(rubberband_root.join("src"))
        .include(rubberband_root.join("rubberband"))
        .flag_if_supported("/std:c++17")
        .flag_if_supported("/bigobj")
        .flag_if_supported("-std=c++17")
        .compile("rubberband_single");
}
