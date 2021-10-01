use std::path::PathBuf;

#[macro_use]
mod integration;

use integration::TestBuilder;

#[tokio::test]
async fn transform_standalone_file() {
  let result = TestBuilder::new()
    .with_loader(|loader| {
      loader.add_local_file("/mod.ts", r#"test;"#);
    })
    .transform().await.unwrap();

  assert_files!(result, &[
    ("mod.ts", "test;")
  ]);
}

#[tokio::test]
async fn transform_deno_shim() {
  let result = TestBuilder::new()
    .with_loader(|loader| {
      loader.add_local_file("/mod.ts", r#"Deno.readTextFile();"#);
    })
    .transform().await.unwrap();

  assert_files!(result, &[
    ("mod.ts", concat!(
      r#"import * as denoShim from "shim-package-name";"#,
      "\ndenoShim.Deno.readTextFile();"
    ))
  ]);
}

#[tokio::test]
async fn transform_deno_shim_with_name_collision() {
  let result = TestBuilder::new()
    .with_loader(|loader| {
      loader.add_local_file("/mod.ts", r#"Deno.readTextFile(); const denoShim = {};"#);
    })
    .shim_package_name("test-shim")
    .transform().await.unwrap();

  assert_files!(result, &[
    ("mod.ts", concat!(
      r#"import * as denoShim1 from "test-shim";"#,
      "\ndenoShim1.Deno.readTextFile(); const denoShim = {};"
    ))
  ]);
}

#[tokio::test]
async fn transform_global_this_deno() {
  let result = TestBuilder::new()
    .with_loader(|loader| {
      loader.add_local_file("/mod.ts", r#"globalThis.Deno.readTextFile();"#);
    })
    .shim_package_name("test-shim")
    .transform().await.unwrap();

  assert_files!(result, &[
    ("mod.ts", concat!(
      r#"import * as denoShim from "test-shim";"#,
      "\n({ Deno: denoShim.Deno, ...globalThis }).Deno.readTextFile();"
    ))
  ]);
}

#[tokio::test]
async fn transform_deno_collision() {
  let result = TestBuilder::new()
    .with_loader(|loader| {
      loader.add_local_file("/mod.ts", concat!(
        "const Deno = {};",
        "const { Deno: Deno2 } = globalThis;",
        "Deno2.readTextFile();",
        "Deno.test;"
      ));
    })
    .shim_package_name("test-shim")
    .transform().await.unwrap();

  assert_files!(result, &[
    ("mod.ts", concat!(
      r#"import * as denoShim from "test-shim";"#,
      "\nconst Deno = {};",
      "const { Deno: Deno2 } = ({ Deno: denoShim.Deno, ...globalThis });",
      "Deno2.readTextFile();",
      "Deno.test;"
    ))
  ]);
}

#[tokio::test]
async fn transform_other_file_no_extensions() {
  let result = TestBuilder::new()
    .with_loader(|loader| {
      loader.add_local_file("/mod.ts",
        "import * as other from './other.ts';"
      )
      .add_local_file("/other.ts",
        "5;"
      );
    })
    .transform().await.unwrap();

  assert_files!(result, &[
    ("mod.ts", "import * as other from './other';"),
    ("other.ts", "5;")
  ]);
}

#[tokio::test]
async fn transform_other_file_keep_extensions() {
  let result = TestBuilder::new()
    .with_loader(|loader| {
      loader.add_local_file("/mod.ts",
        "import * as other from './other.ts';"
      )
      .add_local_file("/other.ts",
        "5;"
      );
    })
    .keep_extensions()
    .transform().await.unwrap();

  assert_files!(result, &[
    ("mod.ts", "import * as other from './other.js';"),
    ("other.ts", "5;")
  ]);
}

#[tokio::test]
async fn transform_remote_files() {
  let result = TestBuilder::new()
    .with_loader(|loader| {
      loader.add_local_file("/mod.ts",
        "import * as other from 'http://localhost/mod.ts';"
      )
      .add_remote_file("http://localhost/mod.ts", "import * as myOther from './other.ts';")
      .add_remote_file("http://localhost/other.ts", "import * as folder from './folder';")
      .add_remote_file("http://localhost/folder", "import * as folder2 from './folder.ts';")
      .add_remote_file("http://localhost/folder.ts", "import * as folder3 from './folder.js';")
      .add_remote_file("http://localhost/folder.js", "import * as otherFolder from './otherFolder';")
      .add_remote_file("http://localhost/otherFolder", "import * as subFolder from './sub/subfolder';")
      .add_remote_file("http://localhost/sub/subfolder", "import * as localhost2 from 'http://localhost2';")
      .add_remote_file("http://localhost2", "import * as localhost3Mod from 'https://localhost3/mod.ts';")
      .add_remote_file("https://localhost3/mod.ts", "import * as localhost3 from 'https://localhost3';")
      .add_remote_file("https://localhost3", "5;");
    })
    .transform().await.unwrap();

  assert_files!(result, &[
    ("mod.ts", "import * as other from './deps/0/mod';"),
    ("deps/0/mod.ts", "import * as myOther from './other';"),
    ("deps/0/other.ts", "import * as folder from './folder';"),
    ("deps/0/folder.js", "import * as folder2 from './folder_2';"),
    ("deps/0/folder_2.ts", "import * as folder3 from './folder_3';"),
    ("deps/0/folder_3.js", "import * as otherFolder from './otherFolder';"),
    ("deps/0/otherFolder.js", "import * as subFolder from './sub/subfolder';"),
    ("deps/0/sub/subfolder.js", "import * as localhost2 from '../../1';"),
    ("deps/1.js", "import * as localhost3Mod from './2/mod';"),
    ("deps/2/mod.ts", "import * as localhost3 from '../2';"),
    ("deps/2.js", "5;"),
  ]);
}
