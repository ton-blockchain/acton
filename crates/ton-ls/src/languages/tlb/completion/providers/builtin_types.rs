use crate::completion::candidate::CompletionCandidate;
use crate::completion::collector::CompletionCollector;
use crate::completion::provider::CompletionProvider;
use crate::completion::ranking::CompletionCategory;
use crate::languages::tlb::completion::context::TlbCompletionContext;
use lsp_types::CompletionItemKind;

#[derive(Default)]
pub(crate) struct BuiltinTypesCompletionProvider;

impl CompletionProvider<TlbCompletionContext> for BuiltinTypesCompletionProvider {
    fn id(&self) -> &'static str {
        "tlb.builtin_types"
    }

    fn is_applicable(&self, ctx: &TlbCompletionContext) -> bool {
        ctx.is_type
    }

    fn collect(&self, ctx: &TlbCompletionContext, out: &mut CompletionCollector) {
        for &(label, description) in BUILTIN_TYPES {
            let mut candidate = CompletionCandidate::new(label);
            candidate.kind = Some(CompletionItemKind::STRUCT);
            candidate.rank = ctx.rank_for(CompletionCategory::ContextElement, label);
            if !description.is_empty() {
                candidate.detail = Some(description.to_string());
            }
            out.add(candidate);
        }
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
