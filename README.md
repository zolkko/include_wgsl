# include_wgsl

This is an utility crate that provides two proc-macroses `include_wgsl!` and `include_wgsl_as_spirv!`.
Both of them work in the same fashion as built in `include_str!` but allow you
to combine multiple WGSL files into a single string.

```rust,ignore
static SHADER: &str = include_wgsl!("path/to/shader.wgsl")
```
and inside the shader file you can write:

```wgsl,ignore
// include "another.wgsl"

fn main() {}
```

The `include_wgsl_as_spirv!` macro does exactly the same but also uses `naga` to convert
WGSL into SPIR-V word stream.
