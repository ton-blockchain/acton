use tolk_syntax::{AstNode, HasName, SourceFile, TopLevel};

pub(super) fn prepare_test_file(content: &str) -> String {
    let Ok(file) = tolk_syntax::parse(content) else {
        return String::new();
    };

    if has_entry_function(&file, content) {
        // very unlikely
        return content.to_owned();
    }

    format!("{content}\n\nfun main() {{}}")
}

fn has_entry_function(file: &SourceFile, content: &str) -> bool {
    file.top_levels()
        .filter_map(|d| match d {
            TopLevel::Func(func) => Some(func),
            _ => None,
        })
        .any(|func| {
            if let Some(name) = func.name() {
                let name = name.text(content);
                return name == "main" || name == "onInternalMessage";
            }

            false
        })
}
