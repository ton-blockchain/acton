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
    pub fn new(content: &'a str) -> Self {
        Self {
            raw_content: content,
        }
    }

    pub fn raw(&self) -> &'a str {
        self.raw_content
    }

    pub fn parsed(&self) -> Vec<VmStackValue<'a>> {
        parse_stack_content(self.raw_content)
    }

    pub fn to_string(&self) -> String {
        format!(
            "[ {} ]",
            self.parsed()
                .iter()
                .map(|v| v.to_string())
                .collect::<Vec<_>>()
                .join(" ")
        )
    }
}

fn parse_stack_content(input: &str) -> Vec<VmStackValue<'_>> {
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
    VmFinalC5 { value: CellLike<'a> },
    VmUnknown { text: &'a str },
}

#[derive(Debug, Clone)]
pub enum VmStackValue<'a> {
    Null,
    NaN,
    Integer(&'a str),
    Tuple(Vec<VmStackValue<'a>>),
    Cell(CellLike<'a>),
    Continuation(&'a str),
    Builder(&'a str),
    CellSlice(CellSlice<'a>),
    Unknown,
}

impl<'a> VmStackValue<'a> {
    pub fn to_string(&self) -> String {
        match self {
            VmStackValue::Null => "()".to_string(),
            VmStackValue::NaN => "NaN".to_string(),
            VmStackValue::Integer(s) => s.to_string(),
            VmStackValue::Tuple(items) => {
                format!(
                    "[ {} ]",
                    items
                        .iter()
                        .map(|v| v.to_string())
                        .collect::<Vec<_>>()
                        .join(" ")
                )
            }
            VmStackValue::Cell(cell) => cell.to_string(),
            VmStackValue::Continuation(s) => format!("Cont{{{}}}", s),
            VmStackValue::Builder(s) => format!("BC{{{}}}", s),
            VmStackValue::CellSlice(cs) => cs.to_string(),
            VmStackValue::Unknown => "???".to_string(),
        }
    }
}

#[derive(Debug, Clone)]
pub enum CellLike<'a> {
    Cell(&'a str),    // C{hex}
    Builder(&'a str), // BC{hex}
}

impl<'a> CellLike<'a> {
    pub fn to_string(&self) -> String {
        match self {
            CellLike::Cell(s) => format!("C{{{}}}", s),
            CellLike::Builder(s) => format!("BC{{{}}}", s),
        }
    }
}

#[derive(Debug, Clone)]
pub struct CellSlice<'a> {
    pub value: &'a str,
    pub bits: Option<(&'a str, &'a str)>,
    pub refs: Option<(&'a str, &'a str)>,
}

impl<'a> CellSlice<'a> {
    pub fn to_string(&self) -> String {
        match (&self.bits, &self.refs) {
            (Some((bits_start, bits_end)), Some((refs_start, refs_end))) => {
                format!(
                    "CS{{Cell{{{}}} bits:{}..{} ; refs:{}..{}}}",
                    self.value, bits_start, bits_end, refs_start, refs_end
                )
            }
            _ => format!("CS{{{}}}", self.value),
        }
    }
}

fn ws0(i: &mut I) -> PResult<()> {
    space0.parse_next(i).map(|_| ())
}

fn ws1(i: &mut I) -> PResult<()> {
    space1.parse_next(i).map(|_| ())
}

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
fn null_val<'a>(i: &mut I<'a>) -> PResult<VmStackValue<'a>> {
    alt((
        delimited("(", ws0, delimited("", ws0, ")")).value(VmStackValue::Null), // "()" с пробелами
        "(null)".value(VmStackValue::Null),
    ))
    .parse_next(i)
    .or_else(|_| "NaN".value(VmStackValue::NaN).parse_next(i))
}

fn integer_val<'a>(i: &mut I<'a>) -> PResult<VmStackValue<'a>> {
    number.map(VmStackValue::Integer).parse_next(i)
}

fn tuple_brackets<'a>(i: &mut I<'a>) -> PResult<VmStackValue<'a>> {
    delimited(
        "[",
        preceded(ws0, repeat(0.., terminated(vm_stack_value, ws0))),
        "]",
    )
    .map(VmStackValue::Tuple)
    .parse_next(i)
}

