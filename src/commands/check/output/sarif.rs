use crate::commands::check::pos;
use serde_json::json;
use serde_sarif::sarif;
use std::collections::{BTreeMap, HashMap};
use std::io::Write;
use std::path::{Path, PathBuf};
use tolk_linter::diagnostic::{Annotation, Applicability, Diagnostic, DiagnosticTag, Severity};
use tolk_resolver::{FileDb, Span};

const DOCS_BASE_URL: &str = "https://i582.github.io/acton/docs";
const SOURCE_ROOT_URI_BASE_ID: &str = "SRCROOT";

pub(crate) fn write_report(
    writer: &mut dyn Write,
    diagnostics: &[Diagnostic],
    file_db: &FileDb,
    project_root: &Path,
) -> anyhow::Result<()> {
    let report = diagnostics_to_sarif(diagnostics, file_db, project_root)?;
    let json = serde_json::to_string_pretty(&report)?;

    writer.write_all(json.as_bytes())?;
    Ok(())
}

fn diagnostics_to_sarif(
    diagnostics: &[Diagnostic],
    file_db: &FileDb,
    project_root: &Path,
) -> anyhow::Result<sarif::Sarif> {
    let mut sorted_diagnostics = diagnostics.to_vec();
    sorted_diagnostics.sort();

    let mut rules_by_id = BTreeMap::<String, sarif::ReportingDescriptor>::new();
    for diagnostic in &sorted_diagnostics {
        let rule_id = diagnostic_rule_id(diagnostic);
        rules_by_id
            .entry(rule_id.clone())
            .or_insert_with(|| diagnostic_to_rule_descriptor(diagnostic, rule_id));
    }

    let rules = rules_by_id.into_values().collect::<Vec<_>>();
    let rule_indices = rules
        .iter()
        .enumerate()
        .map(|(idx, rule)| (rule.id.clone(), idx as i64))
        .collect::<HashMap<_, _>>();

    let results = sorted_diagnostics
        .iter()
        .map(|diagnostic| diagnostic_to_result(diagnostic, file_db, project_root, &rule_indices))
        .collect::<Vec<_>>();

    let mut original_uri_base_ids = BTreeMap::new();
    original_uri_base_ids.insert(
        SOURCE_ROOT_URI_BASE_ID.to_string(),
        sarif::ArtifactLocation {
            description: None,
            index: None,
            properties: None,
            uri: Some(path_to_sarif_uri(project_root, true)),
            uri_base_id: None,
        },
    );

    Ok(sarif::Sarif {
        schema: Some(sarif::SCHEMA_URL.to_string()),
        inline_external_properties: None,
        properties: None,
        runs: vec![sarif::Run {
            addresses: None,
            artifacts: None,
            automation_details: None,
            baseline_guid: None,
            column_kind: None,
            conversion: None,
            default_encoding: None,
            default_source_language: None,
            external_property_file_references: None,
            graphs: None,
            invocations: None,
            language: None,
            logical_locations: None,
            newline_sequences: None,
            original_uri_base_ids: Some(original_uri_base_ids),
            policies: None,
            properties: None,
            redaction_tokens: None,
            results: Some(results),
            run_aggregates: None,
            special_locations: None,
            taxonomies: None,
            thread_flow_locations: None,
            tool: sarif::Tool {
                driver: sarif::ToolComponent {
                    associated_component: None,
                    contents: None,
                    dotted_quad_file_version: None,
                    download_uri: None,
                    full_description: None,
                    full_name: None,
                    global_message_strings: None,
                    guid: None,
                    information_uri: Some(format!("{DOCS_BASE_URL}/commands/check")),
                    is_comprehensive: None,
                    language: None,
                    localized_data_semantic_version: None,
                    locations: None,
                    minimum_required_localized_data_semantic_version: None,
                    name: "acton check".to_string(),
                    notifications: None,
                    organization: None,
                    product: None,
                    product_suite: None,
                    properties: None,
                    release_date_utc: None,
                    rules: if rules.is_empty() { None } else { Some(rules) },
                    semantic_version: None,
                    short_description: None,
                    supported_taxonomies: None,
                    taxa: None,
                    translation_metadata: None,
                    version: None,
                },
                extensions: None,
                properties: None,
            },
            translations: None,
            version_control_provenance: None,
            web_requests: None,
            web_responses: None,
        }],
        version: serde_json::to_value(sarif::Version::V2_1_0)?,
    })
}

