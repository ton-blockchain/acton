use std::path::PathBuf;

#[macro_export]
macro_rules! regex {
    ($re:literal $(,)?) => {{
        static RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
        RE.get_or_init(|| regex::Regex::new($re).unwrap())
    }};
}

static MIN_LITERAL_REDACTIONS: &[(&str, &str)] = &[
    ("[EXE]", std::env::consts::EXE_SUFFIX),
    ("[BROKEN_PIPE]", "Broken pipe (os error 32)"),
    ("[BROKEN_PIPE]", "The pipe is being closed. (os error 232)"),
    ("[NOT_FOUND]", "No such file or directory (os error 2)"),
    (
        "[NOT_FOUND]",
        "The system cannot find the file specified. (os error 2)",
    ),
    (
        "[NOT_FOUND]",
        "The system cannot find the path specified. (os error 3)",
    ),
    ("[NOT_FOUND]", "Access is denied. (os error 5)"),
    ("[NOT_FOUND]", "program not found"),
    ("[EXIT_STATUS]", "exit status"),
    ("[EXIT_STATUS]", "exit code"),
];

pub fn assert_ui() -> snapbox::Assert {
    let mut subs = snapbox::Redactions::new();
    subs.extend(MIN_LITERAL_REDACTIONS.into_iter().cloned())
        .unwrap();
    add_regex_redactions(&mut subs);

    snapbox::Assert::new()
        .action_env(snapbox::assert::DEFAULT_ACTION_ENV)
        .redact_with(subs)
}

fn add_regex_redactions(subs: &mut snapbox::Redactions) {
    subs.insert("[TIME]", regex!(r"(\d+\.)?\d+ms")).unwrap();
    subs.insert("[TIME]", regex!(r"(\d+\.)?\d+µs")).unwrap();
    subs.insert("[LINE]", regex!(r"(\.tolk):\d+:\d+")).unwrap();
}

pub fn acton_exe() -> PathBuf {
    snapbox::cmd::cargo_bin!("acton").to_path_buf()
}

pub trait ActonCommandExt {
    fn acton_ui() -> Self;
}

impl ActonCommandExt for snapbox::cmd::Command {
    fn acton_ui() -> Self {
        Self::new(acton_exe()).with_assert(assert_ui())
    }
}

pub fn strip_ansi(s: &str) -> String {
    let bytes = strip_ansi_escapes::strip(s.as_bytes());
    String::from_utf8(bytes).unwrap()
}

pub fn assertion() -> snapbox::Assert {
    snapbox::Assert::new().action_env(snapbox::assert::DEFAULT_ACTION_ENV)
}
