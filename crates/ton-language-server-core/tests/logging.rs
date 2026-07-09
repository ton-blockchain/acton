use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
#[path = "support/snapshots.rs"]
mod snapshots;
use snapshots::assert_file_snapshot;
use ton_language_server_core::languages::tlb::LANGUAGE_ID;
use ton_language_server_core::{
    CORE_TARGET, DocumentUri, EDIT_TARGET, LogLevel, LoggingConfig, Position, Range, TextEdit,
    default_language_service,
};
use tracing::field::{Field, Visit};
use tracing::level_filters::LevelFilter;
use tracing::span::{Attributes, Id, Record};
use tracing::{Event, Level, Metadata, Subscriber};

#[test]
fn log_level_filter_exposes_debug_events() -> anyhow::Result<()> {
    let info_events = capture_logs(Level::INFO, run_logged_definition)?;
    assert!(has_operation(&info_events, "document.open"));
    assert!(has_operation(&info_events, "definition"));
    assert!(!has_operation(&info_events, "tlb.index.rebuilt"));

    let debug_events = capture_logs(Level::DEBUG, run_logged_definition)?;
    assert!(has_operation(&debug_events, "document.open"));
    assert!(has_operation(&debug_events, "tlb.index.rebuilt"));

    Ok(())
}

#[test]
fn trace_level_exposes_input_edit_events() -> anyhow::Result<()> {
    let events = capture_logs(Level::TRACE, || {
        let uri = DocumentUri::from("acton://fixture/logging-edit.tlb");
        let mut service = default_language_service();
        service.open_document(
            uri.clone(),
            LANGUAGE_ID,
            1,
            "foo$0 a:# = Old;\nbar$1 x:Old = Wrap;\n",
        )?;
        service.edit_document(&uri, 2, [TextEdit::new(range(0, 12, 0, 15), "New")])?;
        Ok(())
    })?;

    assert!(events.iter().any(|event| {
        event.target == EDIT_TARGET && event.field("operation") == Some("edit.input_edit")
    }));

    Ok(())
}

#[test]
fn trace_log_snapshot_is_file_based_and_redacted() -> anyhow::Result<()> {
    let events = capture_logs(Level::TRACE, || {
        let uri = DocumentUri::from("file:///Users/example/private-project/logging-snapshot.tlb");
        let mut service = default_language_service();
        service.open_document(
            uri.clone(),
            LANGUAGE_ID,
            1,
            "foo$0 a:# = Old;\nbar$1 x:Old = Wrap;\n",
        )?;
        service.edit_document(
            &uri,
            2,
            [
                TextEdit::new(range(0, 12, 0, 15), "New"),
                TextEdit::new(range(1, 8, 1, 11), "New"),
            ],
        )?;
        let locations = service.definition(&uri, Position::new(1, 8))?;
        assert_eq!(locations.len(), 1);
        Ok(())
    })?;

    assert_file_snapshot("logging_trace.snap", &render_log_events(&events))?;

    Ok(())
}

#[test]
fn logging_config_builds_adapter_filter_directive() -> anyhow::Result<()> {
    assert_eq!(
        LoggingConfig::new(LogLevel::Debug).filter_directive(),
        "ton_language_server_core=debug"
    );
    assert_eq!(
        LoggingConfig::for_target(EDIT_TARGET, LogLevel::Trace).filter_directive(),
        "ton_language_server_core::edit=trace"
    );
    assert_eq!("WARN".parse::<LogLevel>()?, LogLevel::Warn);
    assert_eq!(
        LogLevel::Trace.as_tracing_level_filter(),
        LevelFilter::TRACE
    );

    Ok(())
}

fn run_logged_definition() -> anyhow::Result<()> {
    let uri = DocumentUri::from("acton://fixture/logging.tlb");
    let mut service = default_language_service();
    service.open_document(
        uri.clone(),
        LANGUAGE_ID,
        1,
        "foo$0 a:# = CommonMsgInfo;\nbaz$2 x:CommonMsgInfo = Wrap;\n",
    )?;
    let locations = service.definition(&uri, Position::new(1, 8))?;
    assert_eq!(locations.len(), 1);
    Ok(())
}

fn capture_logs(
    action_level: Level,
    action: impl FnOnce() -> anyhow::Result<()>,
) -> anyhow::Result<Vec<LogEvent>> {
    let events = Arc::new(Mutex::new(Vec::new()));
    let subscriber = RecordingSubscriber::new(action_level, events.clone());
    let result = tracing::subscriber::with_default(subscriber, action);
    result?;
    let events = events
        .lock()
        .expect("log events should not be poisoned")
        .clone();
    Ok(events)
}