fn diagnostic_to_result(
    diagnostic: &Diagnostic,
    file_db: &FileDb,
    project_root: &Path,
    rule_indices: &HashMap<String, i64>,
) -> sarif::Result {
    let rule_id = diagnostic_rule_id(diagnostic);
    let locations = diagnostic_locations(diagnostic, file_db, project_root);

    sarif::Result {
        analysis_target: None,
        attachments: None,
        baseline_state: None,
        code_flows: None,
        correlation_guid: None,
        fingerprints: None,
        fixes: diagnostic_fixes(diagnostic, file_db, project_root),
        graph_traversals: None,
        graphs: None,
        guid: None,
        hosted_viewer_uri: None,
        kind: None,
        level: Some(severity_to_sarif_level(diagnostic.severity)),
        locations: locations
            .primary
            .clone()
            .map(|primary_location| vec![primary_location]),
        message: sarif::Message {
            arguments: None,
            id: None,
            markdown: Some(diagnostic.message.clone()),
            properties: None,
            text: Some(diagnostic.message.clone()),
        },
        occurrence_count: None,
        partial_fingerprints: None,
        properties: diagnostic_result_properties(diagnostic),
        provenance: None,
        rank: None,
        related_locations: locations.related,
        rule: None,
        rule_id: Some(rule_id.clone()),
        rule_index: rule_indices.get(&rule_id).copied(),
        stacks: None,
        suppressions: None,
        taxa: None,
        web_request: None,
        web_response: None,
        work_item_uris: None,
    }
}

fn diagnostic_to_rule_descriptor(
    diagnostic: &Diagnostic,
    rule_id: String,
) -> sarif::ReportingDescriptor {
    let explanation_markdown = diagnostic.rule.explanation().map(str::to_string);
    let full_description_markdown = explanation_markdown.clone();
    let explanation_text = explanation_markdown
        .as_deref()
        .map(markdown_to_plain_text)
        .unwrap_or_default();
    let short_description_text = format!("{rule_id}: {}", diagnostic.message);
    let full_description_text = if explanation_text.trim().is_empty() {
        diagnostic.message.clone()
    } else {
        truncate_for_github_description(&explanation_text)
    };
    let (group_status, group_since) = rule_group_status_and_since(diagnostic.rule.group());

    sarif::ReportingDescriptor {
        default_configuration: None,
        deprecated_guids: None,
        deprecated_ids: None,
        deprecated_names: None,
        full_description: Some(sarif::MultiformatMessageString {
            markdown: full_description_markdown,
            properties: None,
            text: full_description_text,
        }),
        guid: None,
        help: explanation_markdown.map(|markdown| sarif::MultiformatMessageString {
            markdown: Some(markdown),
            properties: None,
            text: if explanation_text.trim().is_empty() {
                diagnostic.message.clone()
            } else {
                explanation_text
            },
        }),
        help_uri: diagnostic.code.as_deref().map(|code| {
            format!(
                "{DOCS_BASE_URL}/linting/rules/{}-{}",
                code.to_ascii_lowercase(),
                diagnostic.rule.name()
            )
        }),
        id: rule_id,
        message_strings: None,
        name: Some(diagnostic.rule.name().to_string()),
        properties: Some(sarif::PropertyBag {
            tags: None,
            additional_properties: BTreeMap::from([
                ("actonRuleGroupStatus".to_string(), json!(group_status)),
                ("actonRuleGroupSince".to_string(), json!(group_since)),
                (
                    "actonFixAvailability".to_string(),
                    json!(diagnostic.rule.fixable()),
                ),
                (
                    "actonDefinitionFile".to_string(),
                    json!(path_to_sarif_uri(Path::new(diagnostic.rule.file()), false)),
                ),
                (
                    "actonDefinitionLine".to_string(),
                    json!(diagnostic.rule.line()),
                ),
            ]),
        }),
        relationships: None,
        short_description: Some(sarif::MultiformatMessageString {
            markdown: Some(short_description_text.clone()),
            properties: None,
            text: short_description_text,
        }),
    }
}

#[derive(Clone)]
struct ResultLocations {
    primary: Option<sarif::Location>,
    related: Option<Vec<sarif::Location>>,
}