fn tuple_paren<'a>(i: &mut I<'a>) -> PResult<VmStackValue<'a>> {
    delimited(
        "(",
        preceded(ws0, repeat(0.., terminated(vm_stack_value, ws0))),
        ")",
    )
    .map(VmStackValue::Tuple)
    .parse_next(i)
}

fn cell<'a>(i: &mut I<'a>) -> PResult<CellLike<'a>> {
    delimited("C{", hex, "}")
        .map(|h: &str| CellLike::Cell(h))
        .parse_next(i)
}

fn builder<'a>(i: &mut I<'a>) -> PResult<CellLike<'a>> {
    delimited("BC{", hex, "}")
        .map(|h: &str| CellLike::Builder(h))
        .parse_next(i)
}

fn continuation<'a>(i: &mut I<'a>) -> PResult<VmStackValue<'a>> {
    delimited(
        "Cont{",
        take_while(0.., |c: char| c.is_ascii_alphanumeric() || c == '_'),
        "}",
    )
    .map(|s: &str| VmStackValue::Continuation(s))
    .parse_next(i)
}

fn cell_slice_bits<'a>(i: &mut I<'a>) -> PResult<(&'a str, &'a str)> {
    preceded("bits:", separated_pair(number, "..", number)).parse_next(i)
}

fn cell_slice_refs<'a>(i: &mut I<'a>) -> PResult<(&'a str, &'a str)> {
    preceded("refs:", separated_pair(number, "..", number)).parse_next(i)
}

fn cell_slice_body_long<'a>(i: &mut I<'a>) -> PResult<CellSlice<'a>> {
    // Cell{HEX} bits:a..b ; refs:c..d
    let value = delimited("Cell{", hex, "}").parse_next(i)?;
    ws1(i)?;
    let bits = cell_slice_bits.parse_next(i)?;
    ws0(i)?;
    tag(i, ";")?;
    ws1(i)?;
    let refs = cell_slice_refs.parse_next(i)?;
    Ok(CellSlice {
        value,
        bits: Some(bits),
        refs: Some(refs),
    })
}

fn cell_slice_body_short<'a>(i: &mut I<'a>) -> PResult<CellSlice<'a>> {
    let h = hex.parse_next(i)?;
    Ok(CellSlice {
        value: h,
        bits: None,
        refs: None,
    })
}

fn cell_slice<'a>(i: &mut I<'a>) -> PResult<VmStackValue<'a>> {
    delimited(
        "CS{",
        alt((cell_slice_body_long, cell_slice_body_short)),
        "}",
    )
    .map(VmStackValue::CellSlice)
    .parse_next(i)
}

fn unknown_val<'a>(i: &mut I<'a>) -> PResult<VmStackValue<'a>> {
    "???".value(VmStackValue::Unknown).parse_next(i)
}

fn vm_stack_value<'a>(i: &mut I<'a>) -> PResult<VmStackValue<'a>> {
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

