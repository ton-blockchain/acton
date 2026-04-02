use dap::types::ExceptionDetails;

use crate::exit_codes;

use super::replayer::ExceptionInfo;

pub(crate) struct ExceptionOverview {
    pub stop_description: String,
    pub stop_text: String,
    pub info_description: String,
    pub type_name: String,
    pub full_type_name: String,
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

    let summary = match exc.errno.parse::<i32>().ok().and_then(exit_codes::find) {
        Some(info) => format!("{kind} {} ({})", exc.errno, info.name),
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
        && let Some(info) = exit_codes::find(code)
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
