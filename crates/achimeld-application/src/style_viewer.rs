//! Closed intents and read-only state for the independent style preview tool.
use crate::IconStyle;

pub enum StyleViewerAction {
    Open,
    DuplicateStyle(String),
    CloneBuiltins,
    DeleteProjectStyle {
        id: String,
        replacement: Option<String>,
    },
    LoadJson,
    SaveJson,
    SelectStyle(String),
    ImportStyle(String),
    SelectImage(StyleViewerSource),
    SetImageSlot {
        index: usize,
        source: Option<StyleViewerSource>,
    },
    SetUnlockValue(String),
    RenameStyle {
        id: String,
        name: String,
    },
    AddImage,
    RemoveImage(usize),
    MoveImage {
        index: usize,
        up: bool,
    },
    RemoveStyle(String),
    LoadSet,
    SaveSet,
    SaveSetAs,
    NewCollection,
    SelectCollection(Option<String>),
    CopyToCollection {
        style: String,
        collection: String,
    },
    UnloadCollection(String),
    LinkSet(SteamAchievementSetLink),
    UnlinkSet(SteamAchievementSetLink),
}

/// An artwork identity, never an executable or a filesystem path.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum StyleViewerSource {
    Image { id: String },
    Atlas { id: Option<String>, tile: u64 },
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StyleViewerPreview {
    #[serde(default)]
    pub image: Option<StyleViewerSource>,
    #[serde(default)]
    pub slots: Option<[Option<StyleViewerSource>; 3]>,
    /// Retain implicit cup inputs in the other slots after editing/clearing one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_slots: Option<[bool; 3]>,
    pub unlock_value: String,
}
impl Default for StyleViewerPreview {
    fn default() -> Self {
        Self {
            image: None,
            slots: None,
            default_slots: None,
            unlock_value: "10".into(),
        }
    }
}
impl StyleViewerPreview {
    pub fn uses_default(&self, index: usize) -> bool {
        self.default_slots
            .as_ref()
            .map_or(self.slots.is_none() && self.image.is_none(), |slots| {
                slots.get(index).copied().unwrap_or(false)
            })
    }
    pub fn apply_images(&self, entity: &mut serde_json::Value, defaults: &[serde_json::Value]) {
        use serde_json::{Value, json};
        let indices: std::collections::BTreeSet<_> = entity["icon"]["layers"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|layer| layer["kind"] == "AchievementImage")
            .map(|layer| layer["generalImageIndex"].as_u64().unwrap_or(0))
            .collect();
        let mut entries = crate::general_image_entries(entity);
        if self.slots.is_some() {
            entries.retain(|e| e["id"].as_u64().is_some_and(|id| id >= 3));
        }
        for index in indices {
            if let Some(source) = usize::try_from(index)
                .ok()
                .and_then(|index| self.slot(index))
            {
                entries.retain(|e| e["id"].as_u64() != Some(index));
                entries.push(match source {
                    StyleViewerSource::Image {id} => json!({"id":index,"imageId":id,"atlasId":null,"tileIndex":0}),
                    StyleViewerSource::Atlas {id,tile} => json!({"id":index,"imageId":null,"atlasId":id.as_deref().unwrap_or("00000000-0000-0000-0000-000000000000"),"tileIndex":tile}),
                });
            } else if usize::try_from(index)
                .ok()
                .is_some_and(|i| self.uses_default(i))
                && !entries.iter().any(|e| e["id"].as_u64() == Some(index))
                && let Some(default) = defaults.iter().find(|e| e["id"].as_u64() == Some(index))
            {
                entries.push(default.clone());
            }
        }
        entity["generalImageEntries"] = Value::Array(entries);
        if self.slots.is_some() {
            for key in ["generalImageId", "generalAtlasId", "generalTileIndex"] {
                entity[key] = Value::Null;
            }
        }
    }
    pub fn slot(&self, index: usize) -> Option<&StyleViewerSource> {
        match &self.slots {
            Some(slots) => slots.get(index).and_then(Option::as_ref),
            None => self.image.as_ref(),
        }
    }
    pub fn set_slot(
        &mut self,
        index: usize,
        source: Option<StyleViewerSource>,
    ) -> Result<(), String> {
        if index >= 3 {
            return Err("Choose preview image ID 0, 1 or 2.".into());
        }
        let mut defaults = std::array::from_fn(|i| self.uses_default(i));
        defaults[index] = false;
        self.default_slots = Some(defaults);
        let slots = self
            .slots
            .get_or_insert_with(|| std::array::from_fn(|_| self.image.clone()));
        slots[index] = source;
        self.image = None;
        self.validate()
    }
    pub fn unlock_number(&self) -> Result<serde_json::Number, String> {
        use std::str::FromStr as _;
        if self.unlock_value.len() > 64 {
            return Err("Enter a valid unlock value.".into());
        }
        rust_decimal::Decimal::from_str(&self.unlock_value)
            .map_err(|_| "Enter a valid unlock value.".to_owned())?
            .normalize()
            .to_string()
            .parse()
            .map_err(|_| "Enter a valid unlock value.".into())
    }
    pub fn validate(&self) -> Result<(), String> {
        self.unlock_number()?;
        for source in std::iter::once(self.image.as_ref())
            .chain(self.slots.iter().flatten().map(Option::as_ref))
        {
            let id = match source {
                Some(StyleViewerSource::Image { id }) => Some(id),
                Some(StyleViewerSource::Atlas { id, tile }) => {
                    if *tile > i32::MAX as u64 {
                        return Err("Invalid atlas tile.".into());
                    }
                    id.as_ref()
                }
                None => None,
            };
            if id.is_some_and(|id| !crate::document::valid_id(id)) {
                return Err("Invalid preview artwork identity.".into());
            }
        }
        Ok(())
    }
    pub fn from_project(project: &serde_json::Value) -> Self {
        project
            .get("styleViewerPreview")
            .and_then(|value| serde_json::from_value::<Self>(value.clone()).ok())
            .filter(|value| value.validate().is_ok())
            .unwrap_or_default()
    }
}

