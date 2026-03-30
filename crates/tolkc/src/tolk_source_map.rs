use anyhow::anyhow;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::sync::Arc;
use ton_source_map::SourceLocation;
use tycho_types::boc::Boc;
use tycho_types::cell::Cell;

#[derive(Debug, Clone)]
pub struct TolkSourceMap {
    pub source_map: crate::SourceMap,
    pub marks_dict: Option<Arc<crate::debug_marks_dict::DebugMarksDict>>,
}

#[derive(Debug, Serialize, Deserialize)]
struct SerializableTolkSourceMap {
    source_map: crate::SourceMap,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    marks_dict: Option<crate::debug_marks_dict::DebugMarksDict>,
}

impl Serialize for TolkSourceMap {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        SerializableTolkSourceMap {
            source_map: self.source_map.clone(),
            marks_dict: self.marks_dict.as_deref().cloned(),
        }
        .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for TolkSourceMap {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = SerializableTolkSourceMap::deserialize(deserializer)?;
        Ok(Self {
            source_map: value.source_map,
            marks_dict: value.marks_dict.map(Arc::new),
        })
    }
}

impl TolkSourceMap {
    #[must_use]
    pub const fn new(source_map: crate::SourceMap) -> Self {
        Self {
            source_map,
            marks_dict: None,
        }
    }

    pub fn from_code_cell(
        source_map: crate::SourceMap,
        code_cell: &Cell,
        marks_boc64: Option<&str>,
    ) -> anyhow::Result<Self> {
        let code_boc: Arc<[u8]> = Arc::from(Boc::encode(code_cell.clone()));
        let marks_boc = decode_optional_boc_base64_bytes(marks_boc64)?;
        let marks_dict = marks_boc.map(|marks_boc| {
            Arc::new(crate::debug_marks_dict::parse_debug_marks(
                marks_boc.as_ref(),
                code_boc.as_ref(),
            ))
        });
        Ok(Self {
            source_map,
            marks_dict,
        })
    }

    #[must_use]
    pub fn without_debug_info() -> Self {
        Self::new(crate::SourceMap::default())
    }

    pub fn find_source_loc(&self, hash: &str, offset: u16) -> Option<SourceLocation> {
        let marks = self.marks_dict.as_ref()?.get(hash)?;
        let target_offset = i32::from(offset);

        let mut approx_loc = None;
        let mut exact_loc = None;

        for &(mark_offset, mark_id) in marks {
            let Some(loc) = self.source_location_for_mark(mark_id as usize) else {
                continue;
            };

            if mark_offset < target_offset {
                approx_loc = Some(loc);
                continue;
            }

            if mark_offset == target_offset {
                exact_loc = Some(loc);
                continue;
            }

            break;
        }

        exact_loc.or(approx_loc)
    }

    fn source_location_for_mark(&self, mark_id: usize) -> Option<SourceLocation> {
        let range = match self.source_map.get_debug_mark(mark_id) {
            crate::source_map::DebugMark::Loc { range, .. }
            | crate::source_map::DebugMark::LeaveFun { range, .. } => range,
            crate::source_map::DebugMark::EnterFun {
                is_inlined: true,
                range,
                ..
            } => range,
            _ => return None,
        };

        let file_id = range.file_id();
        let file = self
            .source_map
            .resolve_file_full_path(file_id)
            .unwrap_or_else(|| self.source_map.resolve_file_name(file_id))
            .to_owned();
        if file.is_empty() || file.starts_with("@stdlib/") {
            return None;
        }

        Some(SourceLocation {
            file,
            line: range.start_line() as i64,
            column: range.start_col() as i64,
            end_line: range.end_line() as i64,
            end_column: range.end_col() as i64,
            length: 0,
        })
    }
}

fn decode_optional_boc_base64_bytes(boc_b64: Option<&str>) -> anyhow::Result<Option<Arc<[u8]>>> {
    match boc_b64.filter(|boc| !boc.is_empty()) {
        Some(boc_b64) => {
            let cell = Boc::decode_base64(boc_b64)
                .map_err(|e| anyhow!("debug marks is not valid BoC: {e}"))?;
            Ok(Some(Arc::from(Boc::encode(cell))))
        }
        None => Ok(None),
    }
}
