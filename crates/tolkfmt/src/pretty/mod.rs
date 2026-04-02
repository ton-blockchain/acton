//! Simplified vendored variant of pretty.rs:
//! https://github.com/Marwes/pretty.rs
//!
//! Adapted for tolkfmt and extended with `break parent` support via
//! `Doc::BreakParent`.
use std::{
    borrow::Cow,
    fmt, io,
    ops::{Add, AddAssign, Deref},
    rc::Rc,
};

mod render;

pub use self::render::{FmtWrite, IoWrite, Render, RenderAnnotated};

/// The concrete document type. This type is not meant to be used directly. Instead use the static
/// functions on `Doc` or the methods on an `DocAllocator`.
///
/// The `T` parameter is used to abstract over pointers to `Doc`. See `RcDoc` for how it is used.
#[derive(Clone, Default)]
pub enum Doc<'a, T, A = ()>
where
    T: DocPtr<'a, A>,
{
    #[default]
    Nil,
    Append(T, T),
    Group(T),
    FlatAlt(T, T),
    Nest(isize, T),
    Hardline,
    // Stores the length of a string document that is not just ascii
    RenderLen(usize, T),
    OwnedText(Box<str>),
    BorrowedText(&'a str),
    SmallText(SmallText),
    Annotated(A, T),
    Union(T, T),
    Column(T::ColumnFn),
    Nesting(T::ColumnFn),
    BreakParent,
    Fail,
}

pub type SmallText = arrayvec::ArrayString<22>;

fn append_docs<'a, 'd, T, A>(
    mut doc: &'d Doc<'a, T, A>,
    consumer: &mut impl FnMut(&'d Doc<'a, T, A>),
) where
    T: DocPtr<'a, A>,
{
    loop {
        match doc {
            Doc::Append(l, r) => {
                append_docs(l, consumer);
                doc = r;
            }
            _ => break consumer(doc),
        }
    }
}

impl<'a, T, A> fmt::Debug for Doc<'a, T, A>
where
    T: DocPtr<'a, A> + fmt::Debug,
    A: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let is_line = |doc: &Doc<'a, T, A>| match doc {
            Doc::FlatAlt(x, y) => {
                matches!((&**x, &**y), (Doc::Hardline, Doc::BorrowedText(" ")))
            }
            _ => false,
        };
        let is_line_ = |doc: &Doc<'a, T, A>| match doc {
            Doc::FlatAlt(x, y) => {
                matches!((&**x, &**y), (Doc::Hardline, Doc::Nil))
            }
            _ => false,
        };
        match self {
            Doc::Nil => f.debug_tuple("Nil").finish(),
            Doc::Append(..) => {
                let mut f = f.debug_list();
                append_docs(self, &mut |doc| {
                    f.entry(doc);
                });
                f.finish()
            }
            _ if is_line(self) => f.debug_tuple("Line").finish(),
            _ if is_line_(self) => f.debug_tuple("Line_").finish(),
            Doc::FlatAlt(x, y) => f.debug_tuple("FlatAlt").field(x).field(y).finish(),
            Doc::Group(doc) => {
                if is_line(self) {
                    return f.debug_tuple("SoftLine").finish();
                }
                if is_line_(self) {
                    return f.debug_tuple("SoftLine_").finish();
                }
                f.debug_tuple("Group").field(doc).finish()
            }
            Doc::Nest(off, doc) => f.debug_tuple("Nest").field(&off).field(doc).finish(),
            Doc::Hardline => f.debug_tuple("Hardline").finish(),
            Doc::RenderLen(_, d) => d.fmt(f),
            Doc::OwnedText(s) => s.fmt(f),
            Doc::BorrowedText(s) => s.fmt(f),
            Doc::SmallText(s) => s.fmt(f),
            Doc::Annotated(ann, doc) => f.debug_tuple("Annotated").field(ann).field(doc).finish(),
            Doc::Union(l, r) => f.debug_tuple("Union").field(l).field(r).finish(),
            Doc::Column(_) => f.debug_tuple("Column(..)").finish(),
            Doc::Nesting(_) => f.debug_tuple("Nesting(..)").finish(),
            Doc::BreakParent => f.debug_tuple("BreakParent").finish(),
            Doc::Fail => f.debug_tuple("Fail").finish(),
        }
    }
}

#[derive(Clone)]
pub struct RcDoc<'a, A = ()>(Rc<Doc<'a, RcDoc<'a, A>, A>>);

impl<'a, A> fmt::Debug for RcDoc<'a, A>
where
    A: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl<'a, A> RcDoc<'a, A> {
    pub fn new(doc: Doc<'a, RcDoc<'a, A>, A>) -> RcDoc<'a, A> {
        RcDoc(Rc::new(doc))
    }
}

impl<'a, A> From<Doc<'a, Self, A>> for RcDoc<'a, A> {
    fn from(doc: Doc<'a, RcDoc<'a, A>, A>) -> RcDoc<'a, A> {
        RcDoc::new(doc)
    }
}

impl<'a, A> Deref for RcDoc<'a, A> {
    type Target = Doc<'a, RcDoc<'a, A>, A>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

pub struct RcAllocator;

impl<'a, A> DocAllocator<'a, A> for RcAllocator
where
    A: 'a,
{
    type Doc = RcDoc<'a, A>;

    #[inline]
    fn alloc(&'a self, doc: Doc<'a, Self::Doc, A>) -> Self::Doc {
        RcDoc::new(doc)
    }

    fn alloc_column_fn(
        &'a self,
        f: impl Fn(usize) -> Self::Doc + 'a,
    ) -> <Self::Doc as DocPtr<'a, A>>::ColumnFn {
        Rc::new(f)
    }

    fn alloc_width_fn(
        &'a self,
        f: impl Fn(isize) -> Self::Doc + 'a,
    ) -> <Self::Doc as DocPtr<'a, A>>::WidthFn {
        Rc::new(f)
    }
}

impl<'a, A> DocPtr<'a, A> for RcDoc<'a, A> {
    type ColumnFn = Rc<dyn Fn(usize) -> Self + 'a>;
    type WidthFn = Rc<dyn Fn(isize) -> Self + 'a>;
}

impl<'a, A> StaticDoc<'a, A> for RcDoc<'a, A> {
    type Allocator = RcAllocator;
    const ALLOCATOR: &'static Self::Allocator = &RcAllocator;
}

