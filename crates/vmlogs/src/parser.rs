use std::fmt;
use thiserror::Error;
use winnow::ascii::{digit1, space0, space1};
use winnow::combinator::{
    alt, delimited, eof, not, opt, peek, preceded, repeat, separated_pair, terminated,
};
use winnow::prelude::*;
use winnow::token::take_while;

type I<'a> = &'a str;

#[derive(Debug, Error)]
pub enum ParseErr {
    #[error("{0}")]
    Msg(String),
}
type PResult<T> = Result<T, winnow::error::ErrMode<winnow::error::ContextError>>;

#[derive(Debug, Clone)]
pub struct VmStack<'a> {
    raw_content: &'a str,
}

impl<'a> VmStack<'a> {
    #[must_use]
    pub const fn new(content: &'a str) -> Self {
        Self {
            raw_content: content,
        }
    }

    #[must_use]
    pub const fn raw(&self) -> &'a str {
        self.raw_content
    }

    #[must_use]
    pub fn parsed(&self) -> Vec<VmStackValue> {
        parse_stack_content(self.raw_content)
    }
}

impl fmt::Display for VmStack<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let parsed = self.parsed();
        f.write_str("[ ")?;
        let mut first = true;
        for value in &parsed {
            if !first {
                f.write_str(" ")?;
            }
            first = false;
            write!(f, "{value}")?;
        }
        f.write_str(" ]")
    }
}

fn parse_stack_content(input: &str) -> Vec<VmStackValue> {
    let content = input.trim_start_matches('[').trim_end_matches(']');
    if content.is_empty() {
        return Vec::new();
    }

    let mut remaining = content;
    let mut values = Vec::new();

    while !remaining.is_empty() {
        match vm_stack_value(&mut remaining) {
            Ok(value) => {
                values.push(value);
            }
            Err(_) => {
                break;
            }
        }
    }

    values
}

#[derive(Debug, Clone)]
pub enum VmLine<'a> {
    VmStack { stack: VmStack<'a> },
    VmLoc { hash: &'a str, offset: &'a str },
    VmExecute { instr: &'a str },
    VmLimitChanged { limit: &'a str },
    VmGasRemaining { gas: &'a str },
    VmException { errno: &'a str, message: &'a str },
    VmExceptionHandler { errno: &'a str },
    VmFinalC5 { value: CellLike },
    VmUnknown { text: &'a str },
}

#[derive(Debug, Clone)]
pub enum VmStackValue {
    Null,
    NaN,
    Integer(String),
    Tuple(Vec<VmStackValue>),
    Cell(CellLike),
    Continuation(String),
    Builder(String),
    CellSlice(CellSlice),
    String(String),
    Unknown,
}

impl fmt::Display for VmStackValue {
    /// TVM log format: "()" for null, space-separated tuples.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VmStackValue::Null => f.write_str("()"),
            VmStackValue::NaN => f.write_str("NaN"),
            VmStackValue::Integer(s) => f.write_str(s),
            VmStackValue::Tuple(items) => {
                f.write_str("[ ")?;
                let mut first = true;
                for value in items {
                    if !first {
                        f.write_str(" ")?;
                    }
                    first = false;
                    write!(f, "{value}")?;
                }
                f.write_str(" ]")
            }
            VmStackValue::Cell(cell) => write!(f, "{cell}"),
            VmStackValue::Continuation(s) => write!(f, "Cont{{{s}}}"),
            VmStackValue::Builder(s) => write!(f, "BC{{{s}}}"),
            VmStackValue::CellSlice(cs) => write!(f, "{cs}"),
            VmStackValue::String(s) => write!(f, "\"{s}\""),
            VmStackValue::Unknown => f.write_str("???"),
        }
    }
}

#[derive(Debug, Clone)]
pub enum CellLike {
    Cell(String),
    Builder(String),
}

impl fmt::Display for CellLike {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CellLike::Cell(s) => write!(f, "C{{{s}}}"),
            CellLike::Builder(s) => write!(f, "BC{{{s}}}"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct CellSlice {
    pub value: String,
    pub bits: Option<(String, String)>,
    pub refs: Option<(String, String)>,
}

impl fmt::Display for CellSlice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match (&self.bits, &self.refs) {
            (Some((bits_start, bits_end)), Some((refs_start, refs_end))) => {
                write!(
                    f,
                    "CS{{Cell{{{}}} bits:{}..{} ; refs:{}..{}}}",
                    self.value, bits_start, bits_end, refs_start, refs_end
                )
            }
            _ => write!(f, "CS{{{}}}", self.value),
        }
    }
}

fn ws0(i: &mut I) -> PResult<()> {
    space0.parse_next(i).map(|_| ())
}

fn ws1(i: &mut I) -> PResult<()> {
    space1.parse_next(i).map(|_| ())
}

#[allow(unsafe_code)]
fn number<'a>(i: &mut I<'a>) -> PResult<&'a str> {
    let start = i.as_ptr() as usize;
    opt('-').parse_next(i)?;
    digit1.parse_next(i)?;
    let end = i.as_ptr() as usize;
    let len = end - start;
    // SAFETY: We know this is valid UTF-8 since we only consumed ASCII characters
    unsafe {
        let slice = std::slice::from_raw_parts(start as *const u8, len);
        Ok(std::str::from_utf8_unchecked(slice))
    }
}

fn hex<'a>(i: &mut I<'a>) -> PResult<&'a str> {
    take_while(1.., |c: char| c.is_ascii_hexdigit()).parse_next(i)
}

