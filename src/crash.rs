use crash_handler::{CrashHandler, Error};

const ACTON_FATAL_CRASH_MESSAGE: &str = concat!(
    "\n",
    "Acton hit an internal crash and had to stop.\n",
    "Please run the same command again. If it crashes again, report it at https://github.com/i582/acton/issues.\n",
    "Include the command you ran, the files or project involved, what Acton printed before the crash, and the steps to reproduce it.\n",
    "Check the Acton log file for more details: ~/.acton/logs/debug.log.\n",
    "If ACTON_LOG_DIR is set, check that directory instead.\n",
    "Version: v",
    env!("ACTON_LONG_VERSION"),
    "\n",
);

pub fn install() -> Result<CrashHandler, Error> {
    #[allow(unsafe_code)]
    // SAFETY: the crash callback only writes a static message with crash-handler's
    // low-level stderr helper and then lets the default crash handling continue.
    let crash_event = unsafe {
        crash_handler::make_single_crash_event(|_| {
            crash_handler::write_stderr(ACTON_FATAL_CRASH_MESSAGE);
            crash_handler::CrashEventResult::Handled(false)
        })
    };

    CrashHandler::attach(crash_event)
}
