use crate::model::{CompletionInfo, CompletionProperty, SchemaDoc, SchemaPathSegment};
use anyhow::{Context, Result};
use jsonptr::PointerBuf;
use referencing::{Registry, Resolver, Resource, ResourceRef};
use regex::Regex;
use serde_json::{Map, Value};
use std::collections::{BTreeMap, BTreeSet, HashSet};

const ROOT_SCHEMA_URI: &str = "memory://ton-ls/root.schema.json";
const MAX_EXPANSION_DEPTH: usize = 32;
const MAX_STATES: usize = 128;

#[derive(Clone)]
struct SchemaState<'a> {
    schema: Value,
    resolver: Resolver<'a>,
}

/// Parsed JSON Schema with utilities for hover/completion-oriented lookups.
#[derive(Debug, Clone)]
pub struct SchemaStore {
    root_schema: Value,
    root_uri: String,
    registry: Registry,
}

impl SchemaStore {
    /// Build a schema store from raw JSON text.
    ///
    /// # Errors
    ///
    /// Returns an error if JSON is invalid or the schema registry cannot be built.
    pub fn from_json_str(schema: &str) -> Result<Self> {
        let value: Value = serde_json::from_str(schema).context("failed to parse schema JSON")?;
        Self::from_value(value)
    }

    /// Build a schema store from a JSON value.
    ///
    /// # Errors
    ///
    /// Returns an error if the schema registry cannot be built.
    pub fn from_value(root_schema: Value) -> Result<Self> {
        let registry = Registry::options()
            .build([(
                ROOT_SCHEMA_URI,
                Resource::from_contents(root_schema.clone()),
            )])
            .context("failed to build schema registry")?;

        Ok(Self {
            root_schema,
            root_uri: ROOT_SCHEMA_URI.to_string(),
            registry,
        })
    }

    #[must_use]
    pub fn pointer_for_path(path: &[SchemaPathSegment]) -> PointerBuf {
        let mut pointer = PointerBuf::new();
        for segment in path {
            pointer.push_back(segment.as_token());
        }
        pointer
    }

    #[must_use]
    pub fn summary_for_path(&self, path: &[SchemaPathSegment]) -> Option<SchemaDoc> {
        let states = self.states_for_path(path);
        Self::merge_docs(
            states
                .into_iter()
                .map(|state| Self::doc_from_value(&state.schema)),
        )
    }

    #[must_use]
    pub fn completion_for_path(&self, path: &[SchemaPathSegment]) -> Option<CompletionInfo> {
        let states = self.states_for_path(path);
        if states.is_empty() {
            return None;
        }

        let mut properties: BTreeMap<String, CompletionProperty> = BTreeMap::new();
        let mut has_pattern_properties = false;
        let mut allows_additional_properties = false;

        for state in states {
            let Some(obj) = state.schema.as_object() else {
                continue;
            };

            if obj.contains_key("patternProperties") {
                has_pattern_properties = true;
            }

            allows_additional_properties |= match obj.get("additionalProperties") {
                Some(Value::Bool(value)) => *value,
                Some(Value::Object(_)) => true,
                Some(_) => false,
                None => Self::type_allows(obj, "object"),
            };

            let required = Self::required_properties(obj);
            let Some(schema_properties) = obj.get("properties").and_then(Value::as_object) else {
                continue;
            };

            for (name, property_schema) in schema_properties {
                let required_here = required.contains(name);
                let property_doc = self.summary_from_schema_value(property_schema, &state.resolver);

                let entry = properties
                    .entry(name.clone())
                    .or_insert_with(|| CompletionProperty {
                        name: name.clone(),
                        required: false,
                        doc: SchemaDoc::default(),
                    });

                entry.required |= required_here;
                entry.doc = Self::merge_pair_docs(&entry.doc, &property_doc);
            }
        }

        Some(CompletionInfo {
            properties: properties.into_values().collect(),
            has_pattern_properties,
            allows_additional_properties,
        })
    }

    #[must_use]
    pub fn is_value_valid_at_path(&self, path: &[SchemaPathSegment], value: &Value) -> bool {
        let states = self.states_for_path(path);
        if states.is_empty() {
            return false;
        }

        states.into_iter().any(|state| {
            jsonschema::validator_for(&state.schema)
                .map(|validator| validator.is_valid(value))
                .unwrap_or(false)
        })
    }

    fn summary_from_schema_value(&self, schema: &Value, resolver: &Resolver<'_>) -> SchemaDoc {
        let states = self.expand_states(vec![self.make_child_state(resolver, schema.clone())]);
        Self::merge_docs(
            states
                .into_iter()
                .map(|state| Self::doc_from_value(&state.schema)),
        )
        .unwrap_or_default()
    }