impl<'a, A> RcDoc<'a, A>
where
    A: 'a,
{
    #[inline]
    pub fn nil() -> Self {
        Doc::Nil.into()
    }

    #[inline]
    pub fn hardline() -> Self {
        Doc::Hardline.into()
    }

    #[inline]
    pub fn space() -> Self {
        Doc::BorrowedText(" ").into()
    }

    #[inline]
    pub fn fail() -> Self {
        Doc::Fail.into()
    }

    #[inline]
    pub fn break_parent() -> Self {
        Doc::BreakParent.into()
    }

    #[inline]
    pub fn line() -> Self {
        Self::hardline().flat_alt(Self::space())
    }

    #[inline]
    pub fn line_() -> Self {
        Self::hardline().flat_alt(Self::nil())
    }

    #[inline]
    pub fn as_string<U: fmt::Display>(data: U) -> Self {
        RcAllocator.as_string(data).into_doc()
    }

    #[inline]
    pub fn text<U: Into<Cow<'a, str>>>(data: U) -> Self {
        RcAllocator.text(data).into_doc()
    }

    #[inline]
    pub fn append<D>(self, that: D) -> Self
    where
        D: Pretty<'a, RcAllocator, A>,
    {
        DocBuilder(&RcAllocator, self.into())
            .append(that)
            .into_doc()
    }

    #[inline]
    pub fn concat<I>(docs: I) -> Self
    where
        I: IntoIterator,
        I::Item: Pretty<'a, RcAllocator, A>,
    {
        RcAllocator.concat(docs).into_doc()
    }

    #[inline]
    pub fn intersperse<I, S>(docs: I, separator: S) -> Self
    where
        I: IntoIterator,
        I::Item: Pretty<'a, RcAllocator, A>,
        S: Pretty<'a, RcAllocator, A> + Clone,
        A: Clone,
    {
        RcAllocator.intersperse(docs, separator).into_doc()
    }

    #[inline]
    pub fn flat_alt<D>(self, doc: D) -> Self
    where
        D: Pretty<'a, RcAllocator, A>,
    {
        DocBuilder(&RcAllocator, self.into())
            .flat_alt(doc)
            .into_doc()
    }

    #[inline]
    pub fn group(self) -> Self {
        DocBuilder(&RcAllocator, self.into()).group().into_doc()
    }

    #[inline]
    pub fn nest(self, offset: isize) -> Self {
        DocBuilder(&RcAllocator, self.into())
            .nest(offset)
            .into_doc()
    }

    #[inline]
    pub fn annotate(self, ann: A) -> Self {
        DocBuilder(&RcAllocator, self.into())
            .annotate(ann)
            .into_doc()
    }

    #[inline]
    pub fn union<D>(self, other: D) -> Self
    where
        D: Into<BuildDoc<'a, Self, A>>,
    {
        DocBuilder(&RcAllocator, self.into())
            .union(other)
            .into_doc()
    }

    #[inline]
    pub fn softline() -> Self {
        Self::line().group()
    }

    #[inline]
    pub fn softline_() -> Self {
        Self::line_().group()
    }

    #[inline]
    pub fn column(f: impl Fn(usize) -> Self + 'static) -> Self {
        DocBuilder(
            &RcAllocator,
            Doc::Column(RcAllocator.alloc_column_fn(f)).into(),
        )
        .into_doc()
    }

    #[inline]
    pub fn nesting(f: impl Fn(usize) -> Self + 'static) -> Self {
        DocBuilder(
            &RcAllocator,
            Doc::Nesting(RcAllocator.alloc_column_fn(f)).into(),
        )
        .into_doc()
    }
}

enum FmtText {
    Small(SmallText),
    Large(String),
}

impl fmt::Write for FmtText {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        match self {
            FmtText::Small(buf) => {
                if buf.try_push_str(s).is_err() {
                    let mut new_str = String::with_capacity(buf.len() + s.len());
                    new_str.push_str(buf);
                    new_str.push_str(s);
                    *self = FmtText::Large(new_str);
                }
            }
            FmtText::Large(buf) => buf.push_str(s),
        }
        Ok(())
    }
}

impl<'a, D, A> Doc<'a, D, A>
where
    D: DocPtr<'a, A>,
{
    #[inline]
    pub const fn nil() -> Self {
        Doc::Nil
    }

    #[inline]
    pub const fn hardline() -> Self {
        Doc::Hardline
    }

    #[inline]
    pub const fn space() -> Self {
        Doc::BorrowedText(" ")
    }

    #[inline]
    pub const fn fail() -> Self {
        Doc::Fail
    }

    #[inline]
    pub const fn break_parent() -> Self {
        Doc::BreakParent
    }
}

impl<'a, D, A> Doc<'a, D, A>
where
    D: StaticDoc<'a, A>,
{
    #[inline]
    pub fn line() -> Self {
        Self::hardline().flat_alt(Self::space())
    }

    #[inline]
    pub fn line_() -> Self {
        Self::hardline().flat_alt(Self::nil())
    }
}

impl<'a, D, A> BuildDoc<'a, D, A>
where
    D: DocPtr<'a, A>,
{
    #[inline]
    pub const fn nil() -> Self {
        BuildDoc::Doc(Doc::Nil)
    }

    #[inline]
    pub const fn hardline() -> Self {
        BuildDoc::Doc(Doc::Hardline)
    }

    #[inline]
    pub const fn space() -> Self {
        BuildDoc::Doc(Doc::BorrowedText(" "))
    }

    #[inline]
    pub const fn fail() -> Self {
        BuildDoc::Doc(Doc::Fail)
    }

    #[inline]
    pub const fn break_parent() -> Self {
        BuildDoc::Doc(Doc::BreakParent)
    }
}

impl<'a, D, A> BuildDoc<'a, D, A>
where
    D: StaticDoc<'a, A>,
{
    #[inline]
    pub fn line() -> Self {
        Self::hardline().flat_alt(Self::space())
    }

    #[inline]
    pub fn line_() -> Self {
        Self::hardline().flat_alt(Self::nil())
    }
}

impl<'a, T, A> BuildDoc<'a, T, A>
where
    T: StaticDoc<'a, A>,
{
    /// The text `t.to_string()`.
    ///
    /// The given text must not contain line breaks.
    #[inline]
    pub fn as_string<U: fmt::Display>(data: U) -> Self {
        T::ALLOCATOR.as_string(data).1
    }

    /// The given text, which must not contain line breaks.
    #[inline]
    pub fn text<U: Into<Cow<'a, str>>>(data: U) -> Self {
        T::ALLOCATOR.text(data).1
    }

    fn flat_alt<D>(self, doc: D) -> Self
    where
        D: Pretty<'a, T::Allocator, A>,
    {
        DocBuilder(T::ALLOCATOR, self).flat_alt(doc).1
    }
}

impl<'a, T, A> Doc<'a, T, A>
where
    T: StaticDoc<'a, A>,
{
    /// The text `t.to_string()`.
    ///
    /// The given text must not contain line breaks.
    #[inline]
    pub fn as_string<U: fmt::Display>(data: U) -> Self {
        T::ALLOCATOR.as_string(data).into_plain_doc()
    }

    /// The given text, which must not contain line breaks.
    #[inline]
    pub fn text<U: Into<Cow<'a, str>>>(data: U) -> Self {
        T::ALLOCATOR.text(data).into_plain_doc()
    }

    fn flat_alt<D>(self, doc: D) -> Self
    where
        D: Pretty<'a, T::Allocator, A>,
    {
        DocBuilder(T::ALLOCATOR, self.into())
            .flat_alt(doc)
            .into_plain_doc()
    }
}

pub trait StaticDoc<'a, A>: DocPtr<'a, A>
where
    A: 'a,
{
    type Allocator: DocAllocator<'a, A, Doc = Self> + 'static;
    const ALLOCATOR: &'static Self::Allocator;
}

impl<'a, T, A, S> From<S> for Doc<'a, T, A>
where
    T: StaticDoc<'a, A>,
    S: Into<Cow<'a, str>>,
{
    fn from(s: S) -> Doc<'a, T, A> {
        Doc::text(s)
    }
}

pub struct PrettyFmt<'a, 'd, T, A>
where
    A: 'a,
    T: DocPtr<'a, A> + 'a,
{
    doc: &'d Doc<'a, T, A>,
    width: usize,
}

