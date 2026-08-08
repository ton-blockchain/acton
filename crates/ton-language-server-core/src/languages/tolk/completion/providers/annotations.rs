use super::support::{ProviderGroup, provider_group};
use super::{DUMMY_IDENTIFIER, TolkCompletionContext, TolkCompletionProviderContext};
use crate::completion::{
    CompletionCategory, CompletionCollector, CompletionProvider, CompletionRank,
};
use crate::{CompletionItem, CompletionItemKind};
use tolk_syntax::{AnnotatedDeclaration, HasAnnotations, HasName};

/// Completes root, ABI, test, and declaration-specific annotations.
///
/// The typed declaration owner controls which annotation paths are valid, while
/// annotations already present on that declaration are omitted from the results.
pub(crate) struct AnnotationCompletionProvider;

impl CompletionProvider<TolkCompletionProviderContext<'_>> for AnnotationCompletionProvider {
    fn is_applicable(&self, context: &TolkCompletionProviderContext<'_>) -> bool {
        provider_group(context.syntax) == ProviderGroup::Annotation
    }

    fn collect(
        &self,
        context: &TolkCompletionProviderContext<'_>,
        collector: &mut CompletionCollector,
    ) -> Option<()> {
        let path_prefix = annotation_path_prefix(context.syntax)?;
        let owner = context.syntax.annotation_owner();
        let existing = existing_annotations(context.syntax, owner);
        let annotations = if !path_prefix.contains('.') {
            ROOT_ANNOTATIONS
        } else if path_prefix.starts_with("abi.") {
            ABI_ANNOTATIONS
        } else if path_prefix.starts_with("test.") {
            TEST_ANNOTATIONS
        } else {
            return None;
        };
        for annotation in annotations {
            if existing.contains(annotation.full_name)
                || !annotation_applies(annotation, owner, context.syntax.source())
            {
                continue;
            }
            let insertion = annotation.insertion.unwrap_or(annotation.label);
            collector.add(
                CompletionItem::new(annotation.label, CompletionItemKind::Event)
                    .with_snippet_replacement(context.syntax.replacement_range, insertion),
                CompletionRank::new(CompletionCategory::Snippet)
                    .with_prefix(&context.syntax.prefix, annotation.label),
            );
        }
        Some(())
    }
}

fn annotation_path_prefix(context: &TolkCompletionContext) -> Option<&str> {
    let before = context.source().get(..context.offset)?;
    let start = before
        .char_indices()
        .rev()
        .find(|(_, character)| {
            !character.is_ascii_alphanumeric() && !matches!(character, '_' | '.')
        })
        .map_or(0, |(index, character)| index + character.len_utf8());
    start
        .checked_sub(1)
        .filter(|index| before.as_bytes().get(*index) == Some(&b'@'))?;
    Some(&before[start..])
}

fn existing_annotations(
    context: &TolkCompletionContext,
    owner: Option<AnnotatedDeclaration<'_>>,
) -> std::collections::BTreeSet<String> {
    let Some(annotations) = owner.and_then(|owner| owner.annotations()) else {
        return std::collections::BTreeSet::new();
    };
    annotations
        .annotations()
        .filter_map(|annotation| annotation.name())
        .map(|name| context.text_of(name))
        .filter(|name| !name.ends_with(DUMMY_IDENTIFIER))
        .map(str::to_owned)
        .collect()
}