fn until_eol<'a>(i: &mut I<'a>) -> PResult<&'a str> {
    take_while(0.., |c: char| c != '\n' && c != '\r').parse_next(i)
}

fn tag(i: &mut I, mut s: &'static str) -> PResult<()> {
    s.parse_next(i).map(|_: &str| ())
}

// Null / NaN / Integer
fn null_val(i: &mut I<'_>) -> PResult<VmStackValue> {
    alt((
        delimited("(", ws0, delimited("", ws0, ")")).value(VmStackValue::Null), // "()" with spaces
        "(null)".value(VmStackValue::Null),
        "null".value(VmStackValue::Null),
    ))
    .parse_next(i)
    .or_else(|_| "NaN".value(VmStackValue::NaN).parse_next(i))
}

fn integer_val(i: &mut I<'_>) -> PResult<VmStackValue> {
    number
        .map(|s: &str| VmStackValue::Integer(s.to_string()))
        .parse_next(i)
}

fn tuple_brackets(i: &mut I<'_>) -> PResult<VmStackValue> {
    delimited(
        "[",
        preceded(ws0, repeat(0.., terminated(vm_stack_value, ws0))),
        "]",
    )
    .map(VmStackValue::Tuple)
    .parse_next(i)
}

fn tuple_paren(i: &mut I<'_>) -> PResult<VmStackValue> {
    delimited(
        "(",
        preceded(ws0, repeat(0.., terminated(vm_stack_value, ws0))),
        ")",
    )
    .map(VmStackValue::Tuple)
    .parse_next(i)
}

fn cell(i: &mut I<'_>) -> PResult<CellLike> {
    delimited("C{", hex, "}")
        .map(|h: &str| CellLike::Cell(h.to_string()))
        .parse_next(i)
}

fn builder(i: &mut I<'_>) -> PResult<CellLike> {
    delimited("BC{", hex, "}")
        .map(|h: &str| CellLike::Builder(h.to_string()))
        .parse_next(i)
}

fn continuation(i: &mut I<'_>) -> PResult<VmStackValue> {
    delimited(
        "Cont{",
        take_while(0.., |c: char| c.is_ascii_alphanumeric() || c == '_'),
        "}",
    )
    .map(|s: &str| VmStackValue::Continuation(s.to_string()))
    .parse_next(i)
}

