use thiserror::Error;
use winnow::ascii::digit1;
use winnow::combinator::{alt, eof, not, opt, peek, terminated};
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
pub enum ExecutorLine<'a> {
    Steps {
        steps: &'a str,
        gas_used: &'a str,
        gas_max: &'a str,
        gas_limit: &'a str,
        gas_credit: &'a str,
    },
    OutOfGas {
        out_of_gas: bool,
        accepted: bool,
        success: bool,
        time: &'a str,
        cpu_time: &'a str,
    },
    GasFees {
        fees: &'a str,
        calculation: &'a str,
        price: &'a str,
        flat_rate: &'a str,
        remaining_balance: &'a str,
    },
    ProcessSendMessage {
        message_hash: &'a str,
    },
    ProcessRawReserve {
        mode: &'a str,
    },
    RemainingBalance {
        balance: &'a str,
    },
    ActionReserveCurrency {
        mode: &'a str,
        reserve: &'a str,
        balance: &'a str,
        original_balance: &'a str,
    },
    ChangedBalance {
        remaining_balance: &'a str,
        reserved_balance: &'a str,
    },
    Unknown {
        text: &'a str,
    },
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

fn until_eol<'a>(i: &mut I<'a>) -> PResult<&'a str> {
    take_while(0.., |c: char| c != '\n' && c != '\r').parse_next(i)
}

// steps: 28 gas: used=1421, max=1000000, limit=1000000, credit=0
fn executor_steps<'a>(i: &mut I<'a>) -> PResult<ExecutorLine<'a>> {
    let _ = "steps: ".parse_next(i)?;
    let steps = number.parse_next(i)?;
    let _ = " gas: used=".parse_next(i)?;
    let gas_used = number.parse_next(i)?;
    let _ = ", max=".parse_next(i)?;
    let gas_max = number.parse_next(i)?;
    let _ = ", limit=".parse_next(i)?;
    let gas_limit = number.parse_next(i)?;
    let _ = ", credit=".parse_next(i)?;
    let gas_credit = number.parse_next(i)?;
    Ok(ExecutorLine::Steps {
        steps,
        gas_used,
        gas_max,
        gas_limit,
        gas_credit,
    })
}

// out_of_gas=false, accepted=true, success=true, time=0.000000s, cpu_time=0.000000
fn executor_out_of_gas<'a>(i: &mut I<'a>) -> PResult<ExecutorLine<'a>> {
    let _ = "out_of_gas=".parse_next(i)?;
    let out_of_gas = alt(("true".value(true), "false".value(false))).parse_next(i)?;
    let _ = ", accepted=".parse_next(i)?;
    let accepted = alt(("true".value(true), "false".value(false))).parse_next(i)?;
    let _ = ", success=".parse_next(i)?;
    let success = alt(("true".value(true), "false".value(false))).parse_next(i)?;
    let _ = ", time=".parse_next(i)?;
    let time = take_while(1.., |c: char| {
        c.is_ascii_alphanumeric() || c == '.' || c == 's'
    })
    .parse_next(i)?;
    let _ = ", cpu_time=".parse_next(i)?;
    let cpu_time =
        take_while(1.., |c: char| c.is_ascii_alphanumeric() || c == '.').parse_next(i)?;
    Ok(ExecutorLine::OutOfGas {
        out_of_gas,
        accepted,
        success,
        time,
        cpu_time,
    })
}

// gas fees: 568400 = 26214400 * 1421 /2^16 ; price=26214400; flat rate=[40000 for 100]; remaining balance=998442400ng
fn executor_gas_fees<'a>(i: &mut I<'a>) -> PResult<ExecutorLine<'a>> {
    let _ = "gas fees: ".parse_next(i)?;
    let fees = number.parse_next(i)?;
    let _ = " = ".parse_next(i)?;
    let calculation = take_while(1.., |c: char| c != ';').parse_next(i)?;
    let _ = "; price=".parse_next(i)?;
    let price = number.parse_next(i)?;
    let _ = "; flat rate=[".parse_next(i)?;
    let flat_rate = take_while(1.., |c: char| c != ']').parse_next(i)?;
    let _ = "]; remaining balance=".parse_next(i)?;
    let remaining_balance = number.parse_next(i)?;
    let _ = "ng".parse_next(i)?;
    Ok(ExecutorLine::GasFees {
        fees,
        calculation,
        price,
        flat_rate,
        remaining_balance,
    })
}

