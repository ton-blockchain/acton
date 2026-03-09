use std::{cmp, fmt, io};

use termcolor::{ColorSpec, WriteColor};

use crate::pretty::{Doc, DocPtr};

/// Trait representing the operations necessary to render a document
pub trait Render {
    type Error;

    fn write_str(&mut self, s: &str) -> Result<usize, Self::Error>;

    fn write_str_all(&mut self, mut s: &str) -> Result<(), Self::Error> {
        while !s.is_empty() {
            let count = self.write_str(s)?;
            s = &s[count..];
        }
        Ok(())
    }

    fn fail_doc(&self) -> Self::Error;
}

/// Writes to something implementing `std::io::Write`
pub struct IoWrite<W> {
    upstream: W,
}

impl<W> IoWrite<W> {
    pub const fn new(upstream: W) -> IoWrite<W> {
        IoWrite { upstream }
    }
}

impl<W> Render for IoWrite<W>
where
    W: io::Write,
{
    type Error = io::Error;

    fn write_str(&mut self, s: &str) -> io::Result<usize> {
        self.upstream.write(s.as_bytes())
    }

    fn write_str_all(&mut self, s: &str) -> io::Result<()> {
        self.upstream.write_all(s.as_bytes())
    }

    fn fail_doc(&self) -> Self::Error {
        io::Error::other("Document failed to render")
    }
}

/// Writes to something implementing `std::fmt::Write`
pub struct FmtWrite<W> {
    upstream: W,
}

impl<W> FmtWrite<W> {
    pub const fn new(upstream: W) -> FmtWrite<W> {
        FmtWrite { upstream }
    }
}

impl<W> Render for FmtWrite<W>
where
    W: fmt::Write,
{
    type Error = fmt::Error;

    fn write_str(&mut self, s: &str) -> Result<usize, fmt::Error> {
        self.write_str_all(s).map(|_| s.len())
    }

    fn write_str_all(&mut self, s: &str) -> fmt::Result {
        self.upstream.write_str(s)
    }

    fn fail_doc(&self) -> Self::Error {
        fmt::Error
    }
}

/// Trait representing the operations necessary to write an annotated document.
pub trait RenderAnnotated<'a, A>: Render {
    fn push_annotation(&mut self, annotation: &'a A) -> Result<(), Self::Error>;
    fn pop_annotation(&mut self) -> Result<(), Self::Error>;
}