fn cell_slice_bits<'a>(i: &mut I<'a>) -> PResult<(&'a str, &'a str)> {
    preceded(("bits:", ws0), separated_pair(number, "..", number)).parse_next(i)
}

fn cell_slice_refs<'a>(i: &mut I<'a>) -> PResult<(&'a str, &'a str)> {
    preceded(("refs:", ws0), separated_pair(number, "..", number)).parse_next(i)
}

fn cell_slice_body_long(i: &mut I<'_>) -> PResult<CellSlice> {
    let value = delimited("Cell{", hex, "}").parse_next(i)?;
    ws1(i)?;
    let bits = cell_slice_bits.parse_next(i)?;
    ws0(i)?;
    tag(i, ";")?;
    ws1(i)?;
    let refs = cell_slice_refs.parse_next(i)?;
    Ok(CellSlice {
        value: value.to_string(),
        bits: Some((bits.0.to_string(), bits.1.to_string())),
        refs: Some((refs.0.to_string(), refs.1.to_string())),
    })
}

fn cell_slice_body_short(i: &mut I<'_>) -> PResult<CellSlice> {
    let h = hex.parse_next(i)?;
    Ok(CellSlice {
        value: h.to_string(),
        bits: None,
        refs: None,
    })
}

fn cell_slice(i: &mut I<'_>) -> PResult<VmStackValue> {
    delimited(
        "CS{",
        alt((cell_slice_body_long, cell_slice_body_short)),
        "}",
    )
    .map(VmStackValue::CellSlice)
    .parse_next(i)
}

fn string_literal(i: &mut I<'_>) -> PResult<VmStackValue> {
    delimited("\"", take_while(0.., |c: char| c != '"'), "\"")
        .map(|s: &str| VmStackValue::String(s.to_string()))
        .parse_next(i)
}

fn unknown_val(i: &mut I<'_>) -> PResult<VmStackValue> {
    "???".value(VmStackValue::Unknown).parse_next(i)
}

pub fn vm_stack_value(i: &mut I<'_>) -> PResult<VmStackValue> {
    preceded(
        ws0,
        alt((
            null_val,
            integer_val,
            tuple_brackets,
            tuple_paren,
            cell.map(VmStackValue::Cell),
            continuation,
            builder.map(|b| match b {
                CellLike::Builder(h) => VmStackValue::Builder(h),
                CellLike::Cell(_) => unreachable!(),
            }),
            cell_slice,
            string_literal,
            unknown_val,
        )),
    )
    .parse_next(i)
}

fn vm_stack<'a>(i: &mut I<'a>) -> PResult<VmLine<'a>> {
    // "stack: " <capture everything until end of line as raw string>
    let _ = "stack: ".parse_next(i)?;
    let raw_stack = until_eol.parse_next(i)?;
    Ok(VmLine::VmStack {
        stack: VmStack::new(raw_stack.trim()),
    })
}

fn vm_loc<'a>(i: &mut I<'a>) -> PResult<VmLine<'a>> {
    // "code cell hash:" space* hex space+ "offset:" space* number
    let _ = "code cell hash:".parse_next(i)?;
    ws0(i)?;
    let h = hex.parse_next(i)?;
    ws1(i)?;
    let _ = "offset:".parse_next(i)?;
    ws0(i)?;
    let off = number.parse_next(i)?;
    Ok(VmLine::VmLoc {
        hash: h,
        offset: off,
    })
}

fn vm_execute<'a>(i: &mut I<'a>) -> PResult<VmLine<'a>> {
    let _ = "execute ".parse_next(i)?;
    let t = until_eol.parse_next(i)?;
    Ok(VmLine::VmExecute { instr: t.trim() })
}