impl<'a, T, A> fmt::Display for PrettyFmt<'a, '_, T, A>
where
    T: DocPtr<'a, A>,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.doc.render_fmt(self.width, f)
    }
}

impl<'a, T, A> Doc<'a, T, A>
where
    T: DocPtr<'a, A> + 'a,
{
    /// Writes a rendered document to a `std::io::Write` object.
    #[inline]
    pub fn render<W>(&self, width: usize, out: &mut W) -> io::Result<()>
    where
        W: ?Sized + io::Write,
    {
        self.render_raw(width, &mut IoWrite::new(out))
    }

    /// Writes a rendered document to a `std::fmt::Write` object.
    #[inline]
    pub fn render_fmt<W>(&self, width: usize, out: &mut W) -> fmt::Result
    where
        W: ?Sized + fmt::Write,
    {
        self.render_raw(width, &mut FmtWrite::new(out))
    }

    /// Writes a rendered document to a `RenderAnnotated<A>` object.
    #[inline]
    pub fn render_raw<W>(&self, width: usize, out: &mut W) -> Result<(), W::Error>
    where
        W: ?Sized + for<'b> RenderAnnotated<'b, A>,
    {
        render::best(self, width, out)
    }

    /// Returns a value which implements `std::fmt::Display`
    ///
    /// ```
    /// use tolkfmt::pretty::{Doc, RcDoc};
    /// let doc = RcDoc::<()>::group(
    ///     RcDoc::text("hello").append(Doc::line()).append(Doc::text("world"))
    /// );
    /// assert_eq!(format!("{}", doc.pretty(80)), "hello world");
    /// ```
    #[inline]
    pub const fn pretty<'d>(&'d self, width: usize) -> PrettyFmt<'a, 'd, T, A> {
        PrettyFmt { doc: self, width }
    }
}

/// The `DocBuilder` type allows for convenient appending of documents even for arena allocated
/// documents by storing the arena inline.
pub struct DocBuilder<'a, D, A = ()>(pub &'a D, pub BuildDoc<'a, D::Doc, A>)
where
    D: ?Sized + DocAllocator<'a, A>;

impl<'a, D, A, P> Add<P> for DocBuilder<'a, D, A>
where
    D: ?Sized + DocAllocator<'a, A>,
    P: Pretty<'a, D, A>,
{
    type Output = DocBuilder<'a, D, A>;
    fn add(self, other: P) -> Self::Output {
        self.append(other)
    }
}

impl<'a, D, A, P> AddAssign<P> for DocBuilder<'a, D, A>
where
    D: ?Sized + DocAllocator<'a, A>,
    P: Pretty<'a, D, A>,
{
    fn add_assign(&mut self, other: P) {
        *self = DocBuilder(self.0, std::mem::take(&mut self.1)).append(other)
    }
}

impl<'a, D, A> Deref for DocBuilder<'a, D, A>
where
    D: ?Sized + DocAllocator<'a, A>,
{
    type Target = Doc<'a, D::Doc, A>;
    fn deref(&self) -> &Self::Target {
        match &self.1 {
            BuildDoc::DocPtr(d) => d,
            BuildDoc::Doc(d) => d,
        }
    }
}

impl<'a, D, A> fmt::Debug for DocBuilder<'a, D, A>
where
    D: ?Sized + DocAllocator<'a, A>,
    D::Doc: fmt::Debug,
    A: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.1.fmt(f)
    }
}

impl<'a, A, D> Clone for DocBuilder<'a, D, A>
where
    A: Clone,
    D: DocAllocator<'a, A> + 'a,
    D::Doc: Clone,
{
    fn clone(&self) -> Self {
        DocBuilder(self.0, self.1.clone())
    }
}

impl<'a, D, A> From<DocBuilder<'a, D, A>> for BuildDoc<'a, D::Doc, A>
where
    D: ?Sized + DocAllocator<'a, A>,
{
    fn from(val: DocBuilder<'a, D, A>) -> Self {
        val.1
    }
}

pub trait DocPtr<'a, A>: Deref<Target = Doc<'a, Self, A>> + Sized
where
    A: 'a,
{
    type ColumnFn: Deref<Target = dyn Fn(usize) -> Self + 'a> + Clone + 'a;
    type WidthFn: Deref<Target = dyn Fn(isize) -> Self + 'a> + Clone + 'a;
}

