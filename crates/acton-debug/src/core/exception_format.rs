use dap::types::ExceptionDetails;

use crate::exit_codes;
use crate::exit_codes::ExitCodePhase;

use super::replayer::ExceptionInfo;

pub(crate) struct ExceptionOverview {
    pub stop_description: String,
    pub stop_text: String,
    pub info_description: String,
    pub type_name: String,
    pub full_type_name: String,
}

fn exception_display_name(exc: &ExceptionInfo) -> Option<&str> {
    exc.symbolic_name.as_deref().or_else(|| {
        exc.errno
            .parse::<i32>()
            .ok()
            .and_then(|code| exit_codes::find_for_phase(code, ExitCodePhase::Compute))
            .map(|info| info.name)
    })
}

pub(crate) fn exception_overview(exc: &ExceptionInfo) -> ExceptionOverview {
    let kind = if exc.is_uncaught {
        "Uncaught TVM exception"
    } else {
        "TVM exception"
    };
    let stop_description = if exc.is_uncaught {
        "Paused on uncaught exception"
    } else {
        "Paused on exception"
    };
    let type_name = "TVMException".to_string();
    let full_type_name = if exc.is_uncaught {
        "TVM.UncaughtException"
    } else {
        "TVM.Exception"
    }
    .to_string();

    let summary = match exception_display_name(exc) {
        Some(name) => format!("{kind} {} ({name})", exc.errno),
        None => format!("{kind} {}", exc.errno),
    };

    ExceptionOverview {
        stop_description: stop_description.to_string(),
        stop_text: summary.clone(),
        info_description: summary,
        type_name,
        full_type_name,
    }
}

pub(crate) fn build_exception_details(exc: &ExceptionInfo) -> ExceptionDetails {
    let overview = exception_overview(exc);
    let mut message_lines = vec![overview.info_description.clone()];

    if let Some(code) = exc.errno.parse::<i32>().ok()
        && let Some(info) = exit_codes::find_for_phase(code, ExitCodePhase::Compute)
    {
        message_lines.push(info.description.to_string());
        message_lines.push(format!("Phase: {}", info.phase));
    }

    ExceptionDetails {
        message: Some(message_lines.join("\n")),
        type_name: Some(overview.type_name),
        full_type_name: Some(overview.full_type_name),
        evaluate_name: None,
        stack_trace: None,
        inner_exception: None,
    }
}

#[cfg(test)]
mod tests {
    use super::{build_exception_details, exception_overview};
    use crate::core::replayer::ExceptionInfo;

    #[test]
    fn custom_symbolic_name_takes_priority_in_exception_overview() {
        let exc = ExceptionInfo {
            errno: "402".to_owned(),
            symbolic_name: Some("Errors.NotEnoughTon".to_owned()),
            is_uncaught: true,
        };

        let overview = exception_overview(&exc);
        assert_eq!(
            overview.stop_text,
            "Uncaught TVM exception 402 (Errors.NotEnoughTon)"
        );

        let details = build_exception_details(&exc);
        let message = details.message.expect("message");
        assert!(message.contains("Uncaught TVM exception 402 (Errors.NotEnoughTon)"));
    }

    #[test]
    fn standard_exit_code_name_is_used_as_fallback() {
        let exc = ExceptionInfo {
            errno: "7".to_owned(),
            symbolic_name: None,
            is_uncaught: false,
        };

        let overview = exception_overview(&exc);
        assert_eq!(overview.stop_text, "TVM exception 7 (Type Check Error)");

        let details = build_exception_details(&exc);
        let message = details.message.expect("message");
        assert!(message.contains("Type check error"));
        assert!(message.contains("Phase: Compute phase"));
    }

    #[test]
    fn action_phase_exit_code_is_not_used_as_compute_exception_fallback() {
        let exc = ExceptionInfo {
            errno: "32".to_owned(),
            symbolic_name: None,
            is_uncaught: true,
        };

        let overview = exception_overview(&exc);
        assert_eq!(overview.stop_text, "Uncaught TVM exception 32");

        let details = build_exception_details(&exc);
        let message = details.message.expect("message");
        assert!(message.contains("Uncaught TVM exception 32"));
        assert!(!message.contains("Action list is invalid"));
        assert!(!message.contains("Phase: Action phase"));
    }
}
