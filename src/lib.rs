use std::collections::HashSet;
use std::path::{Path, PathBuf};

use nom::IResult;
use nom::bytes::complete::{tag, take_until};
use nom::character::complete::{char, space0, space1};
use proc_macro::TokenStream;
use quote::quote;
use syn::{LitStr, parse_macro_input};

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
    let mut touched = Vec::new();

    match resolve_includes(&root_path, &mut stack, &mut seen, &mut touched) {
        Ok(source) => {
            let tracked = touched.iter().map(|path| {
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
        Err(message) => syn::Error::new(lit.span(), message)
            .to_compile_error()
            .into(),
    }
}

fn resolve_includes(
    path: &Path,
    stack: &mut Vec<PathBuf>,
    seen: &mut HashSet<PathBuf>,
    touched: &mut Vec<PathBuf>,
) -> Result<String, String> {
    let canonical = path
        .canonicalize()
        .map_err(|err| format!("failed to read `{}`: {err}", path.display()))?;

    if stack.contains(&canonical) {
        return Err(format!(
            "circular #include detected for `{}`",
            canonical.display()
        ));
    }

    if seen.insert(canonical.clone()) {
        touched.push(canonical.clone());
    }

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
                let nested = resolve_includes(&nested_path, stack, seen, touched)?;
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
