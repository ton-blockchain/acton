use acton_config::color::{ColorMode, color_mode};
use codespan_reporting::diagnostic::{Diagnostic, Label, Severity};
use codespan_reporting::files::SimpleFiles;
use codespan_reporting::term::{
    self,
    termcolor::{Color, ColorChoice, StandardStream},
};
use std::collections::HashMap;
use tolk_linter::diagnostic::Diagnostic as LinterDiagnostic;
use tolk_resolver::FileDb;

pub(super) fn emit_diagnostics(
    file_db: &FileDb,
    diagnostics: &[LinterDiagnostic],
) -> anyhow::Result<()> {
    let mut files = SimpleFiles::new();
    let mut file_id_map = HashMap::new();

    for info in file_db.iter() {
        let cs_file_id = files.add(
            info.index().path.to_string_lossy().to_string(),
            info.source().source.as_ref().to_owned(),
        );
        file_id_map.insert(info.id(), cs_file_id);
    }

    let writer = StandardStream::stderr(match color_mode() {
        ColorMode::Auto => ColorChoice::Auto,
        ColorMode::Always => ColorChoice::Always,
        ColorMode::Never => ColorChoice::Never,
    });
    let mut config = term::Config::default();

    let mut styles = term::Styles::default();
    styles.header_error.set_intense(false);
    styles.header_warning.set_intense(false);
    styles
        .header_help
        .set_fg(Some(Color::Green))
        .set_intense(false);
    styles
        .primary_label_help
        .set_fg(Some(Color::Green))
        .set_intense(false);

    config.styles = styles;

    config.chars = term::Chars::ascii();

    for diag in diagnostics {
        let severity = match diag.severity {
            tolk_linter::diagnostic::Severity::Info => Severity::Note,
            tolk_linter::diagnostic::Severity::Warning => Severity::Warning,
            tolk_linter::diagnostic::Severity::Error => Severity::Error,
            tolk_linter::diagnostic::Severity::Fatal => Severity::Bug,
            tolk_linter::diagnostic::Severity::Help => Severity::Help,
        };

        let mut cs_diag = Diagnostic::new(severity).with_message(&diag.message);
        if let Some(code) = &diag.code {
            cs_diag = cs_diag.with_code(code);
        }

        if let Some(help) = &diag.help {
            cs_diag = cs_diag.with_notes(vec![help.clone()]);
        }

        let mut labels = vec![];
        for anno in &diag.annotations {
            let cs_file_id = *file_id_map.get(&diag.file_id).unwrap_or(&0);
            let mut label = if anno.is_primary {
                Label::primary(cs_file_id, anno.span.start()..anno.span.end())
            } else {
                Label::secondary(cs_file_id, anno.span.start()..anno.span.end())
            };
            if let Some(msg) = &anno.message {
                label = label.with_message(msg);
            }
            labels.push(label);
        }
        cs_diag = cs_diag.with_labels(labels);

        // this is diagnostic header printed, with yellow
        term::emit(&mut writer.lock(), &config, &files, &cs_diag)?;

        for fix in &diag.fixes {
            let mut labels = vec![];
            // print edit message (help in green) per edit
            for edit in &fix.edits {
                let edit_file_id = edit.file_id;
                let cs_file_id = *file_id_map.get(&edit_file_id).unwrap_or(&0);
                labels.push(
                    Label::primary(cs_file_id, edit.span.start()..edit.span.end())
                        .with_message(&edit.replacement),
                );
            }
            let fix_diag = Diagnostic::new(Severity::Help)
                .with_message(&fix.message)
                .with_labels(labels);
            term::emit(&mut writer.lock(), &config, &files, &fix_diag)?;
        }
    }

    Ok(())
}
