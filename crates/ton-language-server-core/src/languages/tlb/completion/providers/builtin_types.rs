use super::super::context::TlbCompletionContext;
use crate::completion::{CompletionCategory, CompletionCollector, CompletionProvider};
use crate::{CompletionItem, CompletionItemKind};

/// Completes TL-B builtin type constructors and their parameterized width forms.
///
/// The provider is active only in a type position and inserts the replacement for
/// the type token under the cursor.
pub(crate) struct BuiltinTypesCompletionProvider;

impl CompletionProvider<TlbCompletionContext> for BuiltinTypesCompletionProvider {
    fn is_applicable(&self, context: &TlbCompletionContext) -> bool {
        context.is_type
    }

    fn collect(
        &self,
        context: &TlbCompletionContext,
        collector: &mut CompletionCollector,
    ) -> Option<()> {
        for &(label, detail) in BUILTIN_TYPES {
            let mut item = CompletionItem::new(label, CompletionItemKind::Struct)
                .with_replacement(context.replacement_range, label);
            if !detail.is_empty() {
                item.detail = Some(detail.to_owned());
            }
            collector.add(
                item,
                context.rank_for(CompletionCategory::ContextElement, label),
            );
        }
        Some(())
    }
}

const BUILTIN_TYPES: &[(&str, &str)] = &[
    ("#", "Nat, 32-bit unsigned integer"),
    ("##", "Nat: unsigned integer with `x` bits"),
    (
        "#<",
        "Nat: unsigned integer less than `x` stored with minimum bits",
    ),
    (
        "#<=",
        "Nat: unsigned integer less than or equal `x` stored with minimum bits",
    ),
    ("Any", "Remaining bits and references"),
    ("Cell", "Remaining bits and references"),
    ("Int", "257 bits"),
    ("UInt", "256 bits"),
    ("Bits", "1023 bits"),
    ("bits", "X bits"),
    ("uint", ""),
    ("uint8", ""),
    ("uint16", ""),
    ("uint32", ""),
    ("uint64", ""),
    ("uint128", ""),
    ("uint256", ""),
    ("int", ""),
    ("int8", ""),
    ("int16", ""),
    ("int32", ""),
    ("int64", ""),
    ("int128", ""),
    ("int256", ""),
    ("int257", ""),
    ("Type", "Built-in TL-B type representing the type of types"),
];
