// Shared only by test builds. Direct cargo tests also keep generated files out of source.
pub(crate) fn root() -> std::path::PathBuf {
    let root = std::env::var_os("ACHIMELD_ARTIFACT_ROOT")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join("achimeld-tests"));
    std::fs::create_dir_all(&root).expect("Create external test output directory");
    root
}
