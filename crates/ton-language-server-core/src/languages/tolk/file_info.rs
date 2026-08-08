use crate::{Range, TextIndex};
use tolk_resolver::{FileInfo, Span};

/// Language-server conversions for resolver file metadata.
///
/// These methods live in an extension trait because [`FileInfo`] belongs to
/// the protocol-independent resolver crate, while [`Range`] and [`TextIndex`]
/// are language-server types. Callers still use ordinary method syntax without
/// introducing an LSP dependency into the resolver.
pub(super) trait FileInfoExt {
    /// Converts a resolver byte span in this file to an LSP range.
    ///
    /// Resolver spans use UTF-8 byte offsets, while LSP positions use
    /// zero-based lines and UTF-16 code units. Keeping the conversion on the
    /// file makes the source text used for both offsets explicit and prevents
    /// callers from rebuilding a [`TextIndex`] independently.
    fn range_for_span(&self, span: Span) -> Range;
}

impl FileInfoExt for FileInfo {
    fn range_for_span(&self, span: Span) -> Range {
        let source = self.source().source.as_ref();
        TextIndex::new(source).range_for_offsets(source, span.start(), span.end())
    }
}