fn vm_limit_changed<'a>(i: &mut I<'a>) -> PResult<VmLine<'a>> {
    let _ = "changing gas limit to ".parse_next(i)?;
    let n = number.parse_next(i)?;
    Ok(VmLine::VmLimitChanged { limit: n })
}

fn vm_gas_remaining<'a>(i: &mut I<'a>) -> PResult<VmLine<'a>> {
    let _ = "gas remaining: ".parse_next(i)?;
    let n = number.parse_next(i)?;
    Ok(VmLine::VmGasRemaining { gas: n })
}

fn vm_exception<'a>(i: &mut I<'a>) -> PResult<VmLine<'a>> {
    let _ = "handling exception code ".parse_next(i)?;
    let errno = number.parse_next(i)?;
    let _ = ": ".parse_next(i)?;
    let msg = until_eol.parse_next(i)?;
    Ok(VmLine::VmException {
        errno,
        message: msg.trim(),
    })
}

fn vm_exception_handler<'a>(i: &mut I<'a>) -> PResult<VmLine<'a>> {
    let _ = "default exception handler, terminating vm with exit code ".parse_next(i)?;
    let errno = number.parse_next(i)?;
    Ok(VmLine::VmExceptionHandler { errno })
}

fn vm_final_c5<'a>(i: &mut I<'a>) -> PResult<VmLine<'a>> {
    let _ = "final c5: ".parse_next(i)?;
    let c = cell.parse_next(i)?;
    Ok(VmLine::VmFinalC5 { value: c })
}

fn vm_unknown<'a>(i: &mut I<'a>) -> PResult<VmLine<'a>> {
    // not(peek(alt(...)))
    not(peek(alt((
        "stack: ",
        "code cell hash:",
        "execute ",
        "changing gas limit to ",
        "gas remaining: ",
        "handling exception code ",
        "default exception handler, terminating vm with exit code ",
        "final c5:",
    ))))
    .parse_next(i)?;
    let t = until_eol.parse_next(i)?;
    Ok(VmLine::VmUnknown { text: t.trim() })
}

pub fn vm_line<'a>(i: &mut I<'a>) -> PResult<VmLine<'a>> {
    alt((
        vm_loc,
        vm_stack,
        vm_execute,
        vm_limit_changed,
        vm_gas_remaining,
        vm_exception,
        vm_exception_handler,
        vm_final_c5,
        vm_unknown,
    ))
    .parse_next(i)
}

#[must_use]
pub fn parse_lines(input: &str) -> Vec<Result<VmLine<'_>, String>> {
    input
        .split_inclusive('\n')
        .map(|line| {
            let s = line.trim_end_matches(['\r', '\n', ' '].as_ref());
            match terminated(vm_line, opt(eof)).parse(s.as_ref()) {
                Ok(v) => Ok(v),
                Err(e) => Err(format!("{e:?} @ {line:?}")),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{VmStack, VmStackValue};

    #[test]
    fn parses_quoted_string_in_stack() {
        let parsed = VmStack::new("[ \"hello world\" ]").parsed();
        assert_eq!(parsed.len(), 1);
        match &parsed[0] {
            VmStackValue::String(value) => assert_eq!(value, "hello world"),
            other => panic!("expected string, got {other:?}"),
        }
    }

    #[test]
    fn parses_quoted_string_inside_tuple() {
        let parsed = VmStack::new("[ ( \"hello\" 1 ) ]").parsed();
        assert_eq!(parsed.len(), 1);
        match &parsed[0] {
            VmStackValue::Tuple(items) => {
                assert_eq!(items.len(), 2);
                match &items[0] {
                    VmStackValue::String(value) => assert_eq!(value, "hello"),
                    other => panic!("expected tuple string, got {other:?}"),
                }
                match &items[1] {
                    VmStackValue::Integer(value) => assert_eq!(value, "1"),
                    other => panic!("expected tuple integer, got {other:?}"),
                }
            }
            other => panic!("expected tuple, got {other:?}"),
        }
    }
}
