//! Download and/or build official Cap-n-Proto compiler (capnp) release for the current OS and architecture

use anyhow::bail;
use proc_macro::TokenStream;
use quote::{quote, ToTokens};
use std::{
    env,
    path::{Component, Path, PathBuf},
};
use walkdir::WalkDir;
use wax::{BuildError, Glob, Pattern};

include!(concat!(env!("OUT_DIR"), "/binary_decision.rs"));

#[proc_macro]
pub fn capnp_import(input: TokenStream) -> TokenStream {
    // paths is a vector of paths, e.g.: vec!["./cap1.capnp", "./cap2.capnp"]
    // let paths_vec: Vec<&str> = todo!();
    // let _result = process(&paths_vec);
    //"fn answer() -> u32 { 42 }".parse().unwrap()
    let paths: syn::ExprArray = syn::parse(input).unwrap();
    let x: Vec<String> = paths
        .elems
        .iter()
        .map(|path| {
            path.into_token_stream()
                .to_string()
                .trim_matches('\"')
                .to_string()
        })
        .collect();
    let helperfile_contents = process_inner(&x).unwrap();
    println!("File: {:?}", helperfile_contents);
    quote! {#helperfile_contents}.into()
}

fn process_inner<T: AsRef<str>>(path_patterns: &[T]) -> anyhow::Result<String> {
    let cmdpath = CAPNP_BIN_PATH;
    let mut helperfile = String::from("// This file is autogenerated by capnp-fetch\n");

    let mut cmd = capnpc::CompilerCommand::new();
    cmd.capnp_executable(cmdpath);

    // any() wants to borrow the list of strings we give it, but we can't pass in path_patterns
    // because the borrw checker doesn't like it. We also can't pass in Vec<String> because
    // TryInto<Pattern isn't implemented for String. So, we turn the strings into owned Globs
    // (which clones the string internally)
    let globs: Result<Vec<Glob<'static>>, BuildError<'static>> = path_patterns
        .iter()
        .map(|s| {
            Glob::new(s.as_ref())
                .map_err(BuildError::into_owned)
                .map(Glob::into_owned)
        })
        .collect();
    let combined_globs = wax::any::<Glob, _>(globs?)?;

    for entry_result in WalkDir::new(".") {
        let entry = entry_result?;
        let path = normalize_path(entry.path()); // Remove the current directory indicator

        if path.is_file() && combined_globs.is_match(path.as_path()) {
            println!("Processing {:?}", path);
            helperfile += append_path(&mut cmd, &path)?.as_str();
        }
    }

    if let Err(e) = cmd.run() {
        bail!(e.to_string());
    }

    return Ok(helperfile);
}

// fn process(path_patterns: &[&str]) -> anyhow::Result<()> {
//     let target_dir = env::var("OUT_DIR").unwrap();
//     fs::write(
//         target_dir + "/capnp_include.rs",
//         process_inner(path_patterns)?,
//     )?;
//     Ok(())
// }

fn append_path(cmd: &mut capnpc::CompilerCommand, file_path: &Path) -> anyhow::Result<String> {
    let mut helperfile = String::new();

    cmd.file(file_path);

    let file_stem = file_path.file_stem().unwrap().to_str().unwrap();
    let file_extension = file_path.extension().unwrap().to_str().unwrap();
    let module_name = format!("{}_{}", file_stem, file_extension);
    let rust_module_path = file_path.with_file_name(format!("{}.rs", module_name));

    let section = format!(
        "
mod {} {{
include!(concat!(env!(\"OUT_DIR\"), \"/{}\"));
}}",
        module_name,
        rust_module_path.to_string_lossy().replace('\\', "/")
    );
    helperfile.push('\n');
    helperfile += &section;
    Ok(helperfile)
}

fn normalize_path(path: &Path) -> PathBuf {
    path.components()
        .filter(|x| match x {
            Component::Normal(_) => true,
            _ => false,
        })
        .collect()
}

#[test]
fn basic_file_test() -> anyhow::Result<()> {
    println!("{:?}", std::env::current_dir().unwrap());
    assert_eq!(
        process_inner(&["tests/example.capnp"])?,
        "// This file is autogenerated by capnp-fetch\n\n\nmod example_capnp {\ninclude!(concat!(env!(\"OUT_DIR\"), \"/tests/example_capnp.rs\"));\n}"
    );
    Ok(())
}

#[test]
fn glob_test() -> anyhow::Result<()> {
    println!("{:?}", std::env::current_dir().unwrap());

    assert_eq!(
        process_inner(&["tests/**/*.capnp"])?,
        "// This file is autogenerated by capnp-fetch\n\n\nmod example_capnp {\ninclude!(concat!(env!(\"OUT_DIR\"), \"/tests/example_capnp.rs\"));\n}\n\nmod example_capnp {\ninclude!(concat!(env!(\"OUT_DIR\"), \"/tests/folder-test/example_capnp.rs\"));\n}"
    );
    Ok(())
}
