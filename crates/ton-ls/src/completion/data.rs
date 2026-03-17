use serde_json::{Value, json};

pub(crate) const COMPLETION_DATA_VERSION: u8 = 1;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CompletionItemDataEnvelope {
    pub(crate) version: u8,
    pub(crate) language: String,
    pub(crate) provider: String,
    pub(crate) resolve_id: String,
}

impl CompletionItemDataEnvelope {
    #[must_use]
    pub(crate) fn new(language: String, provider: String, resolve_id: String) -> Self {
        Self {
            version: COMPLETION_DATA_VERSION,
            language,
            provider,
            resolve_id,
        }
    }

    #[must_use]
    pub(crate) fn to_json_value(&self) -> Value {
        json!({
            "version": self.version,
            "language": self.language,
            "provider": self.provider,
            "resolve_id": self.resolve_id,
        })
    }

    #[must_use]
    pub(crate) fn from_json_value(value: &Value) -> Option<Self> {
        let object = value.as_object()?;
        let version = object.get("version")?.as_u64()? as u8;
        let language = object.get("language")?.as_str()?.to_owned();
        let provider = object.get("provider")?.as_str()?.to_owned();
        let resolve_id = object.get("resolve_id")?.as_str()?.to_owned();

        Some(Self {
            version,
            language,
            provider,
            resolve_id,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrips_completion_item_data_envelope() {
        let envelope = CompletionItemDataEnvelope::new(
            "toml".to_string(),
            "schema".to_string(),
            "abc".to_string(),
        );
        let json = envelope.to_json_value();
        let decoded = CompletionItemDataEnvelope::from_json_value(&json)
            .expect("envelope should decode from json");
        assert_eq!(decoded, envelope);
    }
}
