use super::super::test_support::Fixture;
use std::fs;

#[path = "source_parser_pin_io_tests.rs"]
mod source_parser_pin_io_tests;

const INPUT_PATHS: &[&str] = &["Cargo.toml", "Cargo.lock", "crates/kdr-linter/Cargo.toml"];
const WORKSPACE: &str = "[workspace]\n[workspace.dependencies]\nsyn = { version = \"3\", features = [\"full\", \"visit\"] }\n";
const LINTER: &str = "[dependencies]\nsyn = { workspace = true }\n";
const LOCK: &str = "[[package]]\nname = \"syn\"\nversion = \"3.0.9\"\n";

#[test]
fn parsed_block_rejects_missing_function_and_invalid_rust() {
    assert!(super::parsed_block("fn present() {}", "missing").is_err());
    assert!(super::parsed_block("fn broken( {}", "broken").is_err());
}

struct ParserPinCase {
    workspace: &'static str,
    linter: &'static str,
    lock: &'static str,
    expected: &'static str,
}

#[derive(Clone, Copy)]
enum AstLookupKind {
    Function,
    Method,
    TraitMethod,
}

struct AstLookupRejection {
    source: &'static str,
    kind: AstLookupKind,
    trait_name: &'static str,
    owner: &'static str,
    name: &'static str,
    expected: &'static str,
}

const AST_LOOKUP_REJECTIONS: &[AstLookupRejection] = &[
    AstLookupRejection {
        source: "fn run() {} fn run() {}",
        kind: AstLookupKind::Function,
        trait_name: "",
        owner: "",
        name: "run",
        expected: "ambiguous-function:run",
    },
    AstLookupRejection {
        source: "struct Owner; impl Owner {}",
        kind: AstLookupKind::Method,
        trait_name: "",
        owner: "Owner",
        name: "run",
        expected: "missing-method:Owner::run",
    },
    AstLookupRejection {
        source: "struct Owner; struct Other; impl Other { fn run(&self) {} }",
        kind: AstLookupKind::Method,
        trait_name: "",
        owner: "Owner",
        name: "run",
        expected: "missing-method:Owner::run",
    },
    AstLookupRejection {
        source: "struct Owner; impl Owner { fn run(&self) {} } impl Owner { fn run(&self) {} }",
        kind: AstLookupKind::Method,
        trait_name: "",
        owner: "Owner",
        name: "run",
        expected: "ambiguous-method:Owner::run",
    },
    AstLookupRejection {
        source: "struct Owner; trait Action {} impl Action for Owner { fn run(&self) {} } impl Action for Owner { fn run(&self) {} }",
        kind: AstLookupKind::TraitMethod,
        trait_name: "Action",
        owner: "Owner",
        name: "run",
        expected: "ambiguous-trait-method:Action for Owner::run",
    },
    AstLookupRejection {
        source: "struct Owner; trait Action {} trait Other { fn run(&self); } impl Other for Owner { fn run(&self) {} }",
        kind: AstLookupKind::TraitMethod,
        trait_name: "Action",
        owner: "Owner",
        name: "run",
        expected: "missing-trait-method:Action for Owner::run",
    },
    AstLookupRejection {
        source: "trait Action {} impl Action for (u8, u8) { fn run(&self) {} }",
        kind: AstLookupKind::TraitMethod,
        trait_name: "Action",
        owner: "Owner",
        name: "run",
        expected: "missing-trait-method:Action for Owner::run",
    },
];

const REJECTIONS: &[ParserPinCase] = &[
    ParserPinCase {
        workspace: "[workspace",
        linter: LINTER,
        lock: LOCK,
        expected: "parser-pin:workspace-manifest:",
    },
    ParserPinCase {
        workspace: WORKSPACE,
        linter: "[dependencies",
        lock: LOCK,
        expected: "parser-pin:linter-manifest:",
    },
    ParserPinCase {
        workspace: WORKSPACE,
        linter: LINTER,
        lock: "[[package",
        expected: "parser-pin:lockfile:",
    },
    ParserPinCase {
        workspace: "[workspace]\n",
        linter: LINTER,
        lock: LOCK,
        expected: "parser-pin:workspace-syn",
    },
    ParserPinCase {
        workspace: "[workspace.dependencies]\nsyn = { version = \"2\", features = [\"full\", \"visit\"] }\n",
        linter: LINTER,
        lock: LOCK,
        expected: "parser-pin:workspace-features",
    },
    ParserPinCase {
        workspace: "[workspace.dependencies]\nsyn = { version = \"3\", features = [\"full\"] }\n",
        linter: LINTER,
        lock: LOCK,
        expected: "parser-pin:workspace-features",
    },
    ParserPinCase {
        workspace: "[workspace.dependencies]\nsyn = { version = \"3\" }\n",
        linter: LINTER,
        lock: LOCK,
        expected: "parser-pin:features",
    },
    ParserPinCase {
        workspace: "[workspace.dependencies]\nsyn = { version = \"3\", features = \"full\" }\n",
        linter: LINTER,
        lock: LOCK,
        expected: "parser-pin:features",
    },
    ParserPinCase {
        workspace: "[workspace.dependencies]\nsyn = { version = \"3\", features = [\"full\", 2] }\n",
        linter: LINTER,
        lock: LOCK,
        expected: "parser-pin:feature-type",
    },
    ParserPinCase {
        workspace: WORKSPACE,
        linter: "[dependencies]\n",
        lock: LOCK,
        expected: "parser-pin:linter-syn",
    },
    ParserPinCase {
        workspace: WORKSPACE,
        linter: "[dependencies]\nsyn = { version = \"3\" }\n",
        lock: LOCK,
        expected: "parser-pin:linter-workspace-dependency",
    },
    ParserPinCase {
        workspace: WORKSPACE,
        linter: "[dependencies]\nsyn = { workspace = false }\n",
        lock: LOCK,
        expected: "parser-pin:linter-workspace-dependency",
    },
    ParserPinCase {
        workspace: WORKSPACE,
        linter: LINTER,
        lock: "[metadata]\n",
        expected: "parser-pin:lock-packages",
    },
    ParserPinCase {
        workspace: WORKSPACE,
        linter: LINTER,
        lock: "[[package]]\nname = \"syn\"\nversion = \"2.0.119\"\n",
        expected: "parser-pin:locked-syn-missing",
    },
    ParserPinCase {
        workspace: WORKSPACE,
        linter: LINTER,
        lock: "[[package]]\nname = \"syn\"\nversion = \"3.0.9\"\n[[package]]\nname = \"syn\"\nversion = \"3.0.10\"\n",
        expected: "parser-pin:locked-syn-ambiguous",
    },
];

