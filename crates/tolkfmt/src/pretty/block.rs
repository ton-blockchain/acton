//! Formatting helpers for block-like documents.
//!
//! Some number of prefixes and suffixes can ideally be laid out on a single line instead of being
//! broken into multiple lines. See `BlockDoc` for an example.

use crate::{
    docs,
    pretty::{Doc, DocAllocator, DocBuilder},
};

pub struct Affixes<'doc, D, A>
where
    D: DocAllocator<'doc, A>,
{
    prefix: DocBuilder<'doc, D, A>,
    suffix: DocBuilder<'doc, D, A>,
    nest: bool,
}

impl<'a, D, A> Clone for Affixes<'a, D, A>
where
    A: Clone,
    D: DocAllocator<'a, A> + 'a,
    D::Doc: Clone,
{
    fn clone(&self) -> Self {
        Affixes {
            prefix: self.prefix.clone(),
            suffix: self.suffix.clone(),
            nest: self.nest,
        }
    }
}

impl<'doc, D, A> Affixes<'doc, D, A>
where
    D: DocAllocator<'doc, A>,
{
    pub const fn new(prefix: DocBuilder<'doc, D, A>, suffix: DocBuilder<'doc, D, A>) -> Self {
        Affixes {
            prefix,
            suffix,
            nest: false,
        }
    }

    pub const fn nest(mut self) -> Self {
        self.nest = true;
        self
    }
}

/// Formats a set of `prefix` and `suffix` documents around a `body`
///
/// The following document split into the prefixes [\x y ->, \z ->, {], suffixes [nil, nil, }] and
/// body [result: x + y - z] will try to be formatted
///
/// ```gluon
/// \x y -> \z -> { result: x + y - z }
/// ```
///
/// ```gluon
/// \x y -> \z -> {
///     result: x + y - z
/// }
/// ```
///
/// ```gluon
/// \x y -> \z ->
///     {
///         result: x + y - z
///     }
/// ```
///
/// ```gluon
/// \x y ->
///     \z ->
///         {
///             result: x + y - z
///         }
/// ```
pub struct BlockDoc<'doc, D, A>
where
    D: DocAllocator<'doc, A>,
{
    pub affixes: Vec<Affixes<'doc, D, A>>,
    pub body: DocBuilder<'doc, D, A>,
}

impl<'doc, D, A> BlockDoc<'doc, D, A>
where
    D: DocAllocator<'doc, A>,
    D::Doc: Clone,
    A: Clone,
{
    pub fn format(self, nest: isize) -> DocBuilder<'doc, D, A> {
        let arena = self.body.0;

        let fail_on_multi_line = arena.fail().flat_alt(arena.nil());

        let mut docs = (1..self.affixes.len() + 1).rev().map(|split| {
            let (before, after) = self.affixes.split_at(split);
            let last = before.len() == 1;
            docs![
                arena,
                docs![
                    arena,
                    arena.concat(before.iter().map(|affixes| affixes.prefix.clone())),
                    if last {
                        arena.nil()
                    } else {
                        fail_on_multi_line.clone()
                    }
                ]
                .group(),
                docs![
                    arena,
                    after.iter().rev().cloned().fold(
                        docs![
                            arena,
                            self.body.clone(),
                            // If there is no prefix then we must not allow the body to laid out on multiple
                            // lines without nesting
                            if !last
                                && before
                                    .iter()
                                    .all(|affixes| matches!(&*affixes.prefix.1, Doc::Nil))
                            {
                                fail_on_multi_line.clone()
                            } else {
                                arena.nil()
                            },
                        ]
                        .nest(nest)
                        .append(arena.concat(after.iter().map(|affixes| affixes.suffix.clone()))),
                        |acc, affixes| {
                            let mut doc = affixes.prefix.append(acc);
                            if affixes.nest {
                                doc = doc.nest(nest);
                            }
                            doc.group()
                        },
                    ),
                    arena.concat(before.iter().map(|affixes| affixes.suffix.clone())),
                ]
                .group(),
            ]
        });

        let Some(first) = docs.next() else {
            return self.body;
        };

        docs.fold(first, |acc, doc| acc.union(doc))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::pretty::RcAllocator;

    #[test]
    fn format_block() {
        let arena = &RcAllocator;
        let mk_doc = || -> BlockDoc<'_, RcAllocator, ()> {
            BlockDoc {
                affixes: vec![
                    Affixes::new(docs![arena, "\\x y ->"], arena.nil()).nest(),
                    Affixes::new(docs![arena, arena.line(), "\\z ->"], arena.nil()).nest(),
                    Affixes::new(
                        docs![arena, arena.line(), "{"],
                        docs![arena, arena.line(), "}"],
                    )
                    .nest(),
                ],
                body: docs![arena, arena.line(), "result"],
            }
        };
        expect_test::expect![[r#"\x y -> \z -> { result }"#]]
            .assert_eq(&mk_doc().format(4).1.pretty(40).to_string());
        expect_test::expect![[r#"
\x y -> \z -> {
    result
}"#]]
        .assert_eq(&mk_doc().format(4).1.pretty(15).to_string());
        expect_test::expect![[r#"
\x y -> \z ->
    {
        result
    }"#]]
        .assert_eq(&mk_doc().format(4).1.pretty(14).to_string());
        expect_test::expect![[r#"
\x y ->
    \z ->
        {
            result
        }"#]]
        .assert_eq(&mk_doc().format(4).1.pretty(12).to_string());
    }
}