impl<A, W> RenderAnnotated<'_, A> for IoWrite<W>
where
    W: io::Write,
{
    fn push_annotation(&mut self, _: &A) -> Result<(), Self::Error> {
        Ok(())
    }

    fn pop_annotation(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}

impl<A, W> RenderAnnotated<'_, A> for FmtWrite<W>
where
    W: fmt::Write,
{
    fn push_annotation(&mut self, _: &A) -> Result<(), Self::Error> {
        Ok(())
    }

    fn pop_annotation(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}

pub struct TermColored<W> {
    color_stack: Vec<ColorSpec>,
    upstream: W,
}

impl<W> TermColored<W> {
    pub const fn new(upstream: W) -> TermColored<W> {
        TermColored {
            color_stack: Vec::new(),
            upstream,
        }
    }
}

impl<W> Render for TermColored<W>
where
    W: io::Write,
{
    type Error = io::Error;

    fn write_str(&mut self, s: &str) -> io::Result<usize> {
        self.upstream.write(s.as_bytes())
    }

    fn write_str_all(&mut self, s: &str) -> io::Result<()> {
        self.upstream.write_all(s.as_bytes())
    }

    fn fail_doc(&self) -> Self::Error {
        io::Error::other("Document failed to render")
    }
}

impl<W> RenderAnnotated<'_, ColorSpec> for TermColored<W>
where
    W: WriteColor,
{
    fn push_annotation(&mut self, color: &ColorSpec) -> Result<(), Self::Error> {
        self.color_stack.push(color.clone());
        self.upstream.set_color(color)
    }

    fn pop_annotation(&mut self) -> Result<(), Self::Error> {
        self.color_stack.pop();
        match self.color_stack.last() {
            Some(previous) => self.upstream.set_color(previous),
            None => self.upstream.reset(),
        }
    }
}

enum Annotation<'a, A> {
    Push(&'a A),
    Pop,
}

struct BufferWrite<'a, A> {
    buffer: String,
    annotations: Vec<(usize, Annotation<'a, A>)>,
}

impl<'a, A> BufferWrite<'a, A> {
    const fn new() -> Self {
        BufferWrite {
            buffer: String::new(),
            annotations: Vec::new(),
        }
    }

    fn render<W>(&self, render: &mut W) -> Result<(), W::Error>
    where
        W: RenderAnnotated<'a, A> + ?Sized,
    {
        let mut start = 0;
        for (end, annotation) in &self.annotations {
            let s = &self.buffer[start..*end];
            if !s.is_empty() {
                render.write_str_all(s)?;
            }
            start = *end;
            match annotation {
                Annotation::Push(a) => render.push_annotation(a)?,
                Annotation::Pop => render.pop_annotation()?,
            }
        }
        let s = &self.buffer[start..];
        if !s.is_empty() {
            render.write_str_all(s)?;
        }
        Ok(())
    }
}

impl<A> Render for BufferWrite<'_, A> {
    type Error = ();

    fn write_str(&mut self, s: &str) -> Result<usize, Self::Error> {
        self.buffer.push_str(s);
        Ok(s.len())
    }

    fn write_str_all(&mut self, s: &str) -> Result<(), Self::Error> {
        self.buffer.push_str(s);
        Ok(())
    }

    fn fail_doc(&self) -> Self::Error {}
}

impl<'a, A> RenderAnnotated<'a, A> for BufferWrite<'a, A> {
    fn push_annotation(&mut self, a: &'a A) -> Result<(), Self::Error> {
        self.annotations
            .push((self.buffer.len(), Annotation::Push(a)));
        Ok(())
    }

    fn pop_annotation(&mut self) -> Result<(), Self::Error> {
        self.annotations.push((self.buffer.len(), Annotation::Pop));
        Ok(())
    }
}

macro_rules! make_spaces {
    () => { "" };
    ($s: tt $($t: tt)*) => { concat!("          ", make_spaces!($($t)*)) };
}

pub(crate) const SPACES: &str = make_spaces!(,,,,,,,,,,);

fn append_docs2<'a, 'd, T, A>(
    ldoc: &'d Doc<'a, T, A>,
    rdoc: &'d Doc<'a, T, A>,
    mut consumer: impl FnMut(&'d Doc<'a, T, A>),
) -> &'d Doc<'a, T, A>
where
    T: DocPtr<'a, A>,
{
    let d = append_docs(rdoc, &mut consumer);
    consumer(d);
    append_docs(ldoc, &mut consumer)
}

fn append_docs<'a, 'd, T, A>(
    mut doc: &'d Doc<'a, T, A>,
    consumer: &mut impl FnMut(&'d Doc<'a, T, A>),
) -> &'d Doc<'a, T, A>
where
    T: DocPtr<'a, A>,
{
    loop {
        // Since appended documents often appear in sequence on the left side we
        // gain a slight performance increase by batching these pushes (avoiding
        // to push and directly pop `Append` documents)
        match doc {
            Doc::Append(l, r) => {
                consumer(r);
                doc = l;
            }
            _ => return doc,
        }
    }
}

