// Copyright 2021 the Deno authors. All rights reserved. MIT license.

use std::collections::HashSet;

use deno_ast::swc::common::SyntaxContext;
use deno_ast::swc::utils::ident::IdentLike;
use deno_ast::view::*;

use crate::text_changes::TextChange;

pub struct GetDenoGlobalTextChangesParams<'a> {
  pub program: &'a Program<'a>,
  pub top_level_context: SyntaxContext,
  pub shim_package_name: &'a str,
}

struct Context<'a> {
  program: &'a Program<'a>,
  top_level_context: SyntaxContext,
  has_top_level_deno_decl: bool,
  import_shim: bool,
  text_changes: Vec<TextChange>,
}

pub fn get_deno_global_text_changes<'a>(
  params: &GetDenoGlobalTextChangesParams<'a>,
) -> Vec<TextChange> {
  let top_level_decls =
    get_top_level_declarations(params.program, params.top_level_context);
  let mut context = Context {
    program: params.program,
    top_level_context: params.top_level_context,
    has_top_level_deno_decl: top_level_decls.contains("Deno"),
    import_shim: false,
    text_changes: Vec::new(),
  };
  let program = params.program;

  // currently very crude. This should be improved to only look
  // at binding declarations
  let all_ident_names = get_all_ident_names(context.program);
  let deno_name = get_unique_name("denoShim", &all_ident_names);

  visit_children(&program.into(), &deno_name, &mut context);

  if context.import_shim {
    context.text_changes.push(TextChange {
      span: Span::new(BytePos(0), BytePos(0), Default::default()),
      new_text: format!(
        "import * as {} from \"{}\";\n",
        deno_name, params.shim_package_name,
      ),
    });
  }

  context.text_changes
}

fn visit_children(node: &Node, import_name: &str, context: &mut Context) {
  for child in node.children() {
    visit_children(&child, import_name, context);
  }

  if let Node::Ident(ident) = node {
    let id = ident.inner.to_id();
    let is_top_level_context = id.1 == context.top_level_context;
    let ident_text = ident.text_fast(context.program);
    if is_top_level_context && ident_text == "globalThis" {
      context.text_changes.push(TextChange {
        span: ident.span(),
        new_text: format!("({{ Deno: {}.Deno, ...globalThis }})", import_name),
      });
      context.import_shim = true;
    }

    // check if Deno should be imported
    if is_top_level_context
      && !context.has_top_level_deno_decl
      && ident_text == "Deno"
    {
      context.text_changes.push(TextChange {
        span: ident.span(),
        new_text: format!("{}.Deno", import_name),
      });
      context.import_shim = true;
    }
  }
}

fn get_top_level_declarations(
  program: &Program,
  top_level_context: SyntaxContext,
) -> HashSet<String> {
  use deno_ast::swc::common::collections::AHashSet;
  use deno_ast::swc::utils::collect_decls_with_ctxt;
  use deno_ast::swc::utils::Id;

  let results: AHashSet<Id> = match program {
    Program::Module(module) => {
      collect_decls_with_ctxt(module.inner, top_level_context)
    }
    Program::Script(script) => {
      collect_decls_with_ctxt(script.inner, top_level_context)
    }
  };
  results.iter().map(|v| v.0.to_string()).collect()
}

fn get_all_ident_names(program: &Program) -> HashSet<String> {
  let mut result = HashSet::new();
  visit_children(&program.into(), &mut result);
  return result;

  fn visit_children(node: &Node, result: &mut HashSet<String>) {
    for child in node.children() {
      visit_children(&child, result);
    }

    if let Node::Ident(ident) = node {
      result.insert(ident.sym().to_string());
    }
  }
}

fn get_unique_name(name: &str, all_idents: &HashSet<String>) -> String {
  let mut count = 0;
  let mut new_name = name.to_string();
  while all_idents.contains(&new_name) {
    count += 1;
    new_name = format!("{}{}", name, count);
  }
  new_name
}
