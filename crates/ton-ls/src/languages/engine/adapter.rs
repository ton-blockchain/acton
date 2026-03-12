use tree_sitter::Tree;

pub trait SyntaxAdapter: Send + Sync + 'static {
    type SourceFile: Clone + Send + Sync + 'static;

    fn parse(source: &str) -> anyhow::Result<Self::SourceFile>;

    fn parse_with_old_tree(
        source: &str,
        old_tree: Option<&Tree>,
    ) -> anyhow::Result<Self::SourceFile>;

    fn tree(source_file: &Self::SourceFile) -> &Tree;
}

#[derive(Debug, Clone, Copy, Default)]
pub struct TasmSyntaxAdapter;

impl SyntaxAdapter for TasmSyntaxAdapter {
    type SourceFile = tasm_syntax::SourceFile;

    fn parse(source: &str) -> anyhow::Result<Self::SourceFile> {
        tasm_syntax::parse(source)
    }

    fn parse_with_old_tree(
        source: &str,
        old_tree: Option<&Tree>,
    ) -> anyhow::Result<Self::SourceFile> {
        tasm_syntax::parse_with_old_tree(source, old_tree)
    }

    fn tree(source_file: &Self::SourceFile) -> &Tree {
        &source_file.tree
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct FiftSyntaxAdapter;

impl SyntaxAdapter for FiftSyntaxAdapter {
    type SourceFile = fift_syntax::SourceFile;

    fn parse(source: &str) -> anyhow::Result<Self::SourceFile> {
        fift_syntax::parse(source)
    }

    fn parse_with_old_tree(
        source: &str,
        old_tree: Option<&Tree>,
    ) -> anyhow::Result<Self::SourceFile> {
        fift_syntax::parse_with_old_tree(source, old_tree)
    }

    fn tree(source_file: &Self::SourceFile) -> &Tree {
        &source_file.tree
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct TomlSyntaxAdapter;

impl SyntaxAdapter for TomlSyntaxAdapter {
    type SourceFile = toml_syntax::SourceFile;

    fn parse(source: &str) -> anyhow::Result<Self::SourceFile> {
        toml_syntax::parse(source)
    }

    fn parse_with_old_tree(
        source: &str,
        old_tree: Option<&Tree>,
    ) -> anyhow::Result<Self::SourceFile> {
        toml_syntax::parse_with_old_tree(source, old_tree)
    }

    fn tree(source_file: &Self::SourceFile) -> &Tree {
        &source_file.tree
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct TlbSyntaxAdapter;

impl SyntaxAdapter for TlbSyntaxAdapter {
    type SourceFile = tlb_syntax::SourceFile;

    fn parse(source: &str) -> anyhow::Result<Self::SourceFile> {
        tlb_syntax::parse(source)
    }

    fn parse_with_old_tree(
        source: &str,
        old_tree: Option<&Tree>,
    ) -> anyhow::Result<Self::SourceFile> {
        tlb_syntax::parse_with_old_tree(source, old_tree)
    }

    fn tree(source_file: &Self::SourceFile) -> &Tree {
        &source_file.tree
    }
}