pub(super) fn best<'a, W, T, A>(
    doc: &Doc<'a, T, A>,
    width: usize,
    out: &mut W,
) -> Result<(), W::Error>
where
    T: DocPtr<'a, A> + 'a,
    W: ?Sized + for<'b> RenderAnnotated<'b, A>,
{
    let temp_arena = &typed_arena::Arena::new();
    Best {
        pos: 0,
        bcmds: vec![(0, Mode::Break, doc)],
        fcmds: vec![],
        annotation_levels: vec![],
        width,
        temp_arena,
    }
    .best(0, out)?;

    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum Mode {
    Break,
    Flat,
}

type Cmd<'d, 'a, T, A> = (usize, Mode, &'d Doc<'a, T, A>);

fn write_newline<W>(ind: usize, out: &mut W) -> Result<(), W::Error>
where
    W: ?Sized + Render,
{
    out.write_str_all("\n")?;
    write_spaces(ind, out)
}

fn write_spaces<W>(spaces: usize, out: &mut W) -> Result<(), W::Error>
where
    W: ?Sized + Render,
{
    let mut inserted = 0;
    while inserted < spaces {
        let insert = cmp::min(SPACES.len(), spaces - inserted);
        inserted += out.write_str(&SPACES[..insert])?;
    }

    Ok(())
}

struct Best<'d, 'a, T, A>
where
    T: DocPtr<'a, A> + 'a,
{
    pos: usize,
    bcmds: Vec<Cmd<'d, 'a, T, A>>,
    fcmds: Vec<&'d Doc<'a, T, A>>,
    annotation_levels: Vec<usize>,
    width: usize,
    temp_arena: &'d typed_arena::Arena<T>,
}

impl<'d, 'a, T, A> Best<'d, 'a, T, A>
where
    T: DocPtr<'a, A> + 'a,
{
    fn fitting(&mut self, next: &'d Doc<'a, T, A>, mut pos: usize, ind: usize) -> bool {
        let mut bidx = self.bcmds.len();
        self.fcmds.clear(); // clear from previous calls from best
        self.fcmds.push(next);

        let mut mode = Mode::Flat;
        loop {
            let mut doc = match self.fcmds.pop() {
                None => {
                    if bidx == 0 {
                        // All commands have been processed
                        return true;
                    } else {
                        bidx -= 1;
                        mode = Mode::Break;
                        self.bcmds[bidx].2
                    }
                }
                Some(cmd) => cmd,
            };

            loop {
                match *doc {
                    Doc::Nil => {}
                    Doc::Append(ref ldoc, ref rdoc) => {
                        doc = append_docs2(ldoc, rdoc, |doc| self.fcmds.push(doc));
                        continue;
                    }
                    // Newlines inside the group makes it not fit, but those outside lets it
                    // fit on the current line
                    Doc::Hardline => return mode == Mode::Break,
                    Doc::RenderLen(len, _) => {
                        pos += len;
                        if pos > self.width {
                            return false;
                        }
                    }
                    Doc::BorrowedText(str) => {
                        pos += str.len();
                        if pos > self.width {
                            return false;
                        }
                    }
                    Doc::OwnedText(ref str) => {
                        pos += str.len();
                        if pos > self.width {
                            return false;
                        }
                    }
                    Doc::SmallText(ref str) => {
                        pos += str.len();
                        if pos > self.width {
                            return false;
                        }
                    }
                    Doc::FlatAlt(ref b, ref f) => {
                        doc = match mode {
                            Mode::Break => b,
                            Mode::Flat => f,
                        };
                        continue;
                    }

                    Doc::Column(ref f) => {
                        doc = self.temp_arena.alloc(f(pos));
                        continue;
                    }
                    Doc::Nesting(ref f) => {
                        doc = self.temp_arena.alloc(f(ind));
                        continue;
                    }
                    Doc::Nest(_, ref next)
                    | Doc::Group(ref next)
                    | Doc::Annotated(_, ref next)
                    | Doc::Union(_, ref next) => {
                        doc = next;
                        continue;
                    }
                    Doc::Fail => return false,
                }
                break;
            }
        }
    }

    fn best<W>(&mut self, top: usize, out: &mut W) -> Result<bool, W::Error>
    where
        W: RenderAnnotated<'d, A> + ?Sized,
    {
        let mut fits = true;

        while top < self.bcmds.len() {
            let mut cmd = self
                .bcmds
                .pop()
                .expect("bcmds must be non-empty while processing");
            loop {
                let (ind, mode, doc) = cmd;
                match *doc {
                    Doc::Nil => {}
                    Doc::Append(ref ldoc, ref rdoc) => {
                        cmd.2 = append_docs2(ldoc, rdoc, |doc| self.bcmds.push((ind, mode, doc)));
                        continue;
                    }
                    Doc::FlatAlt(ref b, ref f) => {
                        cmd.2 = match mode {
                            Mode::Break => b,
                            Mode::Flat => f,
                        };
                        continue;
                    }
                    Doc::Group(ref doc) => {
                        if mode == Mode::Break && self.fitting(doc, self.pos, ind) {
                            cmd.1 = Mode::Flat;
                        }
                        cmd.2 = doc;
                        continue;
                    }
                    Doc::Nest(off, ref doc) => {
                        // Once https://doc.rust-lang.org/std/primitive.usize.html#method.saturating_add_signed is stable
                        // this can be replaced
                        let new_ind = if off >= 0 {
                            ind.saturating_add(off as usize)
                        } else {
                            ind.saturating_sub(off.unsigned_abs())
                        };
                        cmd = (new_ind, mode, doc);
                        continue;
                    }
                    Doc::Hardline => {
                        // The next document may have different indentation so we should use it if
                        // we can
                        if let Some(next) = self.bcmds.pop() {
                            write_newline(next.0, out)?;
                            self.pos = next.0;
                            cmd = next;
                            continue;
                        } else {
                            write_newline(ind, out)?;
                            self.pos = ind;
                        }
                    }
                    Doc::RenderLen(len, ref doc) => match **doc {
                        Doc::OwnedText(ref s) => {
                            out.write_str_all(s)?;
                            self.pos += len;
                            fits &= self.pos <= self.width;
                        }
                        Doc::BorrowedText(s) => {
                            out.write_str_all(s)?;
                            self.pos += len;
                            fits &= self.pos <= self.width;
                        }
                        Doc::SmallText(ref s) => {
                            out.write_str_all(s)?;
                            self.pos += len;
                            fits &= self.pos <= self.width;
                        }
                        _ => unreachable!(),
                    },
                    Doc::OwnedText(ref s) => {
                        out.write_str_all(s)?;
                        self.pos += s.len();
                        fits &= self.pos <= self.width;
                    }
                    Doc::BorrowedText(s) => {
                        out.write_str_all(s)?;
                        self.pos += s.len();
                        fits &= self.pos <= self.width;
                    }
                    Doc::SmallText(ref s) => {
                        out.write_str_all(s)?;
                        self.pos += s.len();
                        fits &= self.pos <= self.width;
                    }
                    Doc::Annotated(ref ann, ref doc) => {
                        out.push_annotation(ann)?;
                        self.annotation_levels.push(self.bcmds.len());
                        cmd.2 = doc;
                        continue;
                    }
                    Doc::Union(ref l, ref r) => {
                        let pos = self.pos;
                        let annotation_levels = self.annotation_levels.len();
                        let bcmds = self.bcmds.len();

                        self.bcmds.push((ind, mode, l));

                        let mut buffer = BufferWrite::new();

                        match self.best(bcmds, &mut buffer) {
                            Ok(true) => buffer.render(out)?,
                            Ok(false) | Err(()) => {
                                self.pos = pos;
                                self.bcmds.truncate(bcmds);
                                self.annotation_levels.truncate(annotation_levels);
                                cmd.2 = r;
                                continue;
                            }
                        }
                    }
                    Doc::Column(ref f) => {
                        cmd.2 = self.temp_arena.alloc(f(self.pos));
                        continue;
                    }
                    Doc::Nesting(ref f) => {
                        cmd.2 = self.temp_arena.alloc(f(ind));
                        continue;
                    }
                    Doc::Fail => return Err(out.fail_doc()),
                }

                break;
            }
            while self.annotation_levels.last() == Some(&self.bcmds.len()) {
                self.annotation_levels.pop();
                out.pop_annotation()?;
            }
        }

        Ok(fits)
    }
}
