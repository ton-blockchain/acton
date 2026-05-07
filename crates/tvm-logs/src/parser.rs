use std::fmt;

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

    let mut parser = StackParser::new(content);
    let mut values = Vec::with_capacity(8);

    loop {
        parser.skip_ws();
        if parser.is_eof() {
            break;
        }
        match parser.parse_value() {
            Ok(value) => {
                values.push(value);
            }
            Err(()) => {
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
    VmRegisteredCell { hash: &'a str, boc: &'a str },
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

struct StackParser<'a> {
    input: &'a str,
    pos: usize,
}

impl<'a> StackParser<'a> {
    const fn new(input: &'a str) -> Self {
        Self { input, pos: 0 }
    }

    const fn is_eof(&self) -> bool {
        self.pos >= self.input.len()
    }

    fn peek_byte(&self) -> Option<u8> {
        self.input.as_bytes().get(self.pos).copied()
    }

    fn starts_with(&self, prefix: &str) -> bool {
        self.input[self.pos..].starts_with(prefix)
    }

    fn consume_prefix(&mut self, prefix: &str) -> bool {
        if self.starts_with(prefix) {
            self.pos += prefix.len();
            true
        } else {
            false
        }
    }

    fn consume_byte(&mut self, byte: u8) -> bool {
        if self.peek_byte() == Some(byte) {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    fn expect_byte(&mut self, byte: u8) -> Result<(), ()> {
        if self.consume_byte(byte) {
            Ok(())
        } else {
            Err(())
        }
    }

    fn skip_ws(&mut self) {
        while self
            .peek_byte()
            .is_some_and(|byte| byte.is_ascii_whitespace())
        {
            self.pos += 1;
        }
    }

    fn consume_ws1(&mut self) -> Result<(), ()> {
        let start = self.pos;
        self.skip_ws();
        if self.pos > start { Ok(()) } else { Err(()) }
    }

    fn parse_value(&mut self) -> Result<VmStackValue, ()> {
        self.skip_ws();
        if self.consume_prefix("(null)") {
            return Ok(VmStackValue::Null);
        }

        match self.peek_byte().ok_or(())? {
            b'(' => self.parse_paren_value(),
            b'[' => self.parse_tuple(b'[', b']').map(VmStackValue::Tuple),
            b'-' | b'0'..=b'9' => self
                .parse_number_str()
                .map(|value| VmStackValue::Integer(value.to_string())),
            b'C' => self.parse_c_prefixed_value(),
            b'B' => self.parse_builder(),
            b'N' => {
                if self.consume_prefix("NaN") {
                    Ok(VmStackValue::NaN)
                } else {
                    Err(())
                }
            }
            b'n' => {
                if self.consume_prefix("null") {
                    Ok(VmStackValue::Null)
                } else {
                    Err(())
                }
            }
            b'"' => self.parse_string(),
            b'?' => {
                if self.consume_prefix("???") {
                    Ok(VmStackValue::Unknown)
                } else {
                    Err(())
                }
            }
            _ => Err(()),
        }
    }

    fn parse_paren_value(&mut self) -> Result<VmStackValue, ()> {
        let start = self.pos;
        self.pos += 1;
        self.skip_ws();
        if self.consume_byte(b')') {
            return Ok(VmStackValue::Null);
        }
        self.pos = start;
        self.parse_tuple(b'(', b')').map(VmStackValue::Tuple)
    }

    fn parse_c_prefixed_value(&mut self) -> Result<VmStackValue, ()> {
        if self.starts_with("C{") {
            self.parse_cell().map(VmStackValue::Cell)
        } else if self.starts_with("Cont{") {
            self.parse_continuation()
        } else if self.starts_with("CS{") {
            self.parse_cell_slice()
        } else {
            Err(())
        }
    }

    fn parse_tuple(&mut self, open: u8, close: u8) -> Result<Vec<VmStackValue>, ()> {
        self.expect_byte(open)?;
        let mut values = Vec::with_capacity(4);
        loop {
            self.skip_ws();
            if self.consume_byte(close) {
                return Ok(values);
            }
            if self.is_eof() {
                return Err(());
            }
            values.push(self.parse_value()?);
        }
    }

    fn parse_number_str(&mut self) -> Result<&'a str, ()> {
        let start = self.pos;
        self.consume_byte(b'-');
        let digits_start = self.pos;
        while self.peek_byte().is_some_and(|byte| byte.is_ascii_digit()) {
            self.pos += 1;
        }
        if self.pos == digits_start {
            self.pos = start;
            return Err(());
        }
        Ok(&self.input[start..self.pos])
    }

    fn parse_hex_str(&mut self) -> Result<&'a str, ()> {
        let start = self.pos;
        while self
            .peek_byte()
            .is_some_and(|byte| byte.is_ascii_hexdigit())
        {
            self.pos += 1;
        }
        if self.pos == start {
            return Err(());
        }
        Ok(&self.input[start..self.pos])
    }

    fn parse_cell(&mut self) -> Result<CellLike, ()> {
        if !self.consume_prefix("C{") {
            return Err(());
        }
        let value = self.parse_hex_str()?.to_string();
        self.expect_byte(b'}')?;
        Ok(CellLike::Cell(value))
    }

    fn parse_builder(&mut self) -> Result<VmStackValue, ()> {
        if !self.consume_prefix("BC{") {
            return Err(());
        }
        let value = self.parse_hex_str()?.to_string();
        self.expect_byte(b'}')?;
        Ok(VmStackValue::Builder(value))
    }

    fn parse_continuation(&mut self) -> Result<VmStackValue, ()> {
        if !self.consume_prefix("Cont{") {
            return Err(());
        }
        let start = self.pos;
        while self
            .peek_byte()
            .is_some_and(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
        {
            self.pos += 1;
        }
        let value = self.input[start..self.pos].to_string();
        self.expect_byte(b'}')?;
        Ok(VmStackValue::Continuation(value))
    }

    fn parse_cell_slice(&mut self) -> Result<VmStackValue, ()> {
        if !self.consume_prefix("CS{") {
            return Err(());
        }

        let value = if self.starts_with("Cell{") {
            self.parse_long_cell_slice()?
        } else {
            let value = self.parse_hex_str()?.to_string();
            CellSlice {
                value,
                bits: None,
                refs: None,
            }
        };

        self.expect_byte(b'}')?;
        Ok(VmStackValue::CellSlice(value))
    }

    fn parse_long_cell_slice(&mut self) -> Result<CellSlice, ()> {
        if !self.consume_prefix("Cell{") {
            return Err(());
        }
        let value = self.parse_hex_str()?.to_string();
        self.expect_byte(b'}')?;
        self.consume_ws1()?;

        if !self.consume_prefix("bits:") {
            return Err(());
        }
        self.skip_ws();
        let bits_start = self.parse_number_str()?.to_string();
        if !self.consume_prefix("..") {
            return Err(());
        }
        let bits_end = self.parse_number_str()?.to_string();

        self.skip_ws();
        self.expect_byte(b';')?;
        self.consume_ws1()?;

        if !self.consume_prefix("refs:") {
            return Err(());
        }
        self.skip_ws();
        let refs_start = self.parse_number_str()?.to_string();
        if !self.consume_prefix("..") {
            return Err(());
        }
        let refs_end = self.parse_number_str()?.to_string();

        Ok(CellSlice {
            value,
            bits: Some((bits_start, bits_end)),
            refs: Some((refs_start, refs_end)),
        })
    }

    fn parse_string(&mut self) -> Result<VmStackValue, ()> {
        self.expect_byte(b'"')?;
        let start = self.pos;
        while let Some(byte) = self.peek_byte() {
            if byte == b'"' {
                let value = self.input[start..self.pos].to_string();
                self.pos += 1;
                return Ok(VmStackValue::String(value));
            }
            self.pos += 1;
        }
        Err(())
    }
}

pub fn vm_stack_value(i: &mut &str) -> Result<VmStackValue, &'static str> {
    let input = *i;
    let mut parser = StackParser::new(input);
    let Ok(value) = parser.parse_value() else {
        return Err("expected stack value");
    };
    *i = &input[parser.pos..];
    Ok(value)
}

pub fn parse_lines(input: &str) -> impl Iterator<Item = Result<VmLine<'_>, String>> + '_ {
    input.split_inclusive('\n').map(|line| {
        let s = line.trim_end_matches(['\r', '\n', ' '].as_ref());
        parse_line(s).map_err(|err| format!("{err} @ {line:?}"))
    })
}

fn parse_line(line: &str) -> Result<VmLine<'_>, &'static str> {
    if let Some(raw_stack) = line.strip_prefix("stack: ") {
        return Ok(VmLine::VmStack {
            stack: VmStack::new(raw_stack.trim()),
        });
    }
    if let Some(rest) = line.strip_prefix("code cell hash:") {
        return parse_loc_line(rest);
    }
    if let Some(instr) = line.strip_prefix("execute ") {
        return Ok(VmLine::VmExecute {
            instr: instr.trim(),
        });
    }
    if let Some(rest) = line.strip_prefix("register new cell ") {
        return parse_registered_cell_line(rest);
    }
    if let Some(limit) = line.strip_prefix("changing gas limit to ") {
        return parse_number_line(limit).map(|limit| VmLine::VmLimitChanged { limit });
    }
    if let Some(gas) = line.strip_prefix("gas remaining: ") {
        return parse_number_line(gas).map(|gas| VmLine::VmGasRemaining { gas });
    }
    if let Some(rest) = line.strip_prefix("handling exception code ") {
        return parse_exception_line(rest);
    }
    if let Some(errno) =
        line.strip_prefix("default exception handler, terminating vm with exit code ")
    {
        return parse_number_line(errno).map(|errno| VmLine::VmExceptionHandler { errno });
    }
    if let Some(rest) = line.strip_prefix("final c5: ") {
        return parse_final_c5_line(rest);
    }
    Ok(VmLine::VmUnknown { text: line.trim() })
}

fn parse_loc_line(rest: &str) -> Result<VmLine<'_>, &'static str> {
    let rest = rest.trim_start();
    let Some(hash_end) = rest.find(char::is_whitespace) else {
        return Err("expected code cell hash");
    };
    let hash = &rest[..hash_end];
    if !is_hex(hash) {
        return Err("expected hex code cell hash");
    }
    let rest = rest[hash_end..].trim_start();
    let Some(offset) = rest.strip_prefix("offset:") else {
        return Err("expected code cell offset");
    };
    let offset = parse_number_line(offset.trim_start())?;
    Ok(VmLine::VmLoc { hash, offset })
}

fn parse_registered_cell_line(rest: &str) -> Result<VmLine<'_>, &'static str> {
    let Some((hash, boc)) = rest.split_once(':') else {
        return Err("expected registered cell separator");
    };
    if !is_hex(hash) {
        return Err("expected registered cell hash");
    }
    Ok(VmLine::VmRegisteredCell {
        hash,
        boc: boc.trim(),
    })
}

fn parse_exception_line(rest: &str) -> Result<VmLine<'_>, &'static str> {
    let Some((errno, message)) = rest.split_once(": ") else {
        return Err("expected exception message");
    };
    let errno = parse_number_line(errno)?;
    Ok(VmLine::VmException {
        errno,
        message: message.trim(),
    })
}

fn parse_final_c5_line(rest: &str) -> Result<VmLine<'_>, &'static str> {
    let Some(value) = rest
        .strip_prefix("C{")
        .and_then(|rest| rest.strip_suffix('}'))
    else {
        return Err("expected final c5 cell");
    };
    if !is_hex(value) {
        return Err("expected final c5 cell hex");
    }
    Ok(VmLine::VmFinalC5 {
        value: CellLike::Cell(value.to_string()),
    })
}

fn parse_number_line(value: &str) -> Result<&str, &'static str> {
    if is_number(value) {
        Ok(value)
    } else {
        Err("expected number")
    }
}

fn is_number(value: &str) -> bool {
    let value = value.strip_prefix('-').unwrap_or(value);
    !value.is_empty() && value.as_bytes().iter().all(u8::is_ascii_digit)
}

fn is_hex(value: &str) -> bool {
    !value.is_empty() && value.as_bytes().iter().all(u8::is_ascii_hexdigit)
}

#[cfg(test)]
mod tests {
    use super::{VmLine, VmStack, VmStackValue, parse_lines};

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

    #[test]
    fn parses_registered_cell_line() {
        let parsed: Vec<_> =
            parse_lines("register new cell 0F: B5EE9C72010101010002000000\nstack: [ C{0F} ]\n")
                .collect();

        match &parsed[0] {
            Ok(VmLine::VmRegisteredCell { hash, boc }) => {
                assert_eq!(*hash, "0F");
                assert_eq!(*boc, "B5EE9C72010101010002000000");
            }
            other => panic!("expected registered cell line, got {other:?}"),
        }
        assert!(matches!(parsed[1], Ok(VmLine::VmStack { .. })));
    }
}
