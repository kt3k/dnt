// Copyright 2021 the Deno authors. All rights reserved. MIT license.

use std::path::PathBuf;

use anyhow::Result;
use deno_graph::create_graph;

use mappings::Mappings;
use text_changes::apply_text_changes;
use visitors::get_deno_global_text_changes;
use visitors::get_module_specifier_text_changes;
use visitors::GetDenoGlobalTextChangesParams;
use visitors::GetModuleSpecifierTextChangesParams;

pub use deno_ast::ModuleSpecifier;
pub use loader::LoadResponse;
pub use loader::Loader;

mod loader;
mod mappings;
mod parser;
mod text_changes;
mod utils;
mod visitors;

#[cfg_attr(feature = "serialization", derive(serde::Serialize))]
#[cfg_attr(feature = "serialization", serde(rename_all = "camelCase"))]
#[derive(Debug, PartialEq)]
pub struct OutputFile {
  pub file_path: PathBuf,
  pub file_text: String,
}

pub struct TransformOptions {
  pub entry_point: ModuleSpecifier,
  pub keep_extensions: bool,
  pub shim_package_name: Option<String>,
  pub loader: Option<Box<dyn Loader>>,
}

pub async fn transform(options: TransformOptions) -> Result<Vec<OutputFile>> {
  let shim_package_name = options
    .shim_package_name
    .unwrap_or_else(|| "shim-package-name".to_string());
  let mut loader =
    loader::SourceLoader::new(options.loader.unwrap_or_else(|| {
      #[cfg(feature = "tokio-loader")]
      return Box::new(loader::DefaultLoader::new());
      #[cfg(not(feature = "tokio-loader"))]
      panic!("You must provide a loader or use the 'tokio-loader' feature.")
    }));
  let source_parser = parser::CapturingSourceParser::new();
  let module_graph = create_graph(
    options.entry_point.clone(),
    &mut loader,
    None,
    None,
    Some(&source_parser),
  )
  .await;

  let local_specifiers = loader.local_specifiers();
  let remote_specifiers = loader.remote_specifiers();

  let mappings =
    Mappings::new(&module_graph, &local_specifiers, &remote_specifiers)?;

  if local_specifiers.is_empty() {
    anyhow::bail!("Did not find any local files.");
  }

  // todo: parallelize
  let mut result = Vec::new();
  for specifier in local_specifiers
    .into_iter()
    .chain(remote_specifiers.into_iter())
  {
    let parsed_source = source_parser.get_parsed_source(&specifier)?;

    let keep_extensions = options.keep_extensions;
    let text_changes = parsed_source.with_view(|program| {
      let mut text_changes = get_module_specifier_text_changes(
        &GetModuleSpecifierTextChangesParams {
          specifier: &specifier,
          module_graph: &module_graph,
          mappings: &mappings,
          use_js_extension: keep_extensions,
          program: &program,
        },
      );
      text_changes.extend(get_deno_global_text_changes(
        &GetDenoGlobalTextChangesParams {
          program: &program,
          top_level_context: parsed_source.top_level_context(),
          shim_package_name: shim_package_name.as_str(),
        },
      ));
      text_changes
    });

    let final_file_text = apply_text_changes(
      parsed_source.source().text().to_string(),
      text_changes,
    );

    result.push(OutputFile {
      file_path: mappings.get_file_path(&specifier).to_owned(),
      file_text: final_file_text,
    });
  }

  Ok(result)
}
