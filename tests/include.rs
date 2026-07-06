use include_wgsl::include_wgsl;

#[test]
fn resolves_nested_include() {
    const SHADER: &str = include_wgsl!("tests/fixtures/main.wgsl");

    assert!(SHADER.contains("fn add(a: f32, b: f32) -> f32"));
    assert!(SHADER.contains("fn fs_main()"));
    assert!(!SHADER.contains("#include"));
}
