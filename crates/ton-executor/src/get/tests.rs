#![cfg(test)]

use crate::common::ExecutorVerbosity;
use crate::get::step::StepGetExecutor;
use crate::get::{GetExecutor, GetMethodResult, GetMethodResultSuccess, RunGetMethodArgs};
use std::sync::Arc;

#[test]
fn test_get_executor_new() -> anyhow::Result<()> {
    let code: Arc<str> = "te6ccgECEgEAA5cAART/APSkE/S88sgLAQIBYgIDAgLOBAUC+aFKdRbobe6tzoyuURoJubS2uDYyr7G3urc6MrkXOje2NcH4AAAAAAzhkZZ/kqYDkZ8JoZmZ8izYQ5GfFACBl/+eoRcMjK4Nje8srlEQQ5GdkxoQwAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAACKYlkZ8JoZmZDgYCASAHCAAdTwATRbUIlfCMAAkjBw4IA/z5FsjPigBAy//PUCDI+lLJUAX8AAAAAA+CIAkYTnKgAMjPiQgBUzTIz4TQzMz5Fs8L/wH6AoEAjM8LcBPMzMlxAsj6UslY/AAAAAAJMIIZAuhdcACLh0cmVhc3VyeYiCHIzsmJUxLIz4TQzMz5FsjPigBAy//PUCDI+lLJUAUOCQoAASAB9zI+lLJ/AAAAAARIG6eMG1tbW1tbW1tbW1tbXDg0NcsBvK/+kjTAgGqAtcB0wIBqgLXAdcsCICUbYEArp7XLAmAkvI/4dP/AYEAr+IC0x/XLAaV+gCBAIyc1ywCkvI/4W0BgQCF4gHTP/oA9ATXLAGUMIEApuMOEHiBALCALAEOAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAQAfz8AAAAAA+CIAkYTnKgAMjPiQgBUzTIz4TQzMz5Fs8L/wH6AoEAjM8LcBPMzMlxAsj6UslY/AAAAAAJMMjPhQhSQPpSWPoCcM8LaslwAsj6UslY/AAAAAAJMItns6dG9ufYIvACi1Y29pbnOAFvAfwAAAAAyItXNsaWNlgBbwEMAEDXLAOUMIEAp44V1ywFlDCBAKib1ywHMZLyP+GBAKni4gH+/AAAAAABghAF9eEAyM+QAAAABsnIz4WIFPpSAfoCcc8LahLMyXECyPpSyVj8AAAAAAmNDYvVXNlcnMvcGV0cm1ha2huZXYvZW11bGF0b3ItcnMvY291bnRlci50ZXN0LnRvbGs6MTAyOjWBtbW1tf21tbW1tVHmHVHmHVHaYKg0AqG8KUsD8AAAAAApujkQQiRB5EGkQWRBJEDlQko0JWV4cGVjdCg8YWN0dWFsPikudG9IYXZlVHgoPGV4cGVjdGVkPimAKbwpBMPwAAAAAZ/LCN+BfDAEU/wD0pBP0vPLICw8CASAQEQAE0jAAWvLT/+1E0NP/0RK68qL0BNH4AH+OFiGAEPR4b6UgmALTB9QwAfsAkTLiAbPmWw==".into();

    let params = RunGetMethodArgs {
        code: code.to_string(),
        address: "EQC2jeGorIAFh2LXwsDjHfRK-GSo9UzchdIEMh24A7T7AHot".to_string(),
        verbosity: ExecutorVerbosity::Short,
        method_id: 0,
        ..Default::default()
    };

    let exec = GetExecutor::new(&params)?;
    let res = exec.run_get_method("te6ccgEBAQEABQAABgAAAA==", &params, None)?;

    assert_eq!(
        res,
        GetMethodResult::Success(GetMethodResultSuccess {
            success: true,
            stack: "te6cckEBAQEABQAABgAAANAJX0U=".into(),
            gas_used: "491".to_owned(),
            vm_exit_code: 0,
            vm_log: "execute SETCP 0\nexecute DICTPUSHCONST 19 (xC_,1)\nexecute DICTIGETJMPZ\nexecute implicit RET\n".into(),
            missing_library: None,
            code,
        })
    );

    Ok(())
}

