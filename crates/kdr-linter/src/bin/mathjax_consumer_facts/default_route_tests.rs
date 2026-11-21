#![cfg(test)]

use super::require_default_install_path;
use crate::source::parsed_block;

const INSTALL_PATH: &str = "std::env::temp_dir().join(\"katana-render-runtime\").join(\"generated\").join(\"mathjax-runtime.min.js\")";
const INSTALL_PATH_ERROR: &str = "default-materialize:install-path";

#[test]
fn default_install_path_accepts_exactly_one_argument() -> Result<(), String> {
    let block = parsed_block(&format!("fn route() {{ Some({INSTALL_PATH}) }}"), "route")?;
    require_default_install_path(&block)
}

#[test]
fn default_install_path_rejects_zero_or_two_arguments_with_same_error() -> Result<(), String> {
    for expression in ["Some()".to_string(), format!("Some({INSTALL_PATH}, other)")] {
        let block = parsed_block(&format!("fn route() {{ {expression} }}"), "route")?;
        assert_eq!(
            require_default_install_path(&block).err().as_deref(),
            Some(INSTALL_PATH_ERROR),
        );
    }
    Ok(())
}
