use tolk_syntax::{HasName, SourceFile};

pub(super) fn prepare_test_file(file: &anyhow::Result<SourceFile>, content: &str) -> String {
    let Ok(file) = file else {
        return format!("{content}\n\nfun main() {{}}");
    };

    if has_entry_function(file, content) {
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