/// Local metadata. This is not a Steam API or worker request payload.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind", deny_unknown_fields)]
pub enum SteamAchievementSet {
    BaseGame,
    Custom { id: String, name: String },
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SteamAchievementSetLink {
    pub style_id: String,
    pub app_id: u32,
    pub set: SteamAchievementSet,
}
impl SteamAchievementSetLink {
    pub fn validate(&self) -> Result<(), String> {
        achimeld_core::SteamAppId::new(self.app_id).map_err(|error| error.to_string())?;
        if let SteamAchievementSet::Custom { id, name } = &self.set
            && (id.is_empty()
                || id.len() > 128
                || !id
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || b"_-".contains(&byte))
                || name.trim().is_empty()
                || name.len() > 256
                || name.chars().any(char::is_control))
        {
            return Err("Enter the Steam set ID and a set name (up to 256 characters).".into());
        }
        Ok(())
    }
    pub fn same_target(&self, other: &Self) -> bool {
        self.app_id == other.app_id
            && match (&self.set, &other.set) {
                (SteamAchievementSet::BaseGame, SteamAchievementSet::BaseGame) => true,
                (
                    SteamAchievementSet::Custom { id: first, .. },
                    SteamAchievementSet::Custom { id: second, .. },
                ) => first == second,
                _ => false,
            }
    }
}

#[derive(Clone)]
pub struct StyleViewerImage {
    pub id: String,
    pub name: String,
}

#[cfg(test)]
mod default_preview_tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn editing_one_default_slot_preserves_the_others_and_legacy_clears_stay_empty() {
        let defaults: Vec<_> = (0..3).map(|id| json!({"id":id,"imageId":"cup"})).collect();
        let mut entity = json!({"icon":{"layers":(0..3).map(|id| json!({"kind":"AchievementImage","generalImageIndex":id})).collect::<Vec<_>>()}});
        let mut preview = StyleViewerPreview::default();
        preview.set_slot(1, None).unwrap();
        preview.apply_images(&mut entity, &defaults);
        let images = crate::general_image_entries(&entity);
        assert_eq!(
            images
                .iter()
                .map(|e| e["id"].as_u64().unwrap())
                .collect::<Vec<_>>(),
            [0, 2]
        );
        let loaded: StyleViewerPreview =
            serde_json::from_value(json!({"slots":[null,null,null],"unlockValue":"10"})).unwrap();
        loaded.apply_images(&mut entity, &defaults);
        assert!(crate::general_image_entries(&entity).is_empty());
        let round_trip: StyleViewerPreview =
            serde_json::from_value(serde_json::to_value(&preview).unwrap()).unwrap();
        assert_eq!(preview, round_trip);
    }
}

#[derive(Clone)]
pub struct StyleViewerCollection {
    pub id: String,
    pub name: String,
    pub styles: Vec<IconStyle>,
    pub dirty: bool,
    pub read_only: bool,
    pub steam_sets: Vec<SteamAchievementSetLink>,
}

#[derive(Clone, Default)]
pub struct StyleViewerState {
    pub styles: Vec<IconStyle>,
    pub collections: Vec<StyleViewerCollection>,
    pub active_collection: Option<String>,
    pub selected_style: Option<String>,
    pub images: Vec<StyleViewerImage>,
    pub busy: bool,
    pub dirty: bool,
    pub status: String,
    pub steam_sets: Vec<SteamAchievementSetLink>,
    pub preview: StyleViewerPreview,
}

impl StyleViewerState {
    pub fn selected(&self) -> Option<IconStyle> {
        let id = self.selected_style.as_ref()?;
        if let Some(style) = self.styles.iter().find(|s| &s.id == id) {
            return Some(style.clone());
        }
        let (collection, local) = id.split_once('/')?;
        let mut style = self
            .collections
            .iter()
            .find(|c| c.id == collection && c.read_only && c.name == "Built-in")?
            .styles
            .iter()
            .find(|s| s.id == local)?
            .clone();
        style.id = id.clone();
        Some(style)
    }
}
