use crate::{InlayHintCategory, LanguageId};
use serde::{Deserialize, Serialize};

/// Runtime settings accepted by the TON language server.
///
/// Send this object through `initializationOptions` or
/// `workspace/didChangeConfiguration`. VS Code nests this object under a `ton`
/// property when it sends a configuration change.
#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "schema", schemars(default))]
#[serde(default, rename_all = "camelCase")]
pub struct LanguageServerSettings {
    /// Settings for the Tolk language.
    pub tolk: TolkSettings,
    /// Settings for the TL-B language.
    pub tlb: TlbSettings,
    /// Settings for the Fift language.
    pub fift: FiftSettings,
}

impl LanguageServerSettings {
    #[must_use]
    pub fn inlay_hint_enabled(
        &self,
        language_id: Option<&LanguageId>,
        category: InlayHintCategory,
    ) -> bool {
        if language_id.is_some_and(|language_id| language_id.as_str() == "tolk") {
            let hints = &self.tolk.hints;
            return !hints.disable
                && match category {
                    InlayHintCategory::Type => hints.types,
                    InlayHintCategory::Parameter => hints.parameters,
                    InlayHintCategory::ConstantValue => hints.constant_values,
                    InlayHintCategory::MethodId => hints.show_method_id,
                    InlayHintCategory::ConstructorTag
                    | InlayHintCategory::GasConsumption
                    | InlayHintCategory::Other => true,
                };
        }
        if language_id.is_some_and(|language_id| language_id.as_str() == "tlb") {
            return !self.tlb.hints.disable
                && (category != InlayHintCategory::ConstructorTag
                    || self.tlb.hints.show_constructor_tag);
        }
        if language_id.is_some_and(|language_id| language_id.as_str() == "fift") {
            return category != InlayHintCategory::GasConsumption
                || self.fift.hints.show_gas_consumption;
        }
        true
    }
}

/// Tolk language features and diagnostics.
#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(default, rename_all = "camelCase")]
pub struct TolkSettings {
    /// Tolk inlay hint settings.
    pub hints: TolkHintSettings,
    /// Tolk completion settings.
    pub completion: TolkCompletionSettings,
    /// Tolk reference search settings.
    pub find_usages: FindUsagesSettings,
    /// Tolk diagnostic settings.
    pub diagnostics: TolkDiagnosticSettings,
}

/// Controls the inlay hints shown in Tolk source files.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(default, rename_all = "camelCase")]
pub struct TolkHintSettings {
    /// Disables every Tolk inlay hint when set to `true`.
    pub disable: bool,
    /// Shows inferred type hints for variables and expressions.
    pub types: bool,
    /// Shows parameter name hints at call sites.
    pub parameters: bool,
    /// Shows computed method IDs for get methods.
    pub show_method_id: bool,
    /// Shows computed values for constants and enum members.
    pub constant_values: bool,
}

impl Default for TolkHintSettings {
    fn default() -> Self {
        Self {
            disable: false,
            types: true,
            parameters: true,
            show_method_id: true,
            constant_values: true,
        }
    }
}

/// Controls completion behavior in Tolk source files.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(default, rename_all = "camelCase")]
pub struct TolkCompletionSettings {
    /// Ranks completion items by their relevance to the expected type.
    pub type_aware: bool,
    /// Adds an import edit when completion inserts a symbol from another file.
    pub add_imports: bool,
}

impl Default for TolkCompletionSettings {
    fn default() -> Self {
        Self {
            type_aware: true,
            add_imports: true,
        }
    }
}

/// Controls the scope of Tolk reference searches.
#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(default)]
pub struct FindUsagesSettings {
    /// Selects which indexed files can appear in reference results.
    #[cfg_attr(feature = "schema", schemars(default = "default_find_usages_scope"))]
    pub scope: FindUsagesScope,
}

/// Files that can appear in Tolk reference results.
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "lowercase")]
pub enum FindUsagesScope {
    /// Searches only files inside the current workspace.
    #[default]
    Workspace,
    /// Searches workspace files and external sources, including the standard library.
    Everywhere,
}

#[cfg(feature = "schema")]
const fn default_find_usages_scope() -> FindUsagesScope {
    FindUsagesScope::Workspace
}

/// Controls all Tolk diagnostics and each diagnostic provider.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(default)]
pub struct TolkDiagnosticSettings {
    /// Enables all Tolk diagnostics when set to `true`.
    ///
    /// This setting takes precedence over individual provider settings.
    pub enabled: bool,
    /// Controls diagnostics produced by the Acton Tolk linter.
    pub linter: DiagnosticProviderSettings,
    /// Controls diagnostics produced by the native Tolk compiler.
    pub compiler: DiagnosticProviderSettings,
}

impl Default for TolkDiagnosticSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            linter: DiagnosticProviderSettings::default(),
            compiler: DiagnosticProviderSettings::default(),
        }
    }
}

/// Enables or disables one diagnostic provider.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(default)]
pub struct DiagnosticProviderSettings {
    /// Enables diagnostics from this provider.
    pub enabled: bool,
}

impl Default for DiagnosticProviderSettings {
    fn default() -> Self {
        Self { enabled: true }
    }
}

/// TL-B language features.
#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(default)]
pub struct TlbSettings {
    /// TL-B inlay hint settings.
    pub hints: TlbHintSettings,
}

/// Controls the inlay hints shown in TL-B source files.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(default, rename_all = "camelCase")]
pub struct TlbHintSettings {
    /// Disables every TL-B inlay hint when set to `true`.
    pub disable: bool,
    /// Shows computed constructor tags for constructors without an explicit tag.
    pub show_constructor_tag: bool,
}

impl Default for TlbHintSettings {
    fn default() -> Self {
        Self {
            disable: false,
            show_constructor_tag: true,
        }
    }
}

/// Fift language features.
#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(default, rename_all = "camelCase")]
pub struct FiftSettings {
    /// Fift inlay hint settings.
    pub hints: FiftHintSettings,
    /// Fift semantic highlighting settings.
    pub semantic_highlighting: SemanticHighlightingSettings,
}

/// Controls the inlay hints shown in Fift source files.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(default, rename_all = "camelCase")]
pub struct FiftHintSettings {
    /// Shows gas consumption for Fift instructions.
    pub show_gas_consumption: bool,
}

impl Default for FiftHintSettings {
    fn default() -> Self {
        Self {
            show_gas_consumption: true,
        }
    }
}

/// Controls semantic highlighting for a language.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(default)]
pub struct SemanticHighlightingSettings {
    /// Enables semantic highlighting when set to `true`.
    pub enabled: bool,
}

impl Default for SemanticHighlightingSettings {
    fn default() -> Self {
        Self { enabled: true }
    }
}
