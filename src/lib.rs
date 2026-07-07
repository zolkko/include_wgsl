use std::collections::HashSet;
use std::error::Error;
use std::path::{Path, PathBuf};

use nom::IResult;
use nom::bytes::complete::{tag, take_until};
use nom::character::complete::{char, space0, space1};
use proc_macro::TokenStream;
use quote::quote;
use syn::{LitStr, parse_macro_input};

/// Includes a WGSL shader and precompiles it to SPIR-V shader language.
///
/// ```ignore
/// static SPIR_V_SHADER: &[u32] = include_wgsl_as_spirv!("shaders/main.wgsl");
/// ```
#[proc_macro]
pub fn include_wgsl_as_spirv(input: TokenStream) -> TokenStream {
    let lit = parse_macro_input!(input as LitStr);

    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".to_string());
    let root_path = Path::new(&manifest_dir).join(lit.value());

    let mut stack = Vec::new();
    let mut seen = HashSet::new();

    into_token_stream(
        resolve_includes(&root_path, &mut stack, &mut seen)
            .and_then(|source| convert_to_spirv(&source).map(VecAsArray)),
        lit.span(),
        seen,
    )
}

fn convert_to_spirv(value: &str) -> Result<Vec<u32>, Box<dyn Error>> {
    let module: naga::Module = naga::front::wgsl::parse_str(value)?;
    let module_info = naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::all(),
    )
    .subgroup_stages(naga::valid::ShaderStages::all())
    .subgroup_operations(naga::valid::SubgroupOperationSet::all())
    .validate(&module)?;

    let spv_source = naga::back::spv::write_vec(
        &module,
        &module_info,
        &naga::back::spv::Options::default(),
        None,
    )?;

    Ok(spv_source)
}

struct VecAsArray<T>(Vec<T>);

impl<T> quote::ToTokens for VecAsArray<T>
where
    T: quote::ToTokens,
{
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let elements = &self.0;
        tokens.extend(quote! { [#(#elements),*] });
    }
}

/// Includes a WGSL file as a `&'static str`, resolving `// include "path"`
/// directives recursively, similarly to `include_str!`.
///
/// The given path is resolved relative to the crate root (`CARGO_MANIFEST_DIR`).
/// Paths inside `include` directives are resolved relative to the file that
/// contains them.
///
/// ```ignore
/// const SHADER: &str = include_wgsl!("shaders/main.wgsl");
/// ```
#[proc_macro]
pub fn include_wgsl(input: TokenStream) -> TokenStream {
    let lit = parse_macro_input!(input as LitStr);

    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".to_string());
    let root_path = Path::new(&manifest_dir).join(lit.value());

    let mut stack = Vec::new();
    let mut seen = HashSet::new();
    into_token_stream(
        resolve_includes(&root_path, &mut stack, &mut seen),
        lit.span(),
        seen,
    )
}

fn resolve_includes(
    path: &Path,
    stack: &mut Vec<PathBuf>,
    seen: &mut HashSet<PathBuf>,
) -> Result<String, Box<dyn Error>> {
    let canonical = path
        .canonicalize()
        .map_err(|err| format!("failed to read `{}`: {err}", path.display()))?;

    if stack.contains(&canonical) {
        return Err(format!("circular include detected for `{}`", canonical.display()).into());
    }

    seen.insert(canonical.clone());

    let content = std::fs::read_to_string(&canonical)
        .map_err(|err| format!("failed to read `{}`: {err}", canonical.display()))?;

    let dir = canonical.parent().unwrap_or_else(|| Path::new("."));

    stack.push(canonical.clone());
    let mut output = String::with_capacity(content.len());
    for line in content.lines() {
        let line = line.trim_end_matches('\r');
        match parse_include_directive(line) {
            Some(include_path) => {
                let nested_path = dir.join(include_path);
                let nested = resolve_includes(&nested_path, stack, seen)?;
                output.push_str(&nested);
                if !nested.ends_with('\n') {
                    output.push('\n');
                }
            }
            None => {
                output.push_str(line);
                output.push('\n');
            }
        }
    }
    stack.pop();

    Ok(output)
}

fn parse_include_directive(line: &str) -> Option<&str> {
    fn directive(input: &str) -> IResult<&str, &str> {
        let (input, _) = space0(input)?;
        let (input, _) = tag("//")(input)?;
        let (input, _) = space0(input)?;
        let (input, _) = tag("include")(input)?;
        let (input, _) = space1(input)?;
        let (input, _) = char('"')(input)?;
        let (input, path) = take_until("\"")(input)?;
        let (input, _) = char('"')(input)?;
        Ok((input, path))
    }

    match directive(line) {
        Ok((rest, path)) if rest.trim().is_empty() => Some(path),
        _ => None,
    }
}

fn into_token_stream<T>(
    input: Result<T, Box<dyn Error>>,
    input_span: proc_macro2::Span,
    touched: impl IntoIterator<Item = PathBuf>,
) -> TokenStream
where
    T: quote::ToTokens,
{
    match input {
        Ok(source) => {
            let tracked = touched.into_iter().map(|path| {
                let path = path.to_string_lossy().into_owned();
                quote! { const _: &str = include_str!(#path); }
            });
            quote! {
                {
                    #(#tracked)*
                    #source
                }
            }
            .into()
        }
        Err(message) => syn::Error::new(input_span, message)
            .to_compile_error()
            .into(),
    }
}
