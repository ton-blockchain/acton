use anyhow::bail;
use lsp_types::{
    FoldingRangeParams, GotoDefinitionParams, HoverParams, InitializeResult, PartialResultParams,
    Position, ReferenceContext, ReferenceParams, SemanticTokensLegend, SemanticTokensParams,
    SemanticTokensRegistrationOptions, SemanticTokensServerCapabilities, TextDocumentIdentifier,
    TextDocumentPositionParams, Url, WorkDoneProgressParams,
};

pub(crate) fn uri_for_case(case_name: &str, extension: &str) -> Url {
    let mut file_name = case_name.replace(' ', "_");
    if file_name.is_empty() {
        file_name = "unnamed".to_owned();
    }
    let path = std::env::temp_dir()
        .join("ton-ls-self-contained-tests")
        .join(format!("{file_name}.{extension}"));
    Url::from_file_path(path).expect("self-contained test URI must be valid file URI")
}

pub(crate) fn goto_definition_params(uri: Url, position: Position) -> GotoDefinitionParams {
    GotoDefinitionParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position,
        },
        work_done_progress_params: WorkDoneProgressParams {
            work_done_token: Option::<lsp_types::ProgressToken>::None,
        },
        partial_result_params: PartialResultParams {
            partial_result_token: Option::<lsp_types::ProgressToken>::None,
        },
    }
}

pub(crate) fn semantic_tokens_params(uri: Url) -> SemanticTokensParams {
    SemanticTokensParams {
        work_done_progress_params: WorkDoneProgressParams {
            work_done_token: Option::<lsp_types::ProgressToken>::None,
        },
        partial_result_params: PartialResultParams {
            partial_result_token: Option::<lsp_types::ProgressToken>::None,
        },
        text_document: TextDocumentIdentifier { uri },
    }
}

pub(crate) fn hover_params(uri: Url, position: Position) -> HoverParams {
    HoverParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position,
        },
        work_done_progress_params: WorkDoneProgressParams {
            work_done_token: Option::<lsp_types::ProgressToken>::None,
        },
    }
}

pub(crate) fn references_params(
    uri: Url,
    position: Position,
    include_declaration: bool,
) -> ReferenceParams {
    ReferenceParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position,
        },
        work_done_progress_params: WorkDoneProgressParams {
            work_done_token: Option::<lsp_types::ProgressToken>::None,
        },
        partial_result_params: PartialResultParams {
            partial_result_token: Option::<lsp_types::ProgressToken>::None,
        },
        context: ReferenceContext {
            include_declaration,
        },
    }
}

pub(crate) fn folding_range_params(uri: Url) -> FoldingRangeParams {
    FoldingRangeParams {
        text_document: TextDocumentIdentifier { uri },
        work_done_progress_params: WorkDoneProgressParams {
            work_done_token: Option::<lsp_types::ProgressToken>::None,
        },
        partial_result_params: PartialResultParams {
            partial_result_token: Option::<lsp_types::ProgressToken>::None,
        },
    }
}

pub(crate) fn extract_semantic_legend(
    init: &InitializeResult,
) -> anyhow::Result<SemanticTokensLegend> {
    let Some(capability) = init.capabilities.semantic_tokens_provider.as_ref() else {
        bail!("semantic_tokens_provider is not available in initialize result");
    };

    match capability {
        SemanticTokensServerCapabilities::SemanticTokensOptions(options) => {
            Ok(options.legend.clone())
        }
        SemanticTokensServerCapabilities::SemanticTokensRegistrationOptions(
            SemanticTokensRegistrationOptions {
                semantic_tokens_options,
                ..
            },
        ) => Ok(semantic_tokens_options.legend.clone()),
    }
}

pub(crate) fn slice_line_utf16(source: &str, line: u32, start: u32, end: u32) -> Option<String> {
    let line_text = source.split('\n').nth(line as usize)?;
    let start_offset = utf16_column_to_byte_offset(line_text, start)?;
    let end_offset = utf16_column_to_byte_offset(line_text, end)?;
    Some(line_text[start_offset..end_offset].to_owned())
}

fn utf16_column_to_byte_offset(line: &str, column: u32) -> Option<usize> {
    let mut utf16 = 0u32;
    for (byte_offset, ch) in line.char_indices() {
        if utf16 == column {
            return Some(byte_offset);
        }

        utf16 += ch.len_utf16() as u32;
        if utf16 > column {
            return None;
        }
    }

    if utf16 == column {
        return Some(line.len());
    }

    None
}