#[test]
fn test_step_get_executor_run() -> anyhow::Result<()> {
    let code = "te6ccgECEgEAA5cAART/APSkE/S88sgLAQIBYgIDAgLOBAUC+aFKdRbobe6tzoyuURoJubS2uDYyr7G3urc6MrkXOje2NcH4AAAAAAzhkZZ/kqYDkZ8JoZmZ8izYQ5GfFACBl/+eoRcMjK4Nje8srlEQQ5GdkxoQwAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAACKYlkZ8JoZmZDgYCASAHCAAdTwATRbUIlfCMAAkjBw4IA/z5FsjPigBAy//PUCDI+lLJUAX8AAAAAA+CIAkYTnKgAMjPiQgBUzTIz4TQzMz5Fs8L/wH6AoEAjM8LcBPMzMlxAsj6UslY/AAAAAAJMIIZAuhdcACLh0cmVhc3VyeYiCHIzsmJUxLIz4TQzMz5FsjPigBAy//PUCDI+lLJUAUOCQoAASAB9zI+lLJ/AAAAAARIG6eMG1tbW1tbW1tbW1tbXDg0NcsBvK/+kjTAgGqAtcB0wIBqgLXAdcsCICUbYEArp7XLAmAkvI/4dP/AYEAr+IC0x/XLAaV+gCBAIyc1ywCkvI/4W0BgQCF4gHTP/oA9ATXLAGUMIEApuMOEHiBALCALAEOAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAQAfz8AAAAAA+CIAkYTnKgAMjPiQgBUzTIz4TQzMz5Fs8L/wH6AoEAjM8LcBPMzMlxAsj6UslY/AAAAAAJMMjPhQhSQPpSWPoCcM8LaslwAsj6UslY/AAAAAAJMItns6dG9ufYIvACi1Y29pbnOAFvAfwAAAAAyItXNsaWNlgBbwEMAEDXLAOUMIEAp44V1ywFlDCBAKib1ywHMZLyP+GBAKni4gH+/AAAAAABghAF9eEAyM+QAAAABsnIz4WIFPpSAfoCcc8LahLMyXECyPpSyVj8AAAAAAmNDYvVXNlcnMvcGV0cm1ha2huZXYvZW11bGF0b3ItcnMvY291bnRlci50ZXN0LnRvbGs6MTAyOjWBtbW1tf21tbW1tVHmHVHmHVHaYKg0AqG8KUsD8AAAAAApujkQQiRB5EGkQWRBJEDlQko0JWV4cGVjdCg8YWN0dWFsPikudG9IYXZlVHgoPGV4cGVjdGVkPimAKbwpBMPwAAAAAZ/LCN+BfDAEU/wD0pBP0vPLICw8CASAQEQAE0jAAWvLT/+1E0NP/0RK68qL0BNH4AH+OFiGAEPR4b6UgmALTB9QwAfsAkTLiAbPmWw==".to_owned();

    let params = RunGetMethodArgs {
        code: code.clone(),
        address: "EQC2jeGorIAFh2LXwsDjHfRK-GSo9UzchdIEMh24A7T7AHot".to_string(),
        verbosity: ExecutorVerbosity::Short,
        method_id: 0,
        ..Default::default()
    };

    let stack_b64 = "te6ccgEBAQEABQAABgAAAA==";
    let exec = StepGetExecutor::new(stack_b64, &params, None)?;

    exec.prepare(0, stack_b64)?;

    let mut steps = 0;
    while !exec.step() {
        steps += 1;
        let _pos = exec.get_code_pos();
        let _stack = exec.get_stack();
    }
    assert!(steps > 0);

    let res = exec.finish(&code)?;
    assert!(matches!(res, GetMethodResult::Success(_)));

    Ok(())
}