fn diagnostic_locations(
    diagnostic: &Diagnostic,
    file_db: &FileDb,
    project_root: &Path,
) -> ResultLocations {
    let Some(file_info) = file_db.get_by_id(diagnostic.file_id) else {
        return ResultLocations {
            primary: None,
            related: None,
        };
    };
    let artifact_location = artifact_location(file_info.path(), project_root);
    let source = file_info.source().source.as_ref();

    if diagnostic.annotations.is_empty() {
        return ResultLocations {
            primary: Some(sarif::Location {
                annotations: None,
                id: None,
                logical_locations: None,
                message: None,
                physical_location: Some(sarif::PhysicalLocation {
                    address: None,
                    artifact_location: Some(artifact_location),
                    context_region: None,
                    properties: None,
                    region: None,
                }),
                properties: None,
                relationships: None,
            }),
            related: None,
        };
    }

    let mut primary = None;
    let mut related = Vec::new();
    let primary_idx = diagnostic
        .annotations
        .iter()
        .position(|annotation| annotation.is_primary)
        .unwrap_or(0);

    for (idx, annotation) in diagnostic.annotations.iter().enumerate() {
        let location = annotation_to_location(annotation, &artifact_location, source);
        if idx == primary_idx {
            primary = Some(location);
        } else {
            related.push(location);
        }
    }

    ResultLocations {
        primary,
        related: if related.is_empty() {
            None
        } else {
            Some(related)
        },
    }
}

fn annotation_to_location(
    annotation: &Annotation,
    artifact_location: &sarif::ArtifactLocation,
    source: &str,
) -> sarif::Location {
    sarif::Location {
        annotations: None,
        id: None,
        logical_locations: None,
        message: annotation.message.clone().map(|text| sarif::Message {
            arguments: None,
            id: None,
            markdown: Some(text.clone()),
            properties: None,
            text: Some(text),
        }),
        physical_location: Some(sarif::PhysicalLocation {
            address: None,
            artifact_location: Some(artifact_location.clone()),
            context_region: None,
            properties: None,
            region: span_to_region(source, &annotation.span),
        }),
        properties: property_bag(
            annotation_tags(annotation),
            BTreeMap::from([("actonIsPrimary".to_string(), json!(annotation.is_primary))]),
        ),
        relationships: None,
    }
}

fn diagnostic_fixes(
    diagnostic: &Diagnostic,
    file_db: &FileDb,
    project_root: &Path,
) -> Option<Vec<sarif::Fix>> {
    let mut fixes = Vec::new();

    for fix in &diagnostic.fixes {
        let mut artifact_changes_by_file = BTreeMap::<PathBuf, sarif::ArtifactChange>::new();

        for edit in &fix.edits {
            let Some(file_info) = file_db.get_by_id(edit.file_id) else {
                continue;
            };

            let Some(region) = span_to_region(file_info.source().source.as_ref(), &edit.span)
            else {
                continue;
            };

            let replacement = sarif::Replacement {
                deleted_region: region,
                inserted_content: Some(sarif::ArtifactContent {
                    binary: None,
                    properties: None,
                    rendered: None,
                    text: Some(edit.replacement.clone()),
                }),
                properties: None,
            };

            artifact_changes_by_file
                .entry(file_info.path().clone())
                .and_modify(|change| change.replacements.push(replacement.clone()))
                .or_insert_with(|| sarif::ArtifactChange {
                    artifact_location: artifact_location(file_info.path(), project_root),
                    properties: None,
                    replacements: vec![replacement],
                });
        }

        let artifact_changes = artifact_changes_by_file.into_values().collect::<Vec<_>>();
        if artifact_changes.is_empty() {
            continue;
        }

        fixes.push(sarif::Fix {
            artifact_changes,
            description: Some(sarif::Message {
                arguments: None,
                id: None,
                markdown: Some(fix.message.clone()),
                properties: None,
                text: Some(fix.message.clone()),
            }),
            properties: Some(sarif::PropertyBag {
                tags: None,
                additional_properties: BTreeMap::from([(
                    "actonApplicability".to_string(),
                    json!(match fix.applicability {
                        Applicability::Auto => "auto",
                        Applicability::Manual => "manual",
                    }),
                )]),
            }),
        });
    }

    if fixes.is_empty() { None } else { Some(fixes) }
}

fn markdown_to_plain_text(markdown: &str) -> String {
    let mut result = String::with_capacity(markdown.len());
    let mut in_code_block = false;

    for line in markdown.lines() {
        let trimmed = line.trim();

        if trimmed.starts_with("```") {
            in_code_block = !in_code_block;
            continue;
        }

        if in_code_block {
            result.push_str(line);
            result.push('\n');
            continue;
        }

        let without_heading = trimmed.trim_start_matches('#').trim_start();
        let without_inline_code = without_heading.replace('`', "");
        result.push_str(&without_inline_code);
        result.push('\n');
    }

    result
}

