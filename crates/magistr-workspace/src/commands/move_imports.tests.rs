use super::*;
use std::collections::BTreeSet;

fn set(paths: &[&str]) -> BTreeSet<String> {
    paths.iter().map(|p| (*p).to_owned()).collect()
}

#[test]
fn path_helpers_follow_the_retained_posix_dialect() {
    assert_eq!(
        relative_posix("src/feature", "src/core/helper.ts"),
        "../core/helper.ts"
    );
    assert_eq!(relative_posix("", "lib/util.js"), "lib/util.js");
    assert_eq!(join_rel("src/app", "../util/./helper"), "src/util/helper");
    assert_eq!(ext_of("a/b.Hpp"), "hpp");
    assert_eq!(ext_of("Makefile"), "");
    assert_eq!(dir_of("root.ts"), "");
}

#[test]
fn typescript_importers_and_the_moved_file_are_rewritten_by_resolution() {
    let files = set(&["src/util/helper.ts", "src/util/shared.ts", "src/app.ts"]);
    let (content, count) = super::super::move_imports_js::rewrite_importer(
        "src/app.ts",
        "import { helper } from \"./util/helper\";\nimport x from './util/other';\n",
        "src/util/helper.ts",
        "src/core/helper.ts",
        &files,
    );
    assert_eq!(count, 1);
    assert!(content.contains("from \"./core/helper\""), "{content}");
    assert!(content.contains("'./util/other'"));
    let (moved, count) = super::super::move_imports_js::rewrite_moved(
        "src/util/helper.ts",
        "src/core/helper.ts",
        "import { shared } from \"./shared\";\nconst dynamic = import(\"./shared.ts\");\n",
        &files,
    );
    assert_eq!(count, 2);
    assert!(moved.contains("from \"../util/shared\""), "{moved}");
    assert!(moved.contains("import(\"../util/shared.ts\")"), "{moved}");
}

#[test]
fn javascript_require_specifiers_keep_their_explicit_extension() {
    let files = set(&["lib/util.js", "lib/index.js", "app.js"]);
    let (content, count) = super::super::move_imports_js::rewrite_importer(
        "app.js",
        "const { util } = require(\"./lib/util.js\");\n",
        "lib/util.js",
        "lib/nested/util.js",
        &files,
    );
    assert_eq!(count, 1);
    assert!(
        content.contains("require(\"./lib/nested/util.js\")"),
        "{content}"
    );
}

#[test]
fn python_java_and_c_families_rewrite_only_resolving_specifiers() {
    let (py, count) = super::super::move_imports_python::rewrite_importer(
        "from pkg.helper import helper\nimport pkg.helper\nimport pkg.helper_extra  # pkg.helper\n",
        "pkg/helper.py",
        "pkg/core/helper.py",
    );
    assert_eq!(count, 2);
    assert!(py.contains("from pkg.core.helper import helper"), "{py}");
    assert!(py.contains("import pkg.core.helper\n"), "{py}");
    assert!(py.contains("import pkg.helper_extra  # pkg.helper"), "{py}");

    let (java, count) = super::super::move_imports_java::rewrite_importer(
        "package com.app;\n\nimport com.example.Util;\nimport static com.example.Util.max;\nimport com.example.Other;\n",
        "com/example/Util.java",
        "com/core/Util.java",
    );
    assert_eq!(count, 2);
    assert!(java.contains("import com.core.Util;"), "{java}");
    assert!(java.contains("import static com.core.Util.max;"), "{java}");
    assert!(java.contains("import com.example.Other;"), "{java}");

    let (c, count) = super::super::move_imports_c::rewrite_importer(
        "src/main.c",
        "#include \"../include/util.h\"\n#include \"include/util.h\"\n#include <stdio.h>\n",
        "include/util.h",
        "include/core/util.h",
    );
    assert_eq!(count, 2);
    assert!(c.contains("#include \"../include/core/util.h\""), "{c}");
    assert!(c.contains("#include \"include/core/util.h\""), "{c}");
    let files = set(&["lib/vec.hpp", "lib/base.hpp", "root.hpp"]);
    let (moved, count) = super::super::move_imports_c::rewrite_moved(
        "lib/vec.hpp",
        "lib/math/vec.hpp",
        "#include \"base.hpp\"\n#include \"root.hpp\"\n#include \"missing.hpp\"\n",
        &files,
    );
    assert_eq!(count, 1);
    assert!(moved.contains("#include \"../base.hpp\""), "{moved}");
    assert!(moved.contains("#include \"root.hpp\""), "{moved}");
    assert!(moved.contains("#include \"missing.hpp\""), "{moved}");
}
