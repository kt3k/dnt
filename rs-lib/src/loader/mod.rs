// Copyright 2021 the Deno authors. All rights reserved. MIT license.

use std::collections::HashMap;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;

use anyhow::Result;
use deno_ast::ModuleSpecifier;
use futures::future;
use futures::Future;

use crate::utils::url_to_file_path;

#[cfg(feature = "tokio-loader")]
mod default_loader;

#[cfg(feature = "tokio-loader")]
pub use default_loader::*;

pub struct LoadResponse {
  pub maybe_headers: Option<HashMap<String, String>>,
  pub content: String,
}

pub trait Loader {
  fn read_file(
    &self,
    file_path: PathBuf,
  ) -> Pin<Box<dyn Future<Output = std::io::Result<String>> + 'static>>;
  fn make_request(
    &self,
    url: ModuleSpecifier,
  ) -> Pin<Box<dyn Future<Output = Result<LoadResponse>> + 'static>>;
}

pub struct LoaderSpecifiers {
  pub local: Vec<ModuleSpecifier>,
  pub remote: Vec<ModuleSpecifier>,
}

pub struct SourceLoader {
  loader: Arc<Box<dyn Loader>>,
  specifiers: LoaderSpecifiers,
}

impl SourceLoader {
  pub fn new(loader: Box<dyn Loader>) -> Self {
    Self {
      loader: Arc::new(loader),
      specifiers: LoaderSpecifiers {
        local: Vec::new(),
        remote: Vec::new(),
      },
    }
  }

  pub fn into_specifiers(self) -> LoaderSpecifiers {
    self.specifiers
  }
}

impl deno_graph::source::Loader for SourceLoader {
  fn load(
    &mut self,
    specifier: &ModuleSpecifier,
    // todo: handle dynamic
    _is_dynamic: bool,
  ) -> deno_graph::source::LoadFuture {
    if specifier.scheme() == "https" || specifier.scheme() == "http" {
      println!("Downloading {}...", specifier);
      self.specifiers.remote.push(specifier.clone());

      let loader = self.loader.clone();
      let specifier = specifier.clone();
      return Box::pin(async move {
        let resp = loader.make_request(specifier.clone()).await;
        (
          specifier.clone(),
          resp.map(|r| {
            Some(deno_graph::source::LoadResponse {
              specifier,
              content: Arc::new(r.content),
              maybe_headers: r.maybe_headers,
            })
          }),
        )
      });
    } else if specifier.scheme() == "file" {
      println!("Loading {}...", specifier);
      self.specifiers.local.push(specifier.clone());

      let file_path = url_to_file_path(specifier).unwrap();
      let loader = self.loader.clone();
      let specifier = specifier.clone();
      return Box::pin(async move {
        let file_text = loader.read_file(file_path).await;
        (
          specifier.clone(),
          match file_text {
            Ok(file_text) => Ok(Some(deno_graph::source::LoadResponse {
              specifier,
              content: Arc::new(file_text),
              maybe_headers: None,
            })),
            Err(err) => Err(anyhow::anyhow!("{}", err.to_string())),
          },
        )
      });
    } else {
      Box::pin(future::ready((
        specifier.clone(),
        Err(anyhow::format_err!("Unsupported scheme: {}", specifier)),
      )))
    }
  }
}