fn truncate_for_github_description(text: &str) -> String {
    const GITHUB_FULL_DESCRIPTION_LIMIT: usize = 1024;
    if text.chars().count() <= GITHUB_FULL_DESCRIPTION_LIMIT {
        return text.to_string();
    }

    let truncated = text
        .chars()
        .take(GITHUB_FULL_DESCRIPTION_LIMIT.saturating_sub(1))
        .collect::<String>();
    format!("{truncated}…")
}

const fn rule_group_status_and_since(
    group: tolk_linter::RuleGroup,
) -> (&'static str, &'static str) {
    match group {
        tolk_linter::RuleGroup::Stable { since } => ("stable", since),
        tolk_linter::RuleGroup::Preview { since } => ("preview", since),
        tolk_linter::RuleGroup::Deprecated { since } => ("deprecated", since),
        tolk_linter::RuleGroup::Removed { since } => ("removed", since),
    }
}

fn diagnostic_result_properties(diagnostic: &Diagnostic) -> Option<sarif::PropertyBag> {
    let mut additional =
        BTreeMap::from([("actonDiagnosticName".to_string(), json!(diagnostic.name))]);

    if let Some(help) = &diagnostic.help {
        additional.insert("actonHelp".to_string(), json!(help));
    }

    property_bag(None, additional)
}

fn annotation_tags(annotation: &Annotation) -> Option<Vec<String>> {
    if annotation.tags.is_empty() {
        return None;
    }

    let tags = annotation
        .tags
        .iter()
        .map(|tag| match tag {
            DiagnosticTag::Unnecessary => "unnecessary".to_string(),
            DiagnosticTag::Deprecated => "deprecated".to_string(),
        })
        .collect::<Vec<_>>();

    Some(tags)
}

fn property_bag(
    tags: Option<Vec<String>>,
    additional_properties: BTreeMap<String, serde_json::Value>,
) -> Option<sarif::PropertyBag> {
    if tags.is_none() && additional_properties.is_empty() {
        return None;
    }

    Some(sarif::PropertyBag {
        tags,
        additional_properties,
    })
}

fn artifact_location(path: &Path, project_root: &Path) -> sarif::ArtifactLocation {
    let (uri, uri_base_id) = if let Ok(relative) = path.strip_prefix(project_root) {
        (
            path_to_sarif_uri(relative, false),
            Some(SOURCE_ROOT_URI_BASE_ID.to_string()),
        )
    } else {
        (path_to_sarif_uri(path, false), None)
    };

    sarif::ArtifactLocation {
        description: None,
        index: None,
        properties: None,
        uri: Some(uri),
        uri_base_id,
    }
}

fn span_to_region(source: &str, span: &Span) -> Option<sarif::Region> {
    let (start_line, start_col) = pos::byte_to_line_col(source, span.start as usize)?;
    let (end_line, end_col) = pos::byte_to_line_col(source, span.end as usize)?;

    Some(sarif::Region {
        byte_length: Some(i64::from(span.end.saturating_sub(span.start))),
        byte_offset: Some(i64::from(span.start)),
        char_length: None,
        char_offset: None,
        end_column: Some(i64::from(end_col + 1)),
        end_line: Some(i64::from(end_line + 1)),
        message: None,
        properties: None,
        snippet: None,
        source_language: None,
        start_column: Some(i64::from(start_col + 1)),
        start_line: Some(i64::from(start_line + 1)),
    })
}

fn diagnostic_rule_id(diagnostic: &Diagnostic) -> String {
    diagnostic
        .code
        .clone()
        .unwrap_or_else(|| diagnostic.name.to_string())
}

const fn severity_to_sarif_level(severity: Severity) -> sarif::ResultLevel {
    match severity {
        Severity::Warning => sarif::ResultLevel::Warning,
        Severity::Error | Severity::Fatal => sarif::ResultLevel::Error,
        Severity::Info | Severity::Help => sarif::ResultLevel::Note,
    }
}

fn path_to_sarif_uri(path: &Path, ensure_trailing_slash: bool) -> String {
    let mut uri = path.to_string_lossy().replace('\\', "/");
    if ensure_trailing_slash && !uri.ends_with('/') {
        uri.push('/');
    }
    uri
}