/// Trait for types which can be converted to a `Document`
pub trait Pretty<'a, D, A = ()>
where
    A: 'a,
    D: ?Sized + DocAllocator<'a, A>,
{
    /// Converts `self` into a document
    fn pretty(self, allocator: &'a D) -> DocBuilder<'a, D, A>;
}

impl<'a, A> Pretty<'a, RcAllocator, A> for RcDoc<'a, A>
where
    A: 'a,
{
    fn pretty(self, allocator: &'a RcAllocator) -> DocBuilder<'a, RcAllocator, A> {
        DocBuilder(allocator, self.into())
    }
}

impl<'a, D, A> Pretty<'a, D, A> for BuildDoc<'a, D::Doc, A>
where
    A: 'a,
    D: ?Sized + DocAllocator<'a, A>,
{
    fn pretty(self, allocator: &'a D) -> DocBuilder<'a, D, A> {
        DocBuilder(allocator, self)
    }
}

impl<'a, D, A> Pretty<'a, D, A> for Doc<'a, D::Doc, A>
where
    A: 'a,
    D: ?Sized + DocAllocator<'a, A>,
{
    fn pretty(self, allocator: &'a D) -> DocBuilder<'a, D, A> {
        DocBuilder(allocator, self.into())
    }
}

impl<'a, D, A> Pretty<'a, D, A> for DocBuilder<'a, D, A>
where
    A: 'a,
    D: ?Sized + DocAllocator<'a, A>,
{
    fn pretty(self, _: &'a D) -> DocBuilder<'a, D, A> {
        self
    }
}

impl<'a, D, A, T> Pretty<'a, D, A> for Option<T>
where
    A: 'a,
    D: ?Sized + DocAllocator<'a, A>,
    T: Pretty<'a, D, A>,
{
    fn pretty(self, allocator: &'a D) -> DocBuilder<'a, D, A> {
        match self {
            Some(x) => x.pretty(allocator),
            None => allocator.nil(),
        }
    }
}

impl<'a, D, A> Pretty<'a, D, A> for &'a str
where
    A: 'a,
    D: ?Sized + DocAllocator<'a, A>,
{
    fn pretty(self, allocator: &'a D) -> DocBuilder<'a, D, A> {
        allocator.text(self)
    }
}

impl<'a, D, A> Pretty<'a, D, A> for &'a String
where
    A: 'a,
    D: ?Sized + DocAllocator<'a, A>,
{
    fn pretty(self, allocator: &'a D) -> DocBuilder<'a, D, A> {
        self[..].pretty(allocator)
    }
}

impl<'a, D, A> Pretty<'a, D, A> for String
where
    A: 'a,
    D: ?Sized + DocAllocator<'a, A>,
{
    fn pretty(self, allocator: &'a D) -> DocBuilder<'a, D, A> {
        allocator.text(self)
    }
}

impl<'a, D, A, S> Pretty<'a, D, A> for Cow<'a, S>
where
    A: 'a,
    D: ?Sized + DocAllocator<'a, A>,
    S: ?Sized + ToOwned,
    &'a S: Pretty<'a, D, A>,
    S::Owned: Pretty<'a, D, A>,
{
    fn pretty(self, allocator: &'a D) -> DocBuilder<'a, D, A> {
        match self {
            Cow::Borrowed(s) => s.pretty(allocator),
            Cow::Owned(s) => s.pretty(allocator),
        }
    }
}

/// The `DocAllocator` trait abstracts over a type which can allocate (pointers to) `Doc`.
pub trait DocAllocator<'a, A = ()>
where
    A: 'a,
{
    type Doc: DocPtr<'a, A>;

    fn alloc(&'a self, doc: Doc<'a, Self::Doc, A>) -> Self::Doc;

    fn alloc_column_fn(
        &'a self,
        f: impl Fn(usize) -> Self::Doc + 'a,
    ) -> <Self::Doc as DocPtr<'a, A>>::ColumnFn;

    fn alloc_width_fn(
        &'a self,
        f: impl Fn(isize) -> Self::Doc + 'a,
    ) -> <Self::Doc as DocPtr<'a, A>>::WidthFn;

    fn alloc_cow(&'a self, doc: BuildDoc<'a, Self::Doc, A>) -> Self::Doc {
        match doc {
            BuildDoc::DocPtr(d) => d,
            BuildDoc::Doc(d) => self.alloc(d),
        }
    }

    /// Allocate an empty document.
    #[inline]
    fn nil(&'a self) -> DocBuilder<'a, Self, A> {
        DocBuilder(self, Doc::Nil.into())
    }

    /// Fails document rendering immediately.
    ///
    /// Primarily used to abort rendering inside the left side of `Union`
    #[inline]
    fn fail(&'a self) -> DocBuilder<'a, Self, A> {
        DocBuilder(self, Doc::Fail.into())
    }

    /// Break all parent groups.
    #[inline]
    fn break_parent(&'a self) -> DocBuilder<'a, Self, A> {
        DocBuilder(self, Doc::BreakParent.into())
    }

    /// Allocate a single hardline.
    #[inline]
    fn hardline(&'a self) -> DocBuilder<'a, Self, A> {
        DocBuilder(self, Doc::Hardline.into())
    }

    #[inline]
    fn space(&'a self) -> DocBuilder<'a, Self, A> {
        self.text(" ")
    }

    /// A line acts like a `\n` but behaves like `space` if it is grouped on a single line.
    #[inline]
    fn line(&'a self) -> DocBuilder<'a, Self, A> {
        self.hardline().flat_alt(self.space())
    }

    /// Acts like `line` but behaves like `nil` if grouped on a single line
    ///
    /// ```
    /// use tolkfmt::pretty::{Doc, RcDoc};
    ///
    /// let doc = RcDoc::<()>::group(
    ///     RcDoc::text("(")
    ///         .append(
    ///             RcDoc::line_()
    ///                 .append(Doc::text("test"))
    ///                 .append(Doc::line())
    ///                 .append(Doc::text("test"))
    ///                 .nest(2),
    ///         )
    ///         .append(Doc::line_())
    ///         .append(Doc::text(")")),
    /// );
    /// assert_eq!(doc.pretty(5).to_string(), "(\n  test\n  test\n)");
    /// assert_eq!(doc.pretty(100).to_string(), "(test test)");
    /// ```
    #[inline]
    fn line_(&'a self) -> DocBuilder<'a, Self, A> {
        self.hardline().flat_alt(self.nil())
    }

    /// A `softline` acts like `space` if the document fits the page, otherwise like `line`
    #[inline]
    fn softline(&'a self) -> DocBuilder<'a, Self, A> {
        self.line().group()
    }

    /// A `softline_` acts like `nil` if the document fits the page, otherwise like `line_`
    #[inline]
    fn softline_(&'a self) -> DocBuilder<'a, Self, A> {
        self.line_().group()
    }

    /// Allocate a document containing the text `t.to_string()`.
    ///
    /// The given text must not contain line breaks.
    #[inline]
    fn as_string<U: fmt::Display>(&'a self, data: U) -> DocBuilder<'a, Self, A> {
        use std::fmt::Write;
        let mut buf = FmtText::Small(SmallText::new());
        write!(buf, "{}", data).expect("FmtText write must be infallible");
        let doc = match buf {
            FmtText::Small(b) => Doc::SmallText(b),
            FmtText::Large(b) => Doc::OwnedText(b.into()),
        };
        DocBuilder(self, doc.into()).with_utf8_len()
    }

    /// Allocate a document containing the given text.
    ///
    /// The given text must not contain line breaks.
    #[inline]
    fn text<U: Into<Cow<'a, str>>>(&'a self, data: U) -> DocBuilder<'a, Self, A> {
        let data: Cow<_> = data.into();
        let doc = if data.is_empty() {
            Doc::Nil.into()
        } else {
            match data {
                Cow::Owned(t) => Doc::OwnedText(t.into()).into(),
                Cow::Borrowed(t) => Doc::BorrowedText(t).into(),
            }
        };
        DocBuilder(self, doc).with_utf8_len()
    }

    /// Allocate a document concatenating the given documents.
    #[inline]
    fn concat<I>(&'a self, docs: I) -> DocBuilder<'a, Self, A>
    where
        I: IntoIterator,
        I::Item: Pretty<'a, Self, A>,
    {
        docs.into_iter().fold(self.nil(), |a, b| a.append(b))
    }

    /// Allocate a document that intersperses the given separator `S` between the given documents
    /// `[A, B, C, ..., Z]`, yielding `[A, S, B, S, C, S, ..., S, Z]`.
    ///
    /// Compare [the `intersperse` method from the `itertools` crate](https://docs.rs/itertools/0.5.9/itertools/trait.Itertools.html#method.intersperse).
    ///
    /// NOTE: The separator type, `S` may need to be cloned. Consider using cheaply cloneable ptr
    /// like `RcDoc`
    #[inline]
    fn intersperse<I, S>(&'a self, docs: I, separator: S) -> DocBuilder<'a, Self, A>
    where
        I: IntoIterator,
        I::Item: Pretty<'a, Self, A>,
        S: Pretty<'a, Self, A> + Clone,
    {
        let mut result = self.nil();
        let mut iter = docs.into_iter();

        if let Some(first) = iter.next() {
            result = result.append(first);

            for doc in iter {
                result = result.append(separator.clone());
                result = result.append(doc);
            }
        }

        result
    }

    /// Allocate a document that acts differently based on the position and page layout
    ///
    /// ```rust
    /// use tolkfmt::pretty::{DocAllocator, DocBuilder, RcAllocator};
    ///
    /// let alloc = &RcAllocator;
    /// let doc: DocBuilder<'_, RcAllocator, ()> = alloc.text("prefix ")
    ///     .append(alloc.column(|l| {
    ///         alloc.text("| <- column ").append(alloc.as_string(l)).into_doc()
    ///     }));
    /// assert_eq!(doc.1.pretty(80).to_string(), "prefix | <- column 7");
    /// ```
    #[inline]
    fn column(&'a self, f: impl Fn(usize) -> Self::Doc + 'a) -> DocBuilder<'a, Self, A> {
        DocBuilder(self, Doc::Column(self.alloc_column_fn(f)).into())
    }

    /// Allocate a document that acts differently based on the current nesting level
    ///
    /// ```rust
    /// use tolkfmt::pretty::{DocAllocator, DocBuilder, RcAllocator};
    ///
    /// let alloc = &RcAllocator;
    /// let doc: DocBuilder<'_, RcAllocator, ()> = alloc.text("prefix ")
    ///     .append(alloc.nesting(|l| {
    ///         alloc.text("[Nested: ").append(alloc.as_string(l)).append("]").into_doc()
    ///     }).nest(4));
    /// assert_eq!(doc.1.pretty(80).to_string(), "prefix [Nested: 4]");
    /// ```
    #[inline]
    fn nesting(&'a self, f: impl Fn(usize) -> Self::Doc + 'a) -> DocBuilder<'a, Self, A> {
        DocBuilder(self, Doc::Nesting(self.alloc_column_fn(f)).into())
    }

    /// Reflows `text` inserting `softline` in place of any whitespace
    #[inline]
    fn reflow(&'a self, text: &'a str) -> DocBuilder<'a, Self, A>
    where
        Self: Sized,
        Self::Doc: Clone,
        A: Clone,
    {
        self.intersperse(text.split(char::is_whitespace), self.softline())
    }
}

/// Either a `Doc` or a pointer to a `Doc` (`D`)
#[derive(Clone)]
pub enum BuildDoc<'a, D, A>
where
    D: DocPtr<'a, A>,
{
    DocPtr(D),
    Doc(Doc<'a, D, A>),
}

impl<'a, D, A> Default for BuildDoc<'a, D, A>
where
    D: DocPtr<'a, A>,
{
    fn default() -> Self {
        Self::Doc(Doc::default())
    }
}

impl<'a, D, A> fmt::Debug for BuildDoc<'a, D, A>
where
    D: DocPtr<'a, A> + fmt::Debug,
    A: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        (**self).fmt(f)
    }
}

impl<'a, D, A> Deref for BuildDoc<'a, D, A>
where
    D: DocPtr<'a, A>,
{
    type Target = Doc<'a, D, A>;
    fn deref(&self) -> &Self::Target {
        match self {
            BuildDoc::DocPtr(d) => d,
            BuildDoc::Doc(d) => d,
        }
    }
}

impl<'a, A> From<RcDoc<'a, A>> for BuildDoc<'a, RcDoc<'a, A>, A> {
    fn from(s: RcDoc<'a, A>) -> Self {
        BuildDoc::DocPtr(s)
    }
}

impl<'a, T, A> From<Doc<'a, T, A>> for BuildDoc<'a, T, A>
where
    T: DocPtr<'a, A>,
{
    fn from(s: Doc<'a, T, A>) -> Self {
        BuildDoc::Doc(s)
    }
}

impl<'a, T, A> From<String> for BuildDoc<'a, T, A>
where
    T: StaticDoc<'a, A>,
{
    fn from(s: String) -> Self {
        BuildDoc::Doc(Doc::text(s))
    }
}

impl<'a, T, A> From<&'a str> for BuildDoc<'a, T, A>
where
    T: StaticDoc<'a, A>,
{
    fn from(s: &'a str) -> Self {
        BuildDoc::Doc(Doc::text(s))
    }
}

impl<'a, T, A> From<&'a String> for BuildDoc<'a, T, A>
where
    T: StaticDoc<'a, A>,
{
    fn from(s: &'a String) -> Self {
        BuildDoc::Doc(Doc::text(s))
    }
}

impl<'a, T, A, S> From<Option<S>> for BuildDoc<'a, T, A>
where
    T: DocPtr<'a, A>,
    S: Into<BuildDoc<'a, T, A>>,
{
    fn from(s: Option<S>) -> Self {
        match s {
            Some(s) => s.into(),
            None => BuildDoc::Doc(Doc::Nil),
        }
    }
}

/// Concatenates a number of documents (or values that can be converted into a document via the
/// `Pretty` trait, like `&str`)
///
/// ```
/// use tolkfmt::docs;
/// use tolkfmt::pretty::{DocAllocator, DocBuilder, RcAllocator};
/// let alloc = &RcAllocator;
/// let doc: DocBuilder<'_, RcAllocator, ()> = docs![
///     alloc,
///     "let",
///     alloc.softline(),
///     "x",
///     alloc.softline(),
///     "=",
///     alloc.softline(),
///     Some("123"),
/// ];
/// assert_eq!(doc.1.pretty(80).to_string(), "let x = 123");
/// ```
#[macro_export]
macro_rules! docs {
    ($alloc: expr, $first: expr $(,)?) => {
        $crate::pretty::Pretty::pretty($first, $alloc)
    };
    ($alloc: expr, $first: expr $(, $rest: expr)+ $(,)?) => {{
        let mut doc = $crate::pretty::Pretty::pretty($first, $alloc);
        $(
            doc = doc.append($rest);
        )*
        doc
    }}
}

impl<'a, D, A> DocBuilder<'a, D, A>
where
    A: 'a,
    D: ?Sized + DocAllocator<'a, A>,
{
    fn with_utf8_len(self) -> Self {
        let s = match &*self {
            Doc::OwnedText(s) => &s[..],
            Doc::BorrowedText(s) => s,
            Doc::SmallText(s) => s,
            _ => return self,
        };

        if s.is_ascii() {
            self
        } else {
            let display_width = unicode_width::UnicodeWidthStr::width(s);

            let DocBuilder(allocator, _) = self;
            DocBuilder(
                allocator,
                Doc::RenderLen(display_width, self.into_doc()).into(),
            )
        }
    }

    /// Append the given document after this document.
    #[inline]
    pub fn append<E>(self, that: E) -> DocBuilder<'a, D, A>
    where
        E: Pretty<'a, D, A>,
    {
        let DocBuilder(allocator, _) = self;
        let that = that.pretty(allocator);
        match (&*self, &*that) {
            (Doc::Nil, _) => that,
            (_, Doc::Nil) => self,
            _ => DocBuilder(
                allocator,
                Doc::Append(
                    allocator.alloc_cow(self.into()),
                    allocator.alloc_cow(that.into()),
                )
                .into(),
            ),
        }
    }

    /// Acts as `self` when laid out on multiple lines and acts as `that` when laid out on a single line.
    ///
    /// ```
    /// use tolkfmt::pretty::{DocAllocator, DocBuilder, RcAllocator};
    ///
    /// let alloc = &RcAllocator;
    /// let body: DocBuilder<'_, RcAllocator, ()> = alloc.line().append("x");
    /// let doc = alloc.text("let")
    ///     .append(alloc.line())
    ///     .append("x")
    ///     .group()
    ///     .append(
    ///         body.clone()
    ///             .flat_alt(
    ///                 alloc.line()
    ///                     .append("in")
    ///                     .append(body)
    ///             )
    ///     )
    ///     .group();
    ///
    /// assert_eq!(doc.1.pretty(100).to_string(), "let x in x");
    /// assert_eq!(doc.1.pretty(8).to_string(), "let x\nx");
    /// ```
    #[inline]
    pub fn flat_alt<E>(self, that: E) -> DocBuilder<'a, D, A>
    where
        E: Pretty<'a, D, A>,
    {
        let DocBuilder(allocator, this) = self;
        let that = that.pretty(allocator);
        DocBuilder(
            allocator,
            Doc::FlatAlt(allocator.alloc_cow(this), allocator.alloc_cow(that.into())).into(),
        )
    }

    /// Mark this document as a group.
    ///
    /// Groups are laid out on a single line if possible.  Within a group, all basic documents with
    /// several possible layouts are assigned the same layout, that is, they are all laid out
    /// horizontally and combined into a one single line, or they are each laid out on their own
    /// line.
    #[inline]
    pub fn group(self) -> DocBuilder<'a, D, A> {
        match *self.1 {
            Doc::Group(_)
            | Doc::OwnedText(_)
            | Doc::BorrowedText(_)
            | Doc::SmallText(_)
            | Doc::Nil => self,
            _ => {
                let DocBuilder(allocator, this) = self;
                DocBuilder(allocator, Doc::Group(allocator.alloc_cow(this)).into())
            }
        }
    }

    /// Increase the indentation level of this document.
    #[inline]
    pub fn nest(self, offset: isize) -> DocBuilder<'a, D, A> {
        if matches!(&*self.1, Doc::Nil) {
            return self;
        }
        if offset == 0 {
            return self;
        }
        let DocBuilder(allocator, this) = self;
        DocBuilder(
            allocator,
            Doc::Nest(offset, allocator.alloc_cow(this)).into(),
        )
    }

    #[inline]
    pub fn annotate(self, ann: A) -> DocBuilder<'a, D, A> {
        let DocBuilder(allocator, this) = self;
        DocBuilder(
            allocator,
            Doc::Annotated(ann, allocator.alloc_cow(this)).into(),
        )
    }

    #[inline]
    pub fn union<E>(self, other: E) -> DocBuilder<'a, D, A>
    where
        E: Into<BuildDoc<'a, D::Doc, A>>,
    {
        let DocBuilder(allocator, this) = self;
        let other = other.into();
        let doc = Doc::Union(allocator.alloc_cow(this), allocator.alloc_cow(other));
        DocBuilder(allocator, doc.into())
    }

    /// Lays out `self` so with the nesting level set to the current column
    ///
    /// NOTE: The doc pointer type, `D` may need to be cloned. Consider using cheaply cloneable ptr
    /// like `RcDoc`
    ///
    /// ```rust
    /// use tolkfmt::docs;
    /// use tolkfmt::pretty::{DocAllocator, DocBuilder, RcAllocator};
    ///
    /// let arena = &RcAllocator;
    /// let doc: DocBuilder<'_, RcAllocator, ()> = docs![
    ///     arena,
    ///     "lorem",
    ///     " ",
    ///     arena.intersperse(["ipsum", "dolor"].iter().cloned(), arena.line_()).align(),
    ///     arena.hardline(),
    ///     "next",
    /// ];
    /// assert_eq!(doc.1.pretty(80).to_string(), "lorem ipsum\n      dolor\nnext");
    /// ```
    #[inline]
    pub fn align(self) -> DocBuilder<'a, D, A>
    where
        DocBuilder<'a, D, A>: Clone,
    {
        let allocator = self.0;
        allocator.column(move |col| {
            let self_ = self.clone();
            allocator
                .nesting(move |nest| self_.clone().nest(col as isize - nest as isize).into_doc())
                .into_doc()
        })
    }

    /// Lays out `self` with a nesting level set to the current level plus `adjust`.
    ///
    /// NOTE: The doc pointer type, `D` may need to be cloned. Consider using cheaply cloneable ptr
    /// like `RcDoc`
    ///
    /// ```rust
    /// use tolkfmt::pretty::{DocAllocator, DocBuilder, RcAllocator};
    ///
    /// let arena = &RcAllocator;
    /// let doc: DocBuilder<'_, RcAllocator, ()> = arena.text("prefix").append(arena.text(" "))
    ///     .append(arena.reflow("Indenting these words with nest").hang(4));
    /// assert_eq!(
    ///     doc.1.pretty(24).to_string(),
    ///     "prefix Indenting these\n           words with\n           nest",
    /// );
    /// ```
    #[inline]
    pub fn hang(self, adjust: isize) -> DocBuilder<'a, D, A>
    where
        DocBuilder<'a, D, A>: Clone,
    {
        self.nest(adjust).align()
    }

    /// Indents `self` by `adjust` spaces from the current cursor position
    ///
    /// NOTE: The doc pointer type, `D` may need to be cloned. Consider using cheaply cloneable ptr
    /// like `RcDoc`
    ///
    /// ```rust
    /// use tolkfmt::pretty::{DocAllocator, DocBuilder, RcAllocator};
    ///
    /// let arena = &RcAllocator;
    /// let doc: DocBuilder<'_, RcAllocator, ()> = arena.text("prefix").append(arena.text(" "))
    ///     .append(arena.reflow("The indent function indents these words!").indent(4));
    /// assert_eq!(
    ///     doc.1.pretty(24).to_string(),
    /// "
    /// prefix     The indent
    ///            function
    ///            indents these
    ///            words!".trim_start(),
    /// );
    /// ```
    #[inline]
    pub fn indent(self, adjust: usize) -> DocBuilder<'a, D, A>
    where
        DocBuilder<'a, D, A>: Clone,
    {
        let spaces = {
            use crate::pretty::render::SPACES;
            let DocBuilder(allocator, _) = self;
            let mut doc = allocator.nil();
            let mut remaining = adjust;
            while remaining != 0 {
                let i = SPACES.len().min(remaining);
                remaining -= i;
                doc = doc.append(allocator.text(&SPACES[..i]))
            }
            doc
        };
        let adjust = isize::try_from(adjust).expect("indent width exceeds isize");
        spaces.append(self).hang(adjust)
    }

    /// Lays out `self` and provides the column width of it available to `f`
    ///
    /// NOTE: The doc pointer type, `D` may need to be cloned. Consider using cheaply cloneable ptr
    /// like `RcDoc`
    ///
    /// ```rust
    /// use tolkfmt::pretty::{DocAllocator, DocBuilder, RcAllocator};
    ///
    /// let arena = &RcAllocator;
    /// let doc: DocBuilder<'_, RcAllocator, ()> = arena.text("prefix ")
    ///     .append(arena.column(|l| {
    ///         arena.text("| <- column ").append(arena.as_string(l)).into_doc()
    ///     }));
    /// assert_eq!(doc.1.pretty(80).to_string(), "prefix | <- column 7");
    /// ```
    #[inline]
    pub fn width(self, f: impl Fn(isize) -> D::Doc + 'a) -> DocBuilder<'a, D, A>
    where
        BuildDoc<'a, D::Doc, A>: Clone,
    {
        let DocBuilder(allocator, this) = self;
        let f = allocator.alloc_width_fn(f);
        allocator.column(move |start| {
            let f = f.clone();

            DocBuilder(allocator, this.clone())
                .append(allocator.column(move |end| f(end as isize - start as isize)))
                .into_doc()
        })
    }

    /// Puts `self` between `before` and `after`
    #[inline]
    pub fn enclose<E, F>(self, before: E, after: F) -> DocBuilder<'a, D, A>
    where
        E: Pretty<'a, D, A>,
        F: Pretty<'a, D, A>,
    {
        let DocBuilder(allocator, _) = self;
        DocBuilder(allocator, before.pretty(allocator).1)
            .append(self)
            .append(after)
    }

    pub fn single_quotes(self) -> DocBuilder<'a, D, A> {
        self.enclose("'", "'")
    }

    pub fn double_quotes(self) -> DocBuilder<'a, D, A> {
        self.enclose("\"", "\"")
    }
    pub fn parens(self) -> DocBuilder<'a, D, A> {
        self.enclose("(", ")")
    }

    pub fn angles(self) -> DocBuilder<'a, D, A> {
        self.enclose("<", ">")
    }
    pub fn braces(self) -> DocBuilder<'a, D, A> {
        self.enclose("{", "}")
    }

    pub fn brackets(self) -> DocBuilder<'a, D, A> {
        self.enclose("[", "]")
    }

    pub fn into_doc(self) -> D::Doc {
        match self.1 {
            BuildDoc::DocPtr(d) => d,
            BuildDoc::Doc(d) => self.0.alloc(d),
        }
    }

    fn into_plain_doc(self) -> Doc<'a, D::Doc, A> {
        match self.1 {
            BuildDoc::DocPtr(_) => unreachable!(),
            BuildDoc::Doc(d) => d,
        }
    }
}

impl<'a, D, A> DocAllocator<'a, A> for &'a D
where
    D: ?Sized + DocAllocator<'a, A>,
    A: 'a,
{
    type Doc = D::Doc;

    #[inline]
    fn alloc(&'a self, doc: Doc<'a, Self::Doc, A>) -> Self::Doc {
        (**self).alloc(doc)
    }

    fn alloc_column_fn(
        &'a self,
        f: impl Fn(usize) -> Self::Doc + 'a,
    ) -> <Self::Doc as DocPtr<'a, A>>::ColumnFn {
        (**self).alloc_column_fn(f)
    }

    fn alloc_width_fn(
        &'a self,
        f: impl Fn(isize) -> Self::Doc + 'a,
    ) -> <Self::Doc as DocPtr<'a, A>>::WidthFn {
        (**self).alloc_width_fn(f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::size_of;

    macro_rules! chain {
        ($first: expr $(, $rest: expr)* $(,)?) => {{
            #[allow(unused_mut)]
            let mut doc = DocBuilder(&RcAllocator, $first.into());
            $(
                doc = doc.append($rest);
            )*
            doc.into_doc()
        }}
    }

    #[cfg(target_pointer_width = "64")]
    #[test]
    fn doc_size() {
        // Safeguard against accidentally growing Doc
        assert_eq!(8 * 4, size_of::<Doc<RcDoc>>());
    }

    macro_rules! test {
        ($size:expr, $actual:expr, $expected:expr) => {
            let mut s = String::new();
            $actual.render_fmt($size, &mut s).unwrap();
            assert_eq!(s, $expected);
        };
        ($actual:expr, $expected:expr) => {
            test!(70, $actual, $expected)
        };
    }

    #[test]
    fn rc_doc_inference() {
        let doc: RcDoc<()> = RcDoc::group(
            RcDoc::text("test")
                .append(RcDoc::line())
                .append(RcDoc::text("test")),
        );

        test!(doc, "test test");
    }

    #[test]
    fn newline_in_text() {
        let doc: RcDoc<()> = RcDoc::group(
            RcDoc::text("test").append(
                RcDoc::line()
                    .append(RcDoc::text("\"test\n     test\""))
                    .nest(4),
            ),
        );

        test!(5, doc, "test\n    \"test\n     test\"");
    }

    #[test]
    fn forced_newline() {
        let doc: RcDoc<()> = RcDoc::group(
            RcDoc::text("test")
                .append(RcDoc::hardline())
                .append(RcDoc::text("test")),
        );

        test!(doc, "test\ntest");
    }

    #[test]
    fn space_do_not_reset_pos() {
        let doc: RcDoc<()> = RcDoc::group(RcDoc::text("test").append(RcDoc::line()))
            .append(RcDoc::text("test"))
            .append(RcDoc::group(RcDoc::line()).append(RcDoc::text("test")));

        test!(9, doc, "test test\ntest");
    }

    // Tests that the `RcDoc::hardline()` does not cause the rest of document to think that it fits on
    // a single line but instead breaks on the `RcDoc::line()` to fit with 6 columns
    #[test]
    fn newline_does_not_cause_next_line_to_be_to_long() {
        let doc: RcDoc<()> = RcDoc::group(
            RcDoc::text("test").append(RcDoc::hardline()).append(
                RcDoc::text("test")
                    .append(RcDoc::line())
                    .append(RcDoc::text("test")),
            ),
        );

        test!(6, doc, "test\ntest\ntest");
    }

    #[test]
    fn newline_after_group_does_not_affect_it() {
        let alloc = &RcAllocator;
        let doc: DocBuilder<'_, RcAllocator, ()> =
            alloc.text("x").append(alloc.line()).append("y").group();

        test!(100, doc.append(alloc.hardline()).1, "x y\n");
    }

    #[test]
    fn block() {
        let doc: RcDoc<()> = RcDoc::group(
            RcDoc::text("{")
                .append(
                    RcDoc::line()
                        .append(RcDoc::text("test"))
                        .append(RcDoc::line())
                        .append(RcDoc::text("test"))
                        .nest(2),
                )
                .append(RcDoc::line())
                .append(RcDoc::text("}")),
        );

        test!(5, doc, "{\n  test\n  test\n}");
    }

    #[test]
    fn block_with_hardline() {
        let doc: RcDoc<()> = RcDoc::group(
            RcDoc::text("{")
                .append(
                    RcDoc::line()
                        .append(RcDoc::text("test"))
                        .append(RcDoc::hardline())
                        .append(RcDoc::text("test"))
                        .nest(2),
                )
                .append(RcDoc::line())
                .append(RcDoc::text("}")),
        );

        test!(10, doc, "{\n  test\n  test\n}");
    }

    #[test]
    fn block_with_hardline_negative_nest() {
        let doc: RcDoc<()> = RcDoc::group(
            RcDoc::text("{")
                .append(
                    RcDoc::line()
                        .append(RcDoc::text("test"))
                        .append(RcDoc::hardline())
                        .append(RcDoc::text("test"))
                        .nest(-2),
                )
                .append(RcDoc::line())
                .append(RcDoc::text("}")),
        );

        test!(10, doc, "{\ntest\ntest\n}");
    }

    #[test]
    fn line_comment() {
        let doc: RcDoc<()> = RcDoc::group(
            RcDoc::text("{")
                .append(
                    RcDoc::line()
                        .append(RcDoc::text("test"))
                        .append(RcDoc::line())
                        .append(RcDoc::text("// a").append(RcDoc::hardline()))
                        .append(RcDoc::text("test"))
                        .nest(2),
                )
                .append(RcDoc::line())
                .append(RcDoc::text("}")),
        );

        test!(14, doc, "{\n  test\n  // a\n  test\n}");
    }

    #[test]
    fn annotation_no_panic() {
        let doc: RcDoc<()> = RcDoc::group(
            RcDoc::text("test")
                .annotate(())
                .append(RcDoc::hardline())
                .annotate(())
                .append(RcDoc::text("test")),
        );

        test!(doc, "test\ntest");
    }

    fn nest_on_line(doc: RcDoc<'static, ()>) -> RcDoc<'static, ()> {
        RcDoc::softline().append(RcDoc::nesting(move |n| {
            let doc = doc.clone();
            RcDoc::column(move |c| {
                if n == c {
                    RcDoc::text("  ").append(doc.clone()).nest(2)
                } else {
                    doc.clone()
                }
            })
        }))
    }

    #[test]
    fn hang_lambda1() {
        let doc = chain![
            chain!["let", RcDoc::line(), "x", RcDoc::line(), "="].group(),
            nest_on_line(chain!["\\y ->", chain![RcDoc::line(), "y"].nest(2).group()]),
        ]
        .group();

        test!(doc, "let x = \\y -> y");
        test!(
            8,
            doc,
            r"let x =
  \y ->
    y"
        );
        test!(
            14,
            doc,
            r"let x = \y ->
  y"
        );
    }

    #[test]
    fn hang_comment() {
        let body = chain!["y"].nest(2).group();
        let doc = chain![
            chain!["let", RcDoc::line(), "x", RcDoc::line(), "="].group(),
            nest_on_line(chain![
                "\\y ->",
                nest_on_line(chain!["// abc", RcDoc::hardline(), body])
            ]),
        ]
        .group();

        test!(8, doc, "let x =\n  \\y ->\n    // abc\n    y");
        test!(14, doc, "let x = \\y ->\n  // abc\n  y");
    }

    #[test]
    fn union() {
        let doc = chain![
            chain!["let", RcDoc::line(), "x", RcDoc::line(), "="].group(),
            nest_on_line(chain![
                "(",
                chain![
                    RcDoc::line_(),
                    chain!["x", ","].group(),
                    RcDoc::line(),
                    chain!["1234567890", ","].group()
                ]
                .nest(2)
                .group(),
                RcDoc::line_().append(")"),
            ])
        ]
        .group();

        test!(doc, "let x = (x, 1234567890,)");
        test!(8, doc, "let x =\n  (\n    x,\n    1234567890,\n  )");
        test!(14, doc, "let x = (\n  x,\n  1234567890,\n)");
    }

    fn hang2(
        from: RcDoc<'static, ()>,
        body_whitespace: RcDoc<'static, ()>,
        body: RcDoc<'static, ()>,
        trailer: RcDoc<'static, ()>,
    ) -> RcDoc<'static, ()> {
        let body1 = body_whitespace
            .append(body.clone())
            .nest(2)
            .group()
            .append(trailer.clone());
        let body2 = RcDoc::hardline()
            .append(body.clone())
            .nest(2)
            .group()
            .append(trailer.clone());

        let single = from.clone().append(body1.clone()).group();

        let hang = from.clone().append(body2).group();

        let break_all = from.append(body1).group().nest(2);

        RcDoc::group(single.union(hang.union(break_all)))
    }

    #[test]
    fn hang_lambda2() {
        let from = chain![
            chain!["let", RcDoc::line(), "x", RcDoc::line(), "="].group(),
            RcDoc::line(),
            "\\y ->",
        ]
        .group();

        let body = chain!["y"].group();

        let trailer = RcDoc::nil();

        let doc = hang2(from, RcDoc::line(), body, trailer);
        eprintln!("{:#?}", doc);

        test!(doc, "let x = \\y -> y");
        test!(14, doc, "let x = \\y ->\n  y");
    }

    #[test]
    fn union2() {
        let from = chain![
            chain!["let", RcDoc::line(), "x", RcDoc::line(), "="].group(),
            RcDoc::line(),
            "(",
        ]
        .group();

        let body = chain![
            chain!["x", ","].group(),
            RcDoc::line(),
            chain!["1234567890", ","].group()
        ]
        .group();

        let trailer = RcDoc::line_().append(")");

        let doc = hang2(from, RcDoc::line_(), body, trailer);

        test!(doc, "let x = (x, 1234567890,)");
        test!(14, doc, "let x = (\n  x,\n  1234567890,\n)");
    }

    #[test]
    fn usize_max_value() {
        let doc: RcDoc<()> = RcDoc::group(
            RcDoc::text("test")
                .append(RcDoc::line())
                .append(RcDoc::text("test")),
        );

        test!(usize::MAX, doc, "test test");
    }

    #[test]
    fn fail() {
        let fail_break: RcDoc<()> = RcDoc::fail().flat_alt(Doc::nil());

        let doc = fail_break.append(Doc::text("12345")).group().union("abc");

        test!(5, doc, "12345");
        test!(4, doc, "abc");
    }

    #[test]
    fn break_parent_forces_group_break() {
        let doc: RcDoc<()> = RcDoc::group(
            RcDoc::text("a")
                .append(RcDoc::line())
                .append(RcDoc::break_parent())
                .append(RcDoc::text("b")),
        );

        test!(doc, "a\nb");
    }

    struct TestWriter<W> {
        upstream: W,
    }

    impl<W> TestWriter<W> {
        fn new(upstream: W) -> Self {
            Self { upstream }
        }
    }

    impl<W> Render for TestWriter<W>
    where
        W: Render,
    {
        type Error = W::Error;

        fn write_str(&mut self, s: &str) -> Result<usize, W::Error> {
            self.upstream.write_str(s)
        }

        fn write_str_all(&mut self, s: &str) -> Result<(), W::Error> {
            self.upstream.write_str_all(s)
        }

        fn fail_doc(&self) -> Self::Error {
            self.upstream.fail_doc()
        }
    }

    impl<W> RenderAnnotated<'_, ()> for TestWriter<W>
    where
        W: Render,
    {
        fn push_annotation(&mut self, _: &()) -> Result<(), Self::Error> {
            self.upstream.write_str_all("[")
        }

        fn pop_annotation(&mut self) -> Result<(), Self::Error> {
            self.upstream.write_str_all("]")
        }
    }

    #[test]
    fn annotations() {
        let actual = RcDoc::text("abc").annotate(()).annotate(());
        let mut s = String::new();
        actual
            .render_raw(70, &mut TestWriter::new(FmtWrite::new(&mut s)))
            .unwrap();
        assert_eq!(s, "[[abc]]");
    }

    #[test]
    fn non_ascii_is_not_byte_length() {
        let doc: RcDoc<()> = RcDoc::group(
            RcDoc::text("ÅÄÖ")
                .append(RcDoc::line())
                .append(RcDoc::text("test")),
        );

        test!(8, doc, "ÅÄÖ test");
    }

    #[test]
    fn cjk_display_width() {
        let alloc = &RcAllocator;
        let doc: RcDoc<()> = alloc
            .text("你好")
            .append(alloc.line().append(alloc.text("abc")).align())
            .into_doc();

        test!(doc, "你好\n    abc");
    }

    #[test]
    fn pretty_cow() {
        let doc: RcDoc<()> = docs![
            &RcAllocator,
            Cow::<str>::Borrowed("abc"),
            RcDoc::line(),
            Cow::<str>::Owned("123".to_string()),
        ]
        .group()
        .into_doc();

        test!(8, doc, "abc 123");
    }

    #[test]
    fn stress_append_left_assoc() {
        let alloc = &RcAllocator;
        let mut doc: DocBuilder<'_, RcAllocator, char> = alloc.nil();
        for _ in 0..1_000 {
            doc = doc.append("a");
        }
        let mut s = String::new();
        doc.render_fmt(80, &mut s).unwrap();
    }

    #[test]
    fn stress_append_right_assoc() {
        let alloc = &RcAllocator;
        let mut doc: RcDoc<'_, char> = alloc.nil().into_doc();
        for _ in 0..1_000 {
            doc = alloc.text("a").append(doc).into_doc();
        }
        let mut s = String::new();
        doc.render_fmt(80, &mut s).unwrap();
    }
}