#[test]
fn mathjax_consumer_facts_ast_linter_rejects_invalid_opaque_source_paths()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::copy_mathjax_inputs(INPUT_PATHS)?;
    fs::create_dir(fixture.root().join("source-directory"))?;
    let cases = [
        ("missing-source.rs", "missing-source:"),
        ("source-directory", "read-source:"),
        ("source-directory/../Cargo.toml", "unsafe-source-path:"),
    ];
    for (relative, expected) in cases {
        let Err(error) = super::opaque_text(fixture.root(), relative) else {
            return Err(format!("opaque source path was accepted: {relative}").into());
        };
        assert!(error.starts_with(expected), "unexpected error: {error}");
    }
    Ok(())
}

#[test]
fn mathjax_consumer_facts_ast_lookup_rejects_missing_or_ambiguous_items() -> Result<(), String> {
    for case in AST_LOOKUP_REJECTIONS {
        let file = syn::parse_file(case.source).map_err(|error| error.to_string())?;
        let actual = match case.kind {
            AstLookupKind::Function => super::free_function(&file, case.name).map(|_| ()),
            AstLookupKind::Method => super::method(&file, case.owner, case.name).map(|_| ()),
            AstLookupKind::TraitMethod => {
                super::trait_method(&file, case.trait_name, case.owner, case.name).map(|_| ())
            }
        };
        assert_eq!(actual.err().as_deref(), Some(case.expected));
    }
    Ok(())
}

#[test]
fn mathjax_consumer_facts_ast_lookup_rejects_nonmatching_expression_shapes() -> Result<(), String> {
    for (source, expected) in [
        ("value;", "route-statement:target"),
        ("let other = value;", "route-binding:target"),
        ("let target;", "route-initializer:target"),
    ] {
        let block = syn::parse_str::<syn::Block>(&format!("{{ {source} }}"))
            .map_err(|error| error.to_string())?;
        assert_eq!(
            super::local(&block, 0, "target").err().as_deref(),
            Some(expected)
        );
    }
    let call = syn::parse_str::<syn::Expr>("(factory())()").map_err(|error| error.to_string())?;
    assert_eq!(super::call_path(&call), None);
    let call = syn::parse_str::<syn::Expr>("factory()").map_err(|error| error.to_string())?;
    assert_eq!(super::call_path(&call).as_deref(), Some("factory"));
    let field = syn::parse_str::<syn::Expr>("item.0").map_err(|error| error.to_string())?;
    assert_eq!(super::path_name_from_expr(&field), "");
    let field = syn::parse_str::<syn::Expr>("(item.field)").map_err(|error| error.to_string())?;
    assert_eq!(super::path_name_from_expr(&field), "item.field");
    Ok(())
}

#[test]
fn mathjax_consumer_facts_ast_linter_accepts_current_parser_pin()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::copy_mathjax_inputs(INPUT_PATHS)?;
    let version = fixture.inspect_parser_pin()?;
    assert!(
        version.starts_with("3."),
        "unexpected current parser: {version}"
    );
    let pinned = fixture.inspect_parser_pin_inputs(WORKSPACE, LINTER, LOCK)?;
    assert_eq!(pinned, "3.0.9");
    Ok(())
}

#[test]
fn mathjax_consumer_facts_ast_linter_rejects_parser_pin_manifest_and_lock_cases()
-> Result<(), String> {
    let fixture = Fixture::copy_mathjax_inputs(INPUT_PATHS).map_err(|error| error.to_string())?;
    for case in REJECTIONS {
        let result = fixture.inspect_parser_pin_inputs(case.workspace, case.linter, case.lock);
        let Err(error) = result else {
            return Err(format!(
                "parser pin accepted case expecting {}",
                case.expected
            ));
        };
        assert!(
            error.to_string().starts_with(case.expected),
            "expected {} diagnostic, got: {error}",
            case.expected
        );
    }
    Ok(())
}
