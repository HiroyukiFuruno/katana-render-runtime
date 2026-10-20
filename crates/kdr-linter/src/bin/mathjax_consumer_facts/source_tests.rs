use super::super::test_support::Fixture;

const INPUT_PATHS: &[&str] = &["Cargo.toml", "Cargo.lock", "crates/kdr-linter/Cargo.toml"];
const WORKSPACE: &str = "[workspace]\n[workspace.dependencies]\nsyn = { version = \"3\", features = [\"full\", \"visit\"] }\n";
const LINTER: &str = "[dependencies]\nsyn = { workspace = true }\n";
const LOCK: &str = "[[package]]\nname = \"syn\"\nversion = \"3.0.9\"\n";

struct ParserPinCase {
    workspace: &'static str,
    linter: &'static str,
    lock: &'static str,
    expected: &'static str,
}

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
fn mathjax_consumer_facts_ast_linter_accepts_current_parser_pin() -> Result<(), String> {
    let fixture = Fixture::copy_mathjax_inputs(INPUT_PATHS).map_err(|error| error.to_string())?;
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
            error.starts_with(case.expected),
            "expected {} diagnostic, got: {error}",
            case.expected
        );
    }
    Ok(())
}
