use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct SourceLocation {
    pub file: String,
    pub line: i64,
    pub column: i64,
    pub end_line: i64,
    pub end_column: i64,
    pub length: i64,
}

impl SourceLocation {
    #[must_use]
    pub fn format(&self) -> String {
        format!(
            "{}:{}:{}",
            Self::normalize_path(&self.file),
            self.line,
            self.column,
        )
    }

    #[must_use]
    pub fn format_normalized(&self) -> String {
        format!(
            "{}:{}:{}",
            Self::normalize_path(&self.file),
            self.line + 1,
            self.column + 2,
        )
    }

    #[must_use]
    pub fn format_full(&self) -> String {
        let file = &self.file;
        format!("{}:{}:{}", file, self.line, self.column)
    }

    #[must_use]
    pub fn normalize_path(file: &str) -> String {
        let normalized = file.to_owned();

        if let Ok(cwd) = std::env::current_dir()
            && let Some(relative) = pathdiff::diff_paths(&normalized, cwd)
        {
            return relative.display().to_string();
        }

        normalized
    }

    pub fn parse(s: &str) -> anyhow::Result<Option<Self>> {
        if s.is_empty() {
            return Ok(None);
        }

        let parts = s.rsplitn(3, ':').collect::<Vec<_>>();
        if parts.len() != 3 {
            anyhow::bail!("invalid source location, expected file:line:col, got {s}");
        }

        let file = parts[2].to_owned();
        let line = parts[1].parse::<i64>()?;
        let column = parts[0].parse::<i64>()?;

        Ok(Some(SourceLocation {
            file,
            line,
            column,
            end_line: line,
            end_column: column,
            length: 0,
        }))
    }
}
