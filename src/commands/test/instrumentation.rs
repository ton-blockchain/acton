use tolk_syntax::{HasName, SourceFile};

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
    file.functions().any(|func| {
        if let Some(name) = func.name() {
            let name = name.normalized_name(content);
            return name == "main" || name == "onInternalMessage";
        }

        false
    })
}
