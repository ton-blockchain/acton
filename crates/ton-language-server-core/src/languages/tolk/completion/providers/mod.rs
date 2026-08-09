mod acton_contract_id;
mod acton_get_method;
mod acton_wallet_name;
mod annotations;
mod contract_fields;
mod entry_points;
mod enum_declaration;
mod enum_values;
mod expression_snippets;
mod field_init;
mod field_modifiers;
mod function_names;
mod import_path;
mod index_access;
mod keywords;
mod match_arms;
mod references;
mod returns;
mod statement_snippets;
mod storage;
mod support;
mod throw_assert;
mod top_level;
mod variable_size_types;

use super::TolkCompletionProviderContext;
use crate::completion::{CompletionCollector, CompletionProvider};
use crate::languages::tolk::TolkResolveSnapshot;
use crate::languages::tolk::completion::imports::WorkspaceCompletionData;
use crate::profiling::BufferedProfiler;
use crate::{CompletionList, DocumentSnapshot, Profiler};
use tolk_resolver::FileId;

pub(super) fn collect(
    snapshot: &TolkResolveSnapshot,
    file_id: FileId,
    document: &DocumentSnapshot,
    syntax: &TolkCompletionContext,
    workspace: WorkspaceCompletionData<'_>,
    profiler: &mut Profiler,
) -> CompletionList {
    let provider_profiler = BufferedProfiler::new(profiler);
    let context = TolkCompletionProviderContext {
        snapshot,
        file_id,
        visible_globals: tolk_resolver::symbol_resolver::GlobalEnv::new(
            &snapshot.project_index,
            file_id,
        ),
        document,
        syntax,
        workspace,
        profiler: &provider_profiler,
    };

    let top_level = TopLevelCompletionProvider;
    let statement_snippets = StatementSnippetCompletionProvider;
    let expression_snippets = ExpressionSnippetCompletionProvider;
    let keywords = KeywordCompletionProvider;
    let references = ReferenceCompletionProvider;
    let throw_assert = ThrowAssertCompletionProvider;
    let returns = ReturnCompletionProvider;
    let entry_points = EntryPointCompletionProvider;
    let annotations = AnnotationCompletionProvider;
    let index_access = IndexAccessCompletionProvider;
    let variable_size_types = VariableSizeTypeCompletionProvider;
    let match_arms = MatchArmCompletionProvider;
    let storage = StorageCompletionProvider;
    let field_init = FieldInitCompletionProvider;
    let function_names = FunctionNameCompletionProvider;
    let field_modifiers = FieldModifierCompletionProvider;
    let enum_declaration = EnumDeclarationCompletionProvider;
    let enum_values = EnumCompletionProvider;
    let contract_fields = ContractFieldCompletionProvider;
    let wallet_names = ActonWalletNameCompletionProvider;
    let contract_ids = ActonContractIdCompletionProvider;
    let import_paths = ImportPathCompletionProvider;
    let get_methods = ActonGetMethodCompletionProvider;
    let providers: [(
        &'static str,
        &dyn CompletionProvider<TolkCompletionProviderContext<'_>>,
    ); 23] = [
        ("tolk.completion.provider.top_level", &top_level),
        (
            "tolk.completion.provider.statement_snippets",
            &statement_snippets,
        ),
        (
            "tolk.completion.provider.expression_snippets",
            &expression_snippets,
        ),
        ("tolk.completion.provider.keywords", &keywords),
        ("tolk.completion.provider.references", &references),
        ("tolk.completion.provider.throw_assert", &throw_assert),
        ("tolk.completion.provider.returns", &returns),
        ("tolk.completion.provider.entry_points", &entry_points),
        ("tolk.completion.provider.annotations", &annotations),
        ("tolk.completion.provider.index_access", &index_access),
        (
            "tolk.completion.provider.variable_size_types",
            &variable_size_types,
        ),
        ("tolk.completion.provider.match_arms", &match_arms),
        ("tolk.completion.provider.storage", &storage),
        ("tolk.completion.provider.field_init", &field_init),
        ("tolk.completion.provider.function_names", &function_names),
        ("tolk.completion.provider.field_modifiers", &field_modifiers),
        (
            "tolk.completion.provider.enum_declaration",
            &enum_declaration,
        ),
        ("tolk.completion.provider.enum_values", &enum_values),
        ("tolk.completion.provider.contract_fields", &contract_fields),
        ("tolk.completion.provider.wallet_names", &wallet_names),
        ("tolk.completion.provider.contract_ids", &contract_ids),
        ("tolk.completion.provider.import_paths", &import_paths),
        ("tolk.completion.provider.get_methods", &get_methods),
    ];

    let mut collector = CompletionCollector::new();
    for (profile_name, provider) in providers {
        if !provider.is_applicable(&context) {
            continue;
        }

        let started_at = profiler.start();
        provider.collect(&context, &mut collector);
        profiler.finish(profile_name, started_at);
        provider_profiler.flush_into(profiler);
    }

    let finish_started_at = profiler.start();
    let completion = collector.finish();
    profiler.finish("tolk.completion.finish", finish_started_at);
    completion
}

pub(super) use super::context::{DUMMY_IDENTIFIER, TolkCompletionContext};
pub(super) use super::imports::{matches_call, string_prefix_and_range};
pub(super) use acton_contract_id::ActonContractIdCompletionProvider;
pub(super) use acton_get_method::ActonGetMethodCompletionProvider;
pub(super) use acton_wallet_name::ActonWalletNameCompletionProvider;
pub(super) use annotations::AnnotationCompletionProvider;
pub(super) use contract_fields::ContractFieldCompletionProvider;
pub(super) use entry_points::EntryPointCompletionProvider;
pub(super) use enum_declaration::EnumDeclarationCompletionProvider;
pub(super) use enum_values::EnumCompletionProvider;
pub(super) use expression_snippets::ExpressionSnippetCompletionProvider;
pub(super) use field_init::FieldInitCompletionProvider;
pub(super) use field_modifiers::FieldModifierCompletionProvider;
pub(super) use function_names::FunctionNameCompletionProvider;
pub(super) use import_path::ImportPathCompletionProvider;
pub(super) use index_access::IndexAccessCompletionProvider;
pub(super) use keywords::KeywordCompletionProvider;
pub(super) use match_arms::MatchArmCompletionProvider;
pub(super) use references::ReferenceCompletionProvider;
pub(super) use returns::ReturnCompletionProvider;
pub(super) use statement_snippets::StatementSnippetCompletionProvider;
pub(super) use storage::StorageCompletionProvider;
pub(super) use throw_assert::ThrowAssertCompletionProvider;
pub(super) use top_level::TopLevelCompletionProvider;
pub(super) use variable_size_types::VariableSizeTypeCompletionProvider;
