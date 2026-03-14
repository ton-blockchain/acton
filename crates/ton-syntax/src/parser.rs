use tree_sitter::{Language, Parser, Tree};

/// Parses source code with a specific tree-sitter language, potentially reusing an old tree.
///
/// # Errors
///
/// Returns an error if parser language cannot be set or if parser returns no tree.
pub fn parse_with_old_tree(
    code: &str,
    old_tree: Option<&Tree>,
    language: Language,
    language_name: &str,
) -> anyhow::Result<Tree> {
    let mut parser = Parser::new();
    parser.set_language(&language)?;

    let Some(tree) = parser.parse(code, old_tree) else {
        anyhow::bail!("cannot parse {language_name} file");
    };

    Ok(tree)
}
