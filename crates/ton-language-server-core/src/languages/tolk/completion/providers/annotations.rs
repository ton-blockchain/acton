use super::support::{ProviderGroup, provider_group};
use super::{DUMMY_IDENTIFIER, TolkCompletionContext, TolkCompletionProviderContext};
use crate::completion::{
    CompletionCategory, CompletionCollector, CompletionProvider, CompletionRank,
};
use crate::{CompletionItem, CompletionItemKind};

pub(crate) struct AnnotationCompletionProvider;

impl CompletionProvider<TolkCompletionProviderContext<'_>> for AnnotationCompletionProvider {
    fn is_applicable(&self, context: &TolkCompletionProviderContext<'_>) -> bool {
        provider_group(context.syntax) == ProviderGroup::Annotation
    }

    fn collect(
        &self,
        context: &TolkCompletionProviderContext<'_>,
        collector: &mut CompletionCollector,
    ) {
        let Some(path_prefix) = annotation_path_prefix(context.syntax) else {
            return;
        };
        let owner = annotation_owner(context.syntax);
        let existing = existing_annotations(context.syntax, owner);
        let annotations = if !path_prefix.contains('.') {
            ROOT_ANNOTATIONS
        } else if path_prefix.starts_with("abi.") {
            ABI_ANNOTATIONS
        } else if path_prefix.starts_with("test.") {
            TEST_ANNOTATIONS
        } else {
            return;
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

fn annotation_owner(context: &TolkCompletionContext) -> Option<tree_sitter::Node<'_>> {
    let mut node = context.cursor_node()?;
    loop {
        if matches!(
            node.kind(),
            "function_declaration"
                | "get_method_declaration"
                | "method_declaration"
                | "struct_declaration"
                | "struct_field_declaration"
                | "global_var_declaration"
                | "constant_declaration"
                | "type_alias_declaration"
                | "enum_declaration"
        ) {
            return Some(node);
        }
        node = node.parent()?;
    }
}

fn existing_annotations(
    context: &TolkCompletionContext,
    owner: Option<tree_sitter::Node<'_>>,
) -> std::collections::BTreeSet<String> {
    let Some(annotations) = owner.and_then(|owner| owner.child_by_field_name("annotations")) else {
        return std::collections::BTreeSet::new();
    };
    let mut cursor = annotations.walk();
    annotations
        .named_children(&mut cursor)
        .filter(|node| node.kind() == "annotation")
        .filter_map(|node| node.child_by_field_name("name"))
        .filter_map(|node| node.utf8_text(context.source().as_bytes()).ok())
        .filter(|name| !name.ends_with(DUMMY_IDENTIFIER))
        .map(str::to_owned)
        .collect()
}

fn annotation_applies(
    annotation: &AnnotationSpec,
    owner: Option<tree_sitter::Node<'_>>,
    source: &str,
) -> bool {
    if annotation.owners.contains(&AnnotationOwner::Any) || owner.is_none() {
        return true;
    }
    let Some(owner) = owner else {
        return true;
    };
    annotation.owners.iter().any(|kind| match kind {
        AnnotationOwner::Any => true,
        AnnotationOwner::Function => matches!(
            owner.kind(),
            "function_declaration" | "method_declaration" | "get_method_declaration"
        ),
        AnnotationOwner::GetMethod => owner.kind() == "get_method_declaration",
        AnnotationOwner::Struct => owner.kind() == "struct_declaration",
        AnnotationOwner::Field => owner.kind() == "struct_field_declaration",
        AnnotationOwner::EntryPoint => {
            owner.kind() == "function_declaration"
                && owner
                    .child_by_field_name("name")
                    .and_then(|name| name.utf8_text(source.as_bytes()).ok())
                    .is_some_and(|name| {
                        matches!(
                            name,
                            "onInternalMessage" | "onExternalMessage" | "onBouncedMessage"
                        )
                    })
        }
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