pub fn parse_lines(input: &str) -> Vec<Result<VmLine<'_>, String>> {
    input
        .split_inclusive('\n')
        .map(|line| {
            let s = line.trim_end_matches(['\r', '\n', ' '].as_ref());
            match terminated(vm_line, opt(eof)).parse(&mut s.as_ref()) {
                Ok(v) => Ok(v),
                Err(e) => Err(format!("{e:?} @ {:?}", line)),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_LOGS: &str = r#"stack: [ 0 ]
code cell hash: C4252597808DE321E4DBEDFCF683B8D9A53BB1E5A77FDD44091B1163114468FA offset: 0
execute PUSHINT 200
gas remaining: 9999977
stack: [ 163613525018 0 C{B5EE9C72010232010002200001DD880100024AC57E0F8D9E1198A9F947061ACEA8AE8D056AC8E185E23049670F4F6396000E1027C7AB85EBDEFF36322CF89905A64762DA99FA1014DEAF4065E7FD17730B424A0AD6492B729326938F81B89DDCA402851CF427AD2B8F308066EBE25C1012F3FC951B41C4D1380000000401024BA476C3FF447BA631C5520DC59C78171343362F8486368CEB6D197F29E530B3E2EE6B2800007002030201200405000002037BA006070201200C0D0013AD9A741281DCD6539DC002012008090201200A0B0012AA11542503B9ACA1DD0011A852184A07735943F90011A83A604A07735942410201200E0F0202702C2D02012010110201201E1F020148121302014818190013AD9A741281DCD650B240020120141502012016170012AA11542503B9ACA1160011A852184A077359487D0011A83A604A07735940BD0013AD9A741281DCD68D28C00201201A1B0201201C1D0012AA11542503B9ACA78E0011A852184A07735941F30011A83A604A0773594195020148202102014826270013AD9A741281DCD652CEC0020120222302012024250012AA11542503B9ACA36C0011A852184A07735940BF0011A83A604A077359413D0013AD9A741281DCD6510BC002012028290201202A2B0012AA11542503B9ACBC310011A852184A07735948110011A83A604A07735948390013AD9A741281DCD650B2C00201202E2F02012030310012AA11542503B9ACA3510011A852184A07735940C10011A83A604A0773594FAB} CS{B5EE9C72010232010001FD00019801C204F8F570BD7BDFE6C6459F1320B4C8EC5B533F42029BD5E80CBCFFA2EE616849415AC9256E5264D271F03713BB948050A39E84F5A571E6100CDD7C4B82025E7F92A368389A270000000001024BA476C3FF447BA631C5520DC59C78171343362F8486368CEB6D197F29E530B3E2EE6B2800007002030201200405000002037BA006070201200C0D0013AD9A741281DCD6539DC002012008090201200A0B0012AA11542503B9ACA1DD0011A852184A07735943F90011A83A604A07735942410201200E0F0202702C2D02012010110201201E1F020148121302014818190013AD9A741281DCD650B240020120141502012016170012AA11542503B9ACA1160011A852184A077359487D0011A83A604A07735940BD0013AD9A741281DCD68D28C00201201A1B0201201C1D0012AA11542503B9ACA78E0011A852184A07735941F30011A83A604A0773594195020148202102014826270013AD9A741281DCD652CEC0020120222302012024250012AA11542503B9ACA36C0011A852184A07735940BF0011A83A604A077359413D0013AD9A741281DCD6510BC002012028290201202A2B0012AA11542503B9ACBC310011A852184A07735948110011A83A604A07735948390013AD9A741281DCD650B2C00201202E2F02012030310012AA11542503B9ACA3510011A852184A07735940C10011A83A604A0773594FAB} -1 ]
code cell hash: C4252597808DE321E4DBEDFCF683B8D9A53BB1E5A77FDD44091B1163114468FA offset: 32
execute FITS 1
gas remaining: 9999943
stack: [ 0 NaN CS{DEAD} ]
execute implicit RET
gas remaining: 9999938"#;

    #[test]
    fn test_parse_vm_logs() {
        let results = parse_lines(&TEST_LOGS);

        for (i, result) in results.iter().enumerate() {
            match result {
                Ok(vm_line) => {
                    println!("Line {}: Successfully parsed {:?}", i + 1, vm_line);
                }
                Err(e) => {
                    panic!("Failed to parse line {}: {}", i + 1, e);
                }
            }
        }

        assert_eq!(results.len(), 11, "Expected11 log lines");

        let success_count = results.iter().filter(|r| r.is_ok()).count();
        assert_eq!(success_count, 11, "All lines should parse successfully");
    }

    #[test]
    fn test_parse_stack_line() {
        let stack_line = "stack: [ 0 ]";
        let results = parse_lines(&stack_line);
        assert_eq!(results.len(), 1);
        assert!(results[0].is_ok());

        if let Ok(VmLine::VmStack { stack }) = &results[0] {
            assert_eq!(stack.raw(), "[ 0 ]");
            // Test lazy parsing
            let parsed = stack.parsed();
            assert_eq!(parsed.len(), 1);
            assert!(matches!(parsed[0], VmStackValue::Integer(s) if s == "0"));
        } else {
            panic!("Expected VmStack variant");
        }
    }

    #[test]
    fn test_parse_execute_line() {
        let execute_line = "execute PUSHINT 200";
        let results = parse_lines(&execute_line);
        assert_eq!(results.len(), 1);
        assert!(results[0].is_ok());

        if let Ok(VmLine::VmExecute { instr }) = &results[0] {
            assert_eq!(*instr, "PUSHINT 200");
        } else {
            panic!("Expected VmExecute variant");
        }
    }

    #[test]
    fn test_parse_gas_remaining_line() {
        let gas_line = "gas remaining: 9999977";
        let results = parse_lines(&gas_line);
        assert_eq!(results.len(), 1);
        assert!(results[0].is_ok());

        if let Ok(VmLine::VmGasRemaining { gas }) = &results[0] {
            assert_eq!(*gas, "9999977");
        } else {
            panic!("Expected VmGasRemaining variant");
        }
    }

    #[test]
    fn test_parse_complex_stack_line() {
        let stack_line = "stack: [ 0 NaN CS{DEAD} ]";
        let results = parse_lines(&stack_line);
        assert_eq!(results.len(), 1);
        assert!(results[0].is_ok());

        if let Ok(VmLine::VmStack { stack }) = &results[0] {
            assert_eq!(stack.raw(), "[ 0 NaN CS{DEAD} ]");
        } else {
            panic!("Expected VmStack variant");
        }
    }

    #[test]
    fn test_parse_large_stack_line() {
        let stack_line = "stack: [ 10000000000000 10000000000000 C{B5EE9C720101060100AB0002AF48000000000000000000000000000000000000000000000000000000000000000001001BC307F1FB14506CD271786A9FC305D70EAF063FACC22935554E47A4966051C1D8246139CA8000000000000000000000000000011901020114FF00F4A413F4BCF2C80B0300106465706C6F79657202012004050004D230005AF2D3FFED44D0D3FFD112BAF2A2F404D1F8007F8E16218010F4786FA5209802D307D43001FB009132E201B3E65B} CS{B5EE9C72010101010002000000} 0 ]";
        let results = parse_lines(&stack_line);
        assert_eq!(results.len(), 1);

        match &results[0] {
            Ok(vm_line) => {
                println!("Successfully parsed: {:?}", vm_line);
                if let VmLine::VmStack { stack } = vm_line {
                    let expected_raw = "[ 10000000000000 10000000000000 C{B5EE9C720101060100AB0002AF48000000000000000000000000000000000000000000000000000000000000000001001BC307F1FB14506CD271786A9FC305D70EAF063FACC22935554E47A4966051C1D8246139CA8000000000000000000000000000011901020114FF00F4A413F4BCF2C80B0300106465706C6F79657202012004050004D230005AF2D3FFED44D0D3FFD112BAF2A2F404D1F8007F8E16218010F4786FA5209802D307D43001FB009132E201B3E65B} CS{B5EE9C72010101010002000000} 0 ]";
                    assert_eq!(stack.raw(), expected_raw);
                } else {
                    panic!("Expected VmStack variant");
                }
            }
            Err(e) => {
                panic!("Failed to parse: {:?}", e);
            }
        }
    }
}
