use include_wgsl::{include_wgsl, wgsl_to_spirv};

#[test]
fn resolves_nested_include() {
    static SHADER: [u32; 20] = wgsl_to_spirv!("");
    dbg!(&SHADER);
    assert!(false);
}