// process send message 96444DE3098C2942729F6B0AD6D215138CF00724C38F3E560ED0C79D2ABF8EE7
fn executor_process_send_message<'a>(i: &mut I<'a>) -> PResult<ExecutorLine<'a>> {
    let _ = "process send message ".parse_next(i)?;
    let message_hash = take_while(1.., |c: char| c.is_ascii_hexdigit()).parse_next(i)?;
    Ok(ExecutorLine::ProcessSendMessage { message_hash })
}

// process raw reserve with mode 16
fn executor_process_raw_reserve<'a>(i: &mut I<'a>) -> PResult<ExecutorLine<'a>> {
    let _ = "process raw reserve with mode ".parse_next(i)?;
    let mode = number.parse_next(i)?;
    Ok(ExecutorLine::ProcessRawReserve { mode })
}

// remaining balance 96968400ng
fn executor_remaining_balance<'a>(i: &mut I<'a>) -> PResult<ExecutorLine<'a>> {
    let _ = "remaining balance ".parse_next(i)?;
    let balance = number.parse_next(i)?;
    let _ = "ng".parse_next(i)?;
    Ok(ExecutorLine::RemainingBalance { balance })
}

// action_reserve_currency: mode=0, reserve=10000ng, balance=96334000ng, original balance=999753200ng
fn executor_action_reserve_currency<'a>(i: &mut I<'a>) -> PResult<ExecutorLine<'a>> {
    let _ = "action_reserve_currency: mode=".parse_next(i)?;
    let mode = number.parse_next(i)?;
    let _ = ", reserve=".parse_next(i)?;
    let reserve = number.parse_next(i)?;
    let _ = "ng, balance=".parse_next(i)?;
    let balance = number.parse_next(i)?;
    let _ = "ng, original balance=".parse_next(i)?;
    let original_balance = number.parse_next(i)?;
    let _ = "ng".parse_next(i)?;
    Ok(ExecutorLine::ActionReserveCurrency {
        mode,
        reserve,
        balance,
        original_balance,
    })
}

// changed remaining balance to 96324000ng, reserved balance to 10000ng
fn executor_changed_balance<'a>(i: &mut I<'a>) -> PResult<ExecutorLine<'a>> {
    let _ = "changed remaining balance to ".parse_next(i)?;
    let remaining_balance = number.parse_next(i)?;
    let _ = "ng, reserved balance to ".parse_next(i)?;
    let reserved_balance = number.parse_next(i)?;
    let _ = "ng".parse_next(i)?;
    Ok(ExecutorLine::ChangedBalance {
        remaining_balance,
        reserved_balance,
    })
}

// Unknown lines
fn executor_unknown<'a>(i: &mut I<'a>) -> PResult<ExecutorLine<'a>> {
    // not(peek(alt(...)))
    not(peek(alt((
        "steps: ",
        "out_of_gas=",
        "gas fees: ",
        "process send message ",
        "process raw reserve with mode ",
        "remaining balance ",
        "action_reserve_currency: mode=",
        "changed remaining balance to ",
    ))))
    .parse_next(i)?;
    let t = until_eol.parse_next(i)?;
    Ok(ExecutorLine::Unknown { text: t.trim() })
}

pub fn executor_line<'a>(i: &mut I<'a>) -> PResult<ExecutorLine<'a>> {
    alt((
        executor_steps,
        executor_out_of_gas,
        executor_gas_fees,
        executor_process_send_message,
        executor_process_raw_reserve,
        executor_remaining_balance,
        executor_action_reserve_currency,
        executor_changed_balance,
        executor_unknown,
    ))
    .parse_next(i)
}

// Parse executor log line, skipping the prefix before "     "
pub fn parse_executor_line(input: &str) -> Result<ExecutorLine, String> {
    let parts: Vec<&str> = input.splitn(2, "	").collect();
    if parts.len() != 2 {
        return Err(format!("Invalid executor log format: {}", input));
    }
    let message_part = parts[1];

    match terminated(executor_line, opt(eof)).parse(&mut message_part.as_ref()) {
        Ok(v) => Ok(v),
        Err(e) => Err(format!("{e:?} @ {:?}", message_part)),
    }
}

