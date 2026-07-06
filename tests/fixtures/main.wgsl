// include "common.wgsl"

@fragment
fn fs_main() -> @location(0) vec4<f32> {
    let x = add(1.0, 2.0);
    return vec4<f32>(x, x, x, 1.0);
}
