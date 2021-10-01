// Copyright 2021 the Deno authors. All rights reserved. MIT license.

use std::collections::HashMap;
use std::path::PathBuf;

use anyhow::Result;
use deno_ast::ModuleSpecifier;
use deno_graph::ModuleGraph;

use crate::utils::url_to_file_path;

pub struct Mappings {
  inner: HashMap<ModuleSpecifier, PathBuf>,
}

impl Mappings {
  pub fn new(
    module_graph: &ModuleGraph,
    local_specifiers: &[ModuleSpecifier],
    remote_specifiers: &[ModuleSpecifier],
  ) -> Result<Self> {
    let mut mappings = HashMap::new();
    let base_dir = get_base_dir(local_specifiers)?;
    for specifier in local_specifiers.iter() {
      let file_path = url_to_file_path(specifier)?;
      let relative_file_path = file_path.strip_prefix(&base_dir)?;
      mappings.insert(specifier.clone(), relative_file_path.to_path_buf());
    }

    let mut root_remote_specifiers: Vec<(
      ModuleSpecifier,
      Vec<ModuleSpecifier>,
    )> = Vec::new();
    for remote_specifier in remote_specifiers.iter() {
      let mut found = false;
      for (root_specifier, specifiers) in root_remote_specifiers.iter_mut() {
        if let Some(relative_url) =
          root_specifier.make_relative(remote_specifier)
        {
          // found a new root
          if relative_url.starts_with("../") {
            // todo: improve, this was just laziness
            let mut new_root_specifier = root_specifier.clone();
            let mut relative_url = relative_url.as_str();
            while relative_url.starts_with("../") {
              relative_url = &relative_url[3..];
              new_root_specifier = new_root_specifier.join("../").unwrap();
            }
            *root_specifier = new_root_specifier;
          }

          specifiers.push(remote_specifier.clone());
          found = true;
          break;
        }
      }
      if !found {
        root_remote_specifiers
          .push((remote_specifier.clone(), vec![remote_specifier.clone()]));
      }
    }

    for (i, (root, specifiers)) in
      root_remote_specifiers.into_iter().enumerate()
    {
      let base_dir = PathBuf::from(format!("deps/{}/", i.to_string()));
      for specifier in specifiers {
        let media_type = module_graph
          .get(&specifier)
          .ok_or_else(|| {
            anyhow::anyhow!(
              "Programming error. Could not find module for: {}",
              specifier.to_string()
            )
          })?
          .media_type;
        let relative = make_url_relative(&root, &specifier)?;
        // todo: Handle urls that are directories on the server.. I think maybe use a special
        // file name and check for collisions (of any extension)
        let mut path = base_dir.join(relative);
        path.set_extension(&media_type.as_ts_extension()[1..]);
        mappings.insert(specifier, path);
      }
    }

    Ok(Mappings { inner: mappings })
  }

  pub fn get_file_path(&self, specifier: &ModuleSpecifier) -> &PathBuf {
    self.inner.get(specifier)
      .unwrap_or_else(|| {
        panic!(
          "Programming error. Could not find file path for specifier: {}",
          specifier.to_string()
        )
      })
  }
}

fn make_url_relative(
  root: &ModuleSpecifier,
  url: &ModuleSpecifier,
) -> Result<String> {
  root.make_relative(&url).ok_or_else(|| {
    anyhow::anyhow!(
      "Error making url ({}) relative to root: {}",
      url.to_string(),
      root.to_string()
    )
  })
}

fn get_base_dir(specifiers: &[ModuleSpecifier]) -> Result<PathBuf> {
  // todo: should maybe error on windows when the files
  // span different drives...
  let mut base_dir = url_to_file_path(&specifiers[0])?
    .to_path_buf()
    .parent()
    .unwrap()
    .to_path_buf();
  for specifier in specifiers {
    let file_path = url_to_file_path(specifier)?;
    let parent_dir = file_path.parent().unwrap();
    if base_dir.starts_with(parent_dir) {
      base_dir = parent_dir.to_path_buf();
    }
  }
  Ok(base_dir)
}