pub fn parse_executor_lines(input: &str) -> Vec<Result<ExecutorLine, String>> {
    input
        .split_inclusive('\n')
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            let s = line.trim_end_matches(['\r', '\n', ' '].as_ref());
            parse_executor_line(s)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_EXECUTOR_LOGS: &str = r#"[ 4][t 0][2025-11-04 08:57:12.814271][transaction.cpp:1948]	steps: 28 gas: used=1421, max=1000000, limit=1000000, credit=0
[ 4][t 0][2025-11-04 08:57:12.814271][transaction.cpp:1948]	out_of_gas=false, accepted=true, success=true, time=0.000000s, cpu_time=0.000000
[ 4][t 0][2025-11-04 08:57:12.814271][transaction.cpp:1948]	gas fees: 568400 = 26214400 * 1421 /2^16 ; price=26214400; flat rate=[40000 for 100]; remaining balance=998442400ng
[ 4][t 0][2025-11-04 08:57:12.814271][transaction.cpp:1948]	process send message 96444DE3098C2942729F6B0AD6D215138CF00724C38F3E560ED0C79D2ABF8EE7
[ 4][t 0][2025-11-04 08:57:12.814271][transaction.cpp:1948]	process raw reserve with mode 16
[ 4][t 0][2025-11-04 08:57:12.814271][transaction.cpp:1948]	remaining balance 96968400ng
[ 4][t 0][2025-11-04 08:57:12.814271][transaction.cpp:1948]	action_reserve_currency: mode=0, reserve=10000ng, balance=96334000ng, original balance=999753200ng
[ 4][t 0][2025-11-04 08:57:12.814271][transaction.cpp:1948]	changed remaining balance to 96324000ng, reserved balance to 10000ng
[ 4][t 0][2025-11-04 08:57:12.814271][transaction.cpp:1948]	some unknown message here"#;

    #[test]
    fn test_parse_executor_logs() {
        let results = parse_executor_lines(&TEST_EXECUTOR_LOGS);

        for (i, result) in results.iter().enumerate() {
            match result {
                Ok(executor_line) => {
                    println!("Line {}: Successfully parsed {:?}", i + 1, executor_line);
                }
                Err(e) => {
                    panic!("Failed to parse line {}: {}", i + 1, e);
                }
            }
        }

        assert_eq!(results.len(), 9, "Expected 9 log lines");

        let success_count = results.iter().filter(|r| r.is_ok()).count();
        assert_eq!(success_count, 9, "All lines should parse successfully");
    }

    #[test]
    fn test_parse_steps_line() {
        let line = "[ 4][t 0][2025-11-04 08:57:12.814271][transaction.cpp:1948]\tsteps: 28 gas: used=1421, max=1000000, limit=1000000, credit=0";
        let result = parse_executor_line(&line);
        assert!(result.is_ok());

        if let Ok(ExecutorLine::Steps {
            steps,
            gas_used,
            gas_max,
            gas_limit,
            gas_credit,
        }) = result
        {
            assert_eq!(steps, "28");
            assert_eq!(gas_used, "1421");
            assert_eq!(gas_max, "1000000");
            assert_eq!(gas_limit, "1000000");
            assert_eq!(gas_credit, "0");
        } else {
            panic!("Expected Steps variant");
        }
    }

    #[test]
    fn test_parse_out_of_gas_line() {
        let line = "[ 4][t 0][2025-11-04 08:57:12.814271][transaction.cpp:1948]\tout_of_gas=false, accepted=true, success=true, time=0.000000s, cpu_time=0.000000";
        let result = parse_executor_line(&line);
        assert!(result.is_ok());

        if let Ok(ExecutorLine::OutOfGas {
            out_of_gas,
            accepted,
            success,
            time,
            cpu_time,
        }) = result
        {
            assert_eq!(out_of_gas, false);
            assert_eq!(accepted, true);
            assert_eq!(success, true);
            assert_eq!(time, "0.000000s");
            assert_eq!(cpu_time, "0.000000");
        } else {
            panic!("Expected OutOfGas variant");
        }
    }

    #[test]
    fn test_parse_gas_fees_line() {
        let line = "[ 4][t 0][2025-11-04 08:57:12.814271][transaction.cpp:1948]\tgas fees: 568400 = 26214400 * 1421 /2^16 ; price=26214400; flat rate=[40000 for 100]; remaining balance=998442400ng";
        let result = parse_executor_line(&line);
        assert!(result.is_ok());

        if let Ok(ExecutorLine::GasFees {
            fees,
            calculation,
            price,
            flat_rate,
            remaining_balance,
        }) = result
        {
            assert_eq!(fees, "568400");
            assert_eq!(calculation, "26214400 * 1421 /2^16 ");
            assert_eq!(price, "26214400");
            assert_eq!(flat_rate, "40000 for 100");
            assert_eq!(remaining_balance, "998442400");
        } else {
            panic!("Expected GasFees variant");
        }
    }

    #[test]
    fn test_parse_process_send_message_line() {
        let line = "[ 4][t 0][2025-11-04 08:57:12.814271][transaction.cpp:1948]\tprocess send message 96444DE3098C2942729F6B0AD6D215138CF00724C38F3E560ED0C79D2ABF8EE7";
        let result = parse_executor_line(&line);
        assert!(result.is_ok());

        if let Ok(ExecutorLine::ProcessSendMessage { message_hash }) = result {
            assert_eq!(
                message_hash,
                "96444DE3098C2942729F6B0AD6D215138CF00724C38F3E560ED0C79D2ABF8EE7"
            );
        } else {
            panic!("Expected ProcessSendMessage variant");
        }
    }

    #[test]
    fn test_parse_process_raw_reserve_line() {
        let line = "[ 4][t 0][2025-11-04 08:57:12.814271][transaction.cpp:1948]\tprocess raw reserve with mode 16";
        let result = parse_executor_line(&line);
        assert!(result.is_ok());

        if let Ok(ExecutorLine::ProcessRawReserve { mode }) = result {
            assert_eq!(mode, "16");
        } else {
            panic!("Expected ProcessRawReserve variant");
        }
    }

    #[test]
    fn test_parse_remaining_balance_line() {
        let line = "[ 4][t 0][2025-11-04 08:57:12.814271][transaction.cpp:1948]\tremaining balance 96968400ng";
        let result = parse_executor_line(&line);
        assert!(result.is_ok());

        if let Ok(ExecutorLine::RemainingBalance { balance }) = result {
            assert_eq!(balance, "96968400");
        } else {
            panic!("Expected RemainingBalance variant");
        }
    }

    #[test]
    fn test_parse_action_reserve_currency_line() {
        let line = "[ 4][t 0][2025-11-04 08:57:12.814271][transaction.cpp:1948]\taction_reserve_currency: mode=0, reserve=10000ng, balance=96334000ng, original balance=999753200ng";
        let result = parse_executor_line(&line);
        assert!(result.is_ok());

        if let Ok(ExecutorLine::ActionReserveCurrency {
            mode,
            reserve,
            balance,
            original_balance,
        }) = result
        {
            assert_eq!(mode, "0");
            assert_eq!(reserve, "10000");
            assert_eq!(balance, "96334000");
            assert_eq!(original_balance, "999753200");
        } else {
            panic!("Expected ActionReserveCurrency variant");
        }
    }

    #[test]
    fn test_parse_changed_balance_line() {
        let line = "[ 4][t 0][2025-11-04 08:57:12.814271][transaction.cpp:1948]\tchanged remaining balance to 96324000ng, reserved balance to 10000ng";
        let result = parse_executor_line(&line);
        assert!(result.is_ok());

        if let Ok(ExecutorLine::ChangedBalance {
            remaining_balance,
            reserved_balance,
        }) = result
        {
            assert_eq!(remaining_balance, "96324000");
            assert_eq!(reserved_balance, "10000");
        } else {
            panic!("Expected ChangedBalance variant");
        }
    }

    #[test]
    fn test_parse_unknown_line() {
        let line = "[ 4][t 0][2025-11-04 08:57:12.814271][transaction.cpp:1948]\tsome unknown message here";
        let result = parse_executor_line(&line);
        assert!(result.is_ok());

        if let Ok(ExecutorLine::Unknown { text }) = result {
            assert_eq!(text, "some unknown message here");
        } else {
            panic!("Expected Unknown variant");
        }
    }
}