fn has_operation(events: &[LogEvent], operation: &str) -> bool {
    events
        .iter()
        .any(|event| event.field("operation") == Some(operation))
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct LogEvent {
    level: Level,
    target: &'static str,
    fields: BTreeMap<String, String>,
}

impl LogEvent {
    fn field(&self, name: &str) -> Option<&str> {
        self.fields.get(name).map(String::as_str)
    }
}

struct RecordingSubscriber {
    max_level: Level,
    events: Arc<Mutex<Vec<LogEvent>>>,
    next_id: AtomicU64,
}

impl RecordingSubscriber {
    const fn new(max_level: Level, events: Arc<Mutex<Vec<LogEvent>>>) -> Self {
        Self {
            max_level,
            events,
            next_id: AtomicU64::new(1),
        }
    }
}

impl Subscriber for RecordingSubscriber {
    fn enabled(&self, metadata: &Metadata<'_>) -> bool {
        metadata.target().starts_with(CORE_TARGET) && *metadata.level() <= self.max_level
    }

    fn new_span(&self, _span: &Attributes<'_>) -> Id {
        Id::from_u64(self.next_id.fetch_add(1, Ordering::Relaxed))
    }

    fn record(&self, _span: &Id, _values: &Record<'_>) {}

    fn record_follows_from(&self, _span: &Id, _follows: &Id) {}

    fn event(&self, event: &Event<'_>) {
        if !self.enabled(event.metadata()) {
            return;
        }

        let mut visitor = FieldVisitor::default();
        event.record(&mut visitor);
        self.events
            .lock()
            .expect("log events should not be poisoned")
            .push(LogEvent {
                level: *event.metadata().level(),
                target: event.metadata().target(),
                fields: visitor.fields,
            });
    }

    fn enter(&self, _span: &Id) {}

    fn exit(&self, _span: &Id) {}

    fn max_level_hint(&self) -> Option<LevelFilter> {
        Some(match self.max_level {
            Level::ERROR => LevelFilter::ERROR,
            Level::WARN => LevelFilter::WARN,
            Level::INFO => LevelFilter::INFO,
            Level::DEBUG => LevelFilter::DEBUG,
            Level::TRACE => LevelFilter::TRACE,
        })
    }
}

#[derive(Default)]
struct FieldVisitor {
    fields: BTreeMap<String, String>,
}

impl Visit for FieldVisitor {
    fn record_bool(&mut self, field: &Field, value: bool) {
        self.fields
            .insert(field.name().to_owned(), value.to_string());
    }

    fn record_i64(&mut self, field: &Field, value: i64) {
        self.fields
            .insert(field.name().to_owned(), value.to_string());
    }

    fn record_u64(&mut self, field: &Field, value: u64) {
        self.fields
            .insert(field.name().to_owned(), value.to_string());
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        self.fields
            .insert(field.name().to_owned(), value.to_owned());
    }

    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        self.fields
            .insert(field.name().to_owned(), format!("{value:?}"));
    }
}

fn render_log_events(events: &[LogEvent]) -> String {
    let mut output = String::new();
    for (idx, event) in events.iter().enumerate() {
        let _ = writeln!(
            output,
            "{idx:02} {} {}",
            render_level(event.level),
            event.target
        );
        for (name, value) in &event.fields {
            let _ = writeln!(output, "  {name}: {}", redact_field(name, value));
        }
    }
    output
}

const fn render_level(level: Level) -> &'static str {
    match level {
        Level::ERROR => "error",
        Level::WARN => "warn",
        Level::INFO => "info",
        Level::DEBUG => "debug",
        Level::TRACE => "trace",
    }
}

fn redact_field(name: &str, value: &str) -> String {
    if is_path_like_field(name) {
        redact_path_like_value(value)
    } else {
        value.to_owned()
    }
}

fn is_path_like_field(name: &str) -> bool {
    matches!(name, "uri" | "path" | "root" | "workspace_root")
        || name.ends_with("_uri")
        || name.ends_with("_path")
        || name.ends_with("_root")
}

fn redact_path_like_value(value: &str) -> String {
    if let Some(path) = value.strip_prefix("file://") {
        let file_name = path.rsplit('/').next().unwrap_or("<unknown>");
        format!("file://<redacted>/{file_name}")
    } else if value.starts_with('/') {
        let file_name = value.rsplit('/').next().unwrap_or("<unknown>");
        format!("<redacted>/{file_name}")
    } else {
        value.to_owned()
    }
}

const fn range(start_line: u32, start_character: u32, end_line: u32, end_character: u32) -> Range {
    Range::new(
        Position::new(start_line, start_character),
        Position::new(end_line, end_character),
    )
}