    fn states_for_path(&self, path: &[SchemaPathSegment]) -> Vec<SchemaState<'_>> {
        let Some(initial) = self.initial_state() else {
            return Vec::new();
        };

        let mut states = self.expand_states(vec![initial]);

        for segment in path {
            let mut next_states = Vec::new();
            for state in states {
                next_states.extend(self.follow_segment(state, segment));
                if next_states.len() >= MAX_STATES {
                    break;
                }
            }
            states = self.expand_states(next_states);
            if states.is_empty() {
                break;
            }
        }

        states
    }

    fn initial_state(&self) -> Option<SchemaState<'_>> {
        let resolver = self.registry.try_resolver(&self.root_uri).ok()?;
        Some(SchemaState {
            schema: self.root_schema.clone(),
            resolver,
        })
    }

    fn expand_states<'a>(&self, mut states: Vec<SchemaState<'a>>) -> Vec<SchemaState<'a>> {
        if states.is_empty() {
            return states;
        }

        for _ in 0..MAX_EXPANSION_DEPTH {
            let mut changed = false;
            let mut next = Vec::new();

            for state in states {
                if Self::is_expandable(&state.schema) {
                    changed = true;
                }
                next.extend(self.expand_state_once(state));
                if next.len() >= MAX_STATES {
                    break;
                }
            }

            states = self.dedup_states(next);
            if !changed || states.is_empty() {
                break;
            }
        }

        states
    }

    fn is_expandable(schema: &Value) -> bool {
        let Some(obj) = schema.as_object() else {
            return false;
        };

        obj.contains_key("$ref")
            || obj.contains_key("allOf")
            || obj.contains_key("anyOf")
            || obj.contains_key("oneOf")
    }

    fn expand_state_once<'a>(&self, state: SchemaState<'a>) -> Vec<SchemaState<'a>> {
        let Some(obj) = state.schema.as_object() else {
            return vec![state];
        };

        if let Some(Value::String(reference)) = obj.get("$ref") {
            let mut result = Vec::new();

            if let Ok(resolved) = state.resolver.lookup(reference) {
                result.push(SchemaState {
                    schema: resolved.contents().clone(),
                    resolver: resolved.resolver().clone(),
                });
            }

            if obj.len() > 1 {
                let mut local = obj.clone();
                local.remove("$ref");
                if !local.is_empty() {
                    result.push(SchemaState {
                        schema: Value::Object(local),
                        resolver: state.resolver.clone(),
                    });
                }
            }

            if !result.is_empty() {
                return result;
            }
        }

        let mut branches = Vec::new();
        let mut has_composition = false;

        for keyword in ["allOf", "anyOf", "oneOf"] {
            if let Some(array) = obj.get(keyword).and_then(Value::as_array) {
                has_composition = true;
                for value in array {
                    branches.push(self.make_child_state(&state.resolver, value.clone()));
                }
            }
        }

        if has_composition {
            let mut base = obj.clone();
            base.remove("allOf");
            base.remove("anyOf");
            base.remove("oneOf");
            if !base.is_empty() {
                branches.push(SchemaState {
                    schema: Value::Object(base),
                    resolver: state.resolver.clone(),
                });
            }

            if !branches.is_empty() {
                return branches;
            }
        }

        vec![state]
    }

    fn follow_segment<'a>(
        &self,
        state: SchemaState<'a>,
        segment: &SchemaPathSegment,
    ) -> Vec<SchemaState<'a>> {
        match (&state.schema, segment) {
            (Value::Bool(false), _) => Vec::new(),
            (Value::Bool(true), _) => vec![state],
            (Value::Object(obj), SchemaPathSegment::Key(key)) => {
                self.follow_object_key(state.resolver, obj, key)
            }
            (Value::Object(obj), SchemaPathSegment::Index(index)) => {
                self.follow_array_index(state.resolver, obj, *index)
            }
            _ => Vec::new(),
        }
    }

    fn follow_object_key<'a>(
        &self,
        resolver: Resolver<'a>,
        obj: &Map<String, Value>,
        key: &str,
    ) -> Vec<SchemaState<'a>> {
        let mut result = Vec::new();
        let mut matched_explicit = false;

        if let Some(properties) = obj.get("properties").and_then(Value::as_object)
            && let Some(value) = properties.get(key)
        {
            result.push(self.make_child_state(&resolver, value.clone()));
            matched_explicit = true;
        }

        if let Some(pattern_properties) = obj.get("patternProperties").and_then(Value::as_object) {
            for (pattern, schema) in pattern_properties {
                let Ok(regex) = Regex::new(pattern) else {
                    continue;
                };
                if regex.is_match(key) {
                    result.push(self.make_child_state(&resolver, schema.clone()));
                    matched_explicit = true;
                }
            }
        }

        if !matched_explicit {
            match obj.get("additionalProperties") {
                Some(Value::Bool(false)) => {}
                Some(Value::Bool(true)) => {
                    result.push(self.make_child_state(&resolver, Value::Object(Map::new())));
                }
                Some(Value::Object(value)) => {
                    result.push(self.make_child_state(&resolver, Value::Object(value.clone())));
                }
                Some(_) => {}
                None => {
                    if Self::type_allows(obj, "object") {
                        result.push(self.make_child_state(&resolver, Value::Object(Map::new())));
                    }
                }
            }
        }

        self.dedup_states(result)
    }

    fn follow_array_index<'a>(
        &self,
        resolver: Resolver<'a>,
        obj: &Map<String, Value>,
        index: usize,
    ) -> Vec<SchemaState<'a>> {
        let mut result = Vec::new();

        if let Some(prefix_items) = obj.get("prefixItems").and_then(Value::as_array)
            && let Some(schema) = prefix_items.get(index)
        {
            result.push(self.make_child_state(&resolver, schema.clone()));
            return self.dedup_states(result);
        }

        if let Some(items) = obj.get("items") {
            match items {
                Value::Object(schema) => {
                    result.push(self.make_child_state(&resolver, Value::Object(schema.clone())));
                }
                Value::Array(schemas) => {
                    if let Some(schema) = schemas.get(index) {
                        result.push(self.make_child_state(&resolver, schema.clone()));
                    } else if let Some(additional_items) = obj.get("additionalItems") {
                        match additional_items {
                            Value::Bool(true) => {
                                result.push(
                                    self.make_child_state(&resolver, Value::Object(Map::new())),
                                );
                            }
                            Value::Object(schema) => {
                                result.push(
                                    self.make_child_state(&resolver, Value::Object(schema.clone())),
                                );
                            }
                            _ => {}
                        }
                    }
                }
                Value::Bool(true) => {
                    result.push(self.make_child_state(&resolver, Value::Object(Map::new())));
                }
                Value::Bool(false) | Value::Null => {}
                _ => {}
            }
        } else if Self::type_allows(obj, "array") {
            result.push(self.make_child_state(&resolver, Value::Object(Map::new())));
        }

        self.dedup_states(result)
    }

    fn make_child_state<'a>(&self, resolver: &Resolver<'a>, schema: Value) -> SchemaState<'a> {
        let child_resolver = resolver
            .in_subresource(ResourceRef::from_contents(&schema))
            .unwrap_or_else(|_| resolver.clone());

        SchemaState {
            schema,
            resolver: child_resolver,
        }
    }

    fn dedup_states<'a>(&self, states: Vec<SchemaState<'a>>) -> Vec<SchemaState<'a>> {
        let mut result = Vec::new();
        let mut seen = HashSet::new();

        for state in states {
            if result.len() >= MAX_STATES {
                break;
            }

            let base_uri = state.resolver.base_uri().as_str().to_string();
            let schema_json = serde_json::to_string(&state.schema).unwrap_or_default();
            if seen.insert((base_uri, schema_json)) {
                result.push(state);
            }
        }

        result
    }

    fn required_properties(obj: &Map<String, Value>) -> BTreeSet<String> {
        obj.get("required")
            .and_then(Value::as_array)
            .map(|entries| {
                entries
                    .iter()
                    .filter_map(Value::as_str)
                    .map(ToString::to_string)
                    .collect::<BTreeSet<_>>()
            })
            .unwrap_or_default()
    }

    fn type_allows(obj: &Map<String, Value>, expected: &str) -> bool {
        match obj.get("type") {
            Some(Value::String(value)) => value == expected,
            Some(Value::Array(values)) => {
                values.iter().any(|value| value.as_str() == Some(expected))
            }
            Some(_) => false,
            None => true,
        }
    }

    fn doc_from_value(schema: &Value) -> SchemaDoc {
        let Some(obj) = schema.as_object() else {
            return SchemaDoc::default();
        };

        let schema_type = match obj.get("type") {
            Some(Value::String(value)) => Some(value.clone()),
            Some(Value::Array(values)) => {
                let parts = values.iter().filter_map(Value::as_str).collect::<Vec<_>>();
                if parts.is_empty() {
                    None
                } else {
                    Some(parts.join(" | "))
                }
            }
            _ => None,
        };

        let enum_values = obj
            .get("enum")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();

        let examples = obj
            .get("examples")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();

        SchemaDoc {
            title: obj
                .get("title")
                .and_then(Value::as_str)
                .map(ToString::to_string),
            description: obj
                .get("description")
                .and_then(Value::as_str)
                .map(ToString::to_string),
            schema_type,
            const_value: obj.get("const").cloned(),
            default_value: obj.get("default").cloned(),
            enum_values,
            examples,
        }
    }

    fn merge_docs(docs: impl IntoIterator<Item = SchemaDoc>) -> Option<SchemaDoc> {
        let mut merged = SchemaDoc::default();
        let mut has_any = false;

        for doc in docs {
            if doc.is_empty() {
                continue;
            }
            has_any = true;
            merged = Self::merge_pair_docs(&merged, &doc);
        }

        if has_any { Some(merged) } else { None }
    }

    fn merge_pair_docs(left: &SchemaDoc, right: &SchemaDoc) -> SchemaDoc {
        let description = match (&left.description, &right.description) {
            (Some(l), Some(r)) if l != r => Some(format!("{l}\n\n{r}")),
            (Some(l), _) => Some(l.clone()),
            (_, Some(r)) => Some(r.clone()),
            _ => None,
        };

        SchemaDoc {
            title: left.title.clone().or_else(|| right.title.clone()),
            description,
            schema_type: left
                .schema_type
                .clone()
                .or_else(|| right.schema_type.clone()),
            const_value: left
                .const_value
                .clone()
                .or_else(|| right.const_value.clone()),
            default_value: left
                .default_value
                .clone()
                .or_else(|| right.default_value.clone()),
            enum_values: Self::merge_unique_values(&left.enum_values, &right.enum_values),
            examples: Self::merge_unique_values(&left.examples, &right.examples),
        }
    }

    fn merge_unique_values(left: &[Value], right: &[Value]) -> Vec<Value> {
        let mut result = Vec::new();
        let mut seen = HashSet::new();

        for value in left.iter().chain(right.iter()) {
            let key = serde_json::to_string(value).unwrap_or_default();
            if seen.insert(key) {
                result.push(value.clone());
            }
        }

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_schema() -> Value {
        serde_json::json!({
            "$schema": "http://json-schema.org/draft-07/schema#",
            "type": "object",
            "properties": {
                "package": { "$ref": "#/$defs/package" },
                "networks": {
                    "type": "object",
                    "additionalProperties": { "$ref": "#/$defs/network" }
                }
            },
            "$defs": {
                "package": {
                    "type": "object",
                    "properties": {
                        "name": {
                            "type": "string",
                            "description": "Project name"
                        },
                        "version": {
                            "type": "string",
                            "description": "Project version"
                        }
                    },
                    "required": ["name"]
                },
                "network": {
                    "type": "object",
                    "properties": {
                        "api": {
                            "type": "object",
                            "properties": {
                                "v2": {
                                    "type": "string",
                                    "description": "TonCenter v2 endpoint"
                                }
                            }
                        }
                    }
                }
            }
        })
    }

    #[test]
    fn resolves_ref_summary() -> Result<()> {
        let store = SchemaStore::from_value(fixture_schema())?;
        let doc = store
            .summary_for_path(&[
                SchemaPathSegment::from("package"),
                SchemaPathSegment::from("name"),
            ])
            .expect("summary should exist");

        assert_eq!(doc.description.as_deref(), Some("Project name"));
        assert_eq!(doc.schema_type.as_deref(), Some("string"));
        Ok(())
    }

    #[test]
    fn resolves_additional_properties_ref_summary() -> Result<()> {
        let store = SchemaStore::from_value(fixture_schema())?;
        let doc = store
            .summary_for_path(&[
                SchemaPathSegment::from("networks"),
                SchemaPathSegment::from("mainnet"),
                SchemaPathSegment::from("api"),
                SchemaPathSegment::from("v2"),
            ])
            .expect("summary should exist");

        assert_eq!(doc.description.as_deref(), Some("TonCenter v2 endpoint"));
        Ok(())
    }

    #[test]
    fn provides_completion_for_object_path() -> Result<()> {
        let store = SchemaStore::from_value(fixture_schema())?;
        let completion = store
            .completion_for_path(&[SchemaPathSegment::from("package")])
            .expect("completion should exist");

        let names = completion
            .properties
            .iter()
            .map(|it| it.name.as_str())
            .collect::<Vec<_>>();
        assert_eq!(names, vec!["name", "version"]);

        let name = completion
            .properties
            .iter()
            .find(|it| it.name == "name")
            .expect("name property expected");
        assert!(name.required);
        Ok(())
    }

    #[test]
    fn validates_value_against_resolved_schema() -> Result<()> {
        let store = SchemaStore::from_value(fixture_schema())?;

        let path = [
            SchemaPathSegment::from("package"),
            SchemaPathSegment::from("name"),
        ];

        assert!(store.is_value_valid_at_path(&path, &Value::String("Acton".to_string())));
        assert!(!store.is_value_valid_at_path(&path, &Value::Number(1.into())));
        Ok(())
    }
}