fn annotation_applies(
    annotation: &AnnotationSpec,
    owner: Option<AnnotatedDeclaration<'_>>,
    source: &str,
) -> bool {
    if annotation.owners.contains(&AnnotationOwner::Any) || owner.is_none() {
        return true;
    }
    let Some(owner) = owner else { return true };
    annotation.owners.iter().any(|kind| match kind {
        AnnotationOwner::Any => true,
        AnnotationOwner::Function => owner.is_function(),
        AnnotationOwner::GetMethod => owner.is_get_method(),
        AnnotationOwner::Struct => owner.is_struct(),
        AnnotationOwner::Field => owner.is_field(),
        AnnotationOwner::EntryPoint => owner.is_entry_point(source),
    })
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum AnnotationOwner {
    Any,
    Function,
    GetMethod,
    Struct,
    Field,
    EntryPoint,
}

struct AnnotationSpec {
    label: &'static str,
    full_name: &'static str,
    owners: &'static [AnnotationOwner],
    insertion: Option<&'static str>,
}

const FUNCTIONS: &[AnnotationOwner] = &[AnnotationOwner::Function, AnnotationOwner::GetMethod];
const GET_METHODS: &[AnnotationOwner] = &[AnnotationOwner::GetMethod];
const STRUCTS: &[AnnotationOwner] = &[AnnotationOwner::Struct];
const FIELDS: &[AnnotationOwner] = &[AnnotationOwner::Field];
const ENTRY_POINTS: &[AnnotationOwner] = &[AnnotationOwner::EntryPoint];
const ANY_OWNER: &[AnnotationOwner] = &[AnnotationOwner::Any];

const ROOT_ANNOTATIONS: &[AnnotationSpec] = &[
    AnnotationSpec {
        label: "inline",
        full_name: "inline",
        owners: FUNCTIONS,
        insertion: None,
    },
    AnnotationSpec {
        label: "pure",
        full_name: "pure",
        owners: FUNCTIONS,
        insertion: None,
    },
    AnnotationSpec {
        label: "inline_ref",
        full_name: "inline_ref",
        owners: FUNCTIONS,
        insertion: None,
    },
    AnnotationSpec {
        label: "noinline",
        full_name: "noinline",
        owners: FUNCTIONS,
        insertion: None,
    },
    AnnotationSpec {
        label: "test",
        full_name: "test",
        owners: GET_METHODS,
        insertion: None,
    },
    AnnotationSpec {
        label: "method_id",
        full_name: "method_id",
        owners: FUNCTIONS,
        insertion: Some("method_id(${1:0x1})$0"),
    },
    AnnotationSpec {
        label: "abi.minimalMsgValue",
        full_name: "abi.minimalMsgValue",
        owners: STRUCTS,
        insertion: Some("abi.minimalMsgValue($0)"),
    },
    AnnotationSpec {
        label: "abi.preferredSendMode",
        full_name: "abi.preferredSendMode",
        owners: STRUCTS,
        insertion: Some("abi.preferredSendMode($0)"),
    },
    AnnotationSpec {
        label: "abi.clientType",
        full_name: "abi.clientType",
        owners: FIELDS,
        insertion: Some("abi.clientType($0)"),
    },
    AnnotationSpec {
        label: "deprecated",
        full_name: "deprecated",
        owners: ANY_OWNER,
        insertion: Some("deprecated(\"$0\")"),
    },
    AnnotationSpec {
        label: "custom",
        full_name: "custom",
        owners: ANY_OWNER,
        insertion: Some("custom($0)"),
    },
    AnnotationSpec {
        label: "overflow1023_policy",
        full_name: "overflow1023_policy",
        owners: STRUCTS,
        insertion: Some("overflow1023_policy(\"${1:suppress}\")$0"),
    },
    AnnotationSpec {
        label: "on_bounced_policy",
        full_name: "on_bounced_policy",
        owners: ENTRY_POINTS,
        insertion: Some("on_bounced_policy(\"${1:manual}\")$0"),
    },
];

const ABI_ANNOTATIONS: &[AnnotationSpec] = &[
    AnnotationSpec {
        label: "minimalMsgValue",
        full_name: "abi.minimalMsgValue",
        owners: STRUCTS,
        insertion: Some("minimalMsgValue($0)"),
    },
    AnnotationSpec {
        label: "preferredSendMode",
        full_name: "abi.preferredSendMode",
        owners: STRUCTS,
        insertion: Some("preferredSendMode($0)"),
    },
    AnnotationSpec {
        label: "clientType",
        full_name: "abi.clientType",
        owners: FIELDS,
        insertion: Some("clientType($0)"),
    },
];

const TEST_ANNOTATIONS: &[AnnotationSpec] = &[
    AnnotationSpec {
        label: "skip",
        full_name: "test.skip",
        owners: GET_METHODS,
        insertion: None,
    },
    AnnotationSpec {
        label: "todo",
        full_name: "test.todo",
        owners: GET_METHODS,
        insertion: None,
    },
    AnnotationSpec {
        label: "todo",
        full_name: "test.todo",
        owners: GET_METHODS,
        insertion: Some("todo(\"$0\")"),
    },
    AnnotationSpec {
        label: "fail_with",
        full_name: "test.fail_with",
        owners: GET_METHODS,
        insertion: Some("fail_with($0)"),
    },
    AnnotationSpec {
        label: "gas_limit",
        full_name: "test.gas_limit",
        owners: GET_METHODS,
        insertion: Some("gas_limit($0)"),
    },
    AnnotationSpec {
        label: "fuzz",
        full_name: "test.fuzz",
        owners: GET_METHODS,
        insertion: None,
    },
    AnnotationSpec {
        label: "fuzz",
        full_name: "test.fuzz",
        owners: GET_METHODS,
        insertion: Some("fuzz($0)"),
    },
];
