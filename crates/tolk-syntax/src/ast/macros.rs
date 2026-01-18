/// A helper macro to match parent nodes of a given kind.
///
/// This is useful when you need to find a parent node and optionally its own parent,
/// ensuring they match specific tree-sitter kinds.
#[macro_export]
macro_rules! match_parents {
    ($node:expr, $kind:ident (...)) => {
        $crate::ast::find_parent_by_kind(&$node, $kind::TREE_SITTER_KIND).map($kind)
    };

    ($node:expr, $kind:ident) => {
        $node
            .parent()
            .filter(|p| p.kind() == $kind::TREE_SITTER_KIND)
            .map(|n| $kind(n))
    };

    ($node:expr, $outer:ident ($binding:ident : $inner:ident (...))) => {
        $crate::match_parents!($node, $inner(...))
            .and_then(|inner_val| {
                inner_val.syntax().parent()
                    .filter(|p| p.kind() == $outer::TREE_SITTER_KIND)
                    .map(|n| ($outer(n), inner_val))
            })
    };

    ($node:expr, $outer:ident ($binding:ident : $inner:ident)) => {
        $crate::match_parents!($node, $inner)
            .and_then(|inner_val| {
                inner_val.syntax().parent()
                    .filter(|p| p.kind() == $outer::TREE_SITTER_KIND)
                    .map(|n| ($outer(n), inner_val))
            })
    };

    ($node:expr, $outer:ident (...) ($binding:ident : $inner:ident (...))) => {
        $crate::match_parents!($node, $inner(...))
            .and_then(|inner_val| {
                $crate::find_parent_by_kind(inner_val.syntax(), $outer::TREE_SITTER_KIND)
                    .map(|n| ($outer(n), inner_val))
            })
    };

    ($node:expr, $outer:ident (...) ($binding:ident : $inner:ident)) => {
        $crate::match_parents!($node, $inner)
            .and_then(|inner_val| {
                $crate::find_parent_by_kind(inner_val.syntax(), $outer::TREE_SITTER_KIND)
                    .map(|n| ($outer(n), inner_val))
            })
    };

    ($node:expr, $outer:ident (...) ($($inner:tt)*)) => {
        $crate::match_parents!($node, $($inner)*)
            .and_then(|n| $crate::find_parent_by_kind(n.syntax(), $outer::TREE_SITTER_KIND))
            .map($outer)
    };

    ($node:expr, $outer:ident ($($inner:tt)*)) => {
        $crate::match_parents!($node, $($inner)*)
            .and_then(|n| n.syntax().parent())
            .filter(|p| p.kind() == $outer::TREE_SITTER_KIND)
            .map(|n| $outer(n))
    };
}
