use achimeld_core::{LANGUAGES, LocalizationId, SteamApiName};
use rand::RngCore;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::str::FromStr;

pub type Result<T> = std::result::Result<T, String>;

/// Resolve the exact unlock-value token without changing the editable template.
pub fn resolve_achievement_text(entity: &Value, template: &str) -> String {
    if !template.contains("{0}") {
        return template.to_owned();
    }
    match decimal(&entity["unlockValue"]).ok().flatten() {
        Some(value) => template.replace("{0}", &value.normalize().to_string()),
        None => template.to_owned(),
    }
}

pub(crate) fn camera_value(zoom: f64, offset_x: f64, offset_y: f64) -> Result<Value> {
    if !zoom.is_finite()
        || !(0.1..=4.0).contains(&zoom)
        || !offset_x.is_finite()
        || !offset_y.is_finite()
        || offset_x.abs() > f64::from(f32::MAX)
        || offset_y.abs() > f64::from(f32::MAX)
    {
        return Err("Invalid canvas camera".into());
    }
    Ok(json!({"zoom":zoom,"offsetX":offset_x,"offsetY":offset_y}))
}

pub fn new_id() -> String {
    let mut bytes = [0u8; 16];
    rand::thread_rng().fill_bytes(&mut bytes);
    bytes[6] = (bytes[6] & 15) | 64;
    bytes[8] = (bytes[8] & 63) | 128;
    let h = hex::encode(bytes);
    format!(
        "{}-{}-{}-{}-{}",
        &h[..8],
        &h[8..12],
        &h[12..16],
        &h[16..20],
        &h[20..]
    )
}

fn localization_id(catalog: &Catalog) -> String {
    loop {
        let id = new_id().replace('-', "")[..6].to_owned();
        if !catalog.rows.contains_key(&id) {
            return id;
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Catalog {
    pub rows: BTreeMap<String, BTreeMap<String, String>>,
}

impl Catalog {
    pub fn text(&self, id: &str, language: &str) -> &str {
        self.rows
            .get(id)
            .and_then(|row| {
                row.get(language)
                    .filter(|v| !v.is_empty())
                    .or_else(|| row.get("English"))
            })
            .map(String::as_str)
            .unwrap_or("")
    }
    fn add(&mut self, context: &str, english: &str) -> String {
        let id = localization_id(self);
        let mut row: BTreeMap<String, String> = LANGUAGES
            .into_iter()
            .map(|l| (l.into(), String::new()))
            .collect();
        row.insert("Context".into(), context.into());
        row.insert("English".into(), english.into());
        self.rows.insert(id.clone(), row);
        id
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum EntityKind {
    Achievement,
    Stat,
}
enum LocalIconKind {
    Style,
    Art,
}
impl EntityKind {
    pub fn key(self) -> &'static str {
        match self {
            Self::Achievement => "achievements",
            Self::Stat => "stats",
        }
    }
}

/// Preserve the complete compatible document, including fields the editor does not yet expose.
/// Mutation is private and only the command executor can change durable content.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Document {
    project: Value,
    catalog: Catalog,
}
impl Default for Document {
    fn default() -> Self {
        Self::new()
    }
}
impl Document {
    pub fn new() -> Self {
        Self {
            project: json!({"format":"com.achimeld.project","schemaVersion":1,"projectId":new_id(),"displayName":"Untitled project","steamAppId":0,"localizationCatalogSha256":"","assets":{"atlas":{"kind":"RelativeToProject","path":"atlas.png"},"localizationCatalog":{"kind":"RelativeToProject","path":"project.achimeld.localization.csv"},"atlases":[],"images":[]},"atlas":{"tileWidth":512,"tileHeight":512,"leftOffset":0,"topOffset":0,"horizontalSpacing":0,"verticalSpacing":0,"traversal":"RowMajor","outputSize":512,"unachievedIcon":{"mode":"Grayscale","opacity":1,"tintHex":"#808080"}},"iconStyles":[],"achievements":[],"stats":[],"tombstones":[],"syncBaselines":[],"lastRemoteSnapshot":null,"nextAchievementInternalId":1}),
            catalog: Catalog::default(),
        }
    }
    pub fn from_parts(mut project: Value, catalog: Catalog) -> Result<Self> {
        if !project.is_object()
            || ["achievements", "stats"].iter().any(|key| {
                project[*key]
                    .as_array()
                    .is_none_or(|items| items.iter().any(|item| !item.is_object()))
            })
        {
            return Err("Project and entity records must be objects".into());
        }
        if !project["nextAchievementInternalId"].is_null()
            && project["nextAchievementInternalId"]
                .as_i64()
                .is_none_or(|n| !(0..=i64::from(i32::MAX)).contains(&n))
        {
            return Err("Invalid achievement counter".into());
        }
        if project["schemaVersion"] == 0 {
            if project["projectId"]
                .as_str()
                .is_none_or(|s| !valid_id(s) || s == "00000000-0000-0000-0000-000000000000")
            {
                project["projectId"] = project
                    .get("id")
                    .cloned()
                    .ok_or("Legacy project ID missing")?;
            }
            project
                .as_object_mut()
                .ok_or("Project must be an object")?
                .remove("id");
            project["schemaVersion"] = json!(1);
        }
        if let Some(assets) = project.get_mut("assets").and_then(Value::as_object_mut) {
            for key in ["images", "atlases"] {
                assets.entry(key).or_insert_with(|| json!([]));
            }
        }
        if project["nextAchievementInternalId"].is_null()
            || project["achievements"]
                .as_array()
                .is_some_and(|items| items.iter().any(|item| item["internalId"].is_null()))
        {
            let items = project
                .get_mut("achievements")
                .and_then(Value::as_array_mut)
                .ok_or("Missing achievements")?;
            let mut count = 0;
            for item in items.iter_mut() {
                if crate::style_achievements::is_local_icon(item) {
                    item["internalId"] = json!(0);
                } else {
                    item["internalId"] = json!(count);
                    count += 1;
                }
            }
            project["nextAchievementInternalId"] = json!(count);
        }
        // Older projects contain the complete recipes but no local style library.
        // Recover shared recipes without consulting machine-specific user settings.
        if project.get("iconStyles").is_none() {
            project["iconStyles"] = json!(crate::editor_state::discover_styles(&project)?);
        }
        let document = Self { project, catalog };
        document.validate()?;
        Ok(document)
    }
    pub fn project(&self) -> &Value {
        &self.project
    }
    pub fn catalog(&self) -> &Catalog {
        &self.catalog
    }
    pub fn icon_styles(&self) -> Vec<crate::IconStyle> {
        // Construction and every history transaction validate this field.
        serde_json::from_value(self.project["iconStyles"].clone()).unwrap_or_default()
    }
    // Only the application history owner can reach these mutable parts; the public
    // document/port API remains read-only and never exposes a mutable document.
    pub(crate) fn history_parts(&mut self) -> (&mut Value, &mut Catalog) {
        (&mut self.project, &mut self.catalog)
    }
    pub fn entities(&self, kind: EntityKind) -> &[Value] {
        self.project[kind.key()]
            .as_array()
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }
    /// Steam definitions exclude local style nodes and art images, regardless of their names.
    pub fn steam_entities(&self, kind: EntityKind) -> impl Iterator<Item = &Value> {
        self.entities(kind)
            .iter()
            .filter(|entity| !crate::style_achievements::is_local_icon(entity))
    }
    /// Active and Unsynced definitions participate; Retired cards remain local.
    pub fn synchronized_entities(&self, kind: EntityKind) -> impl Iterator<Item = &Value> {
        self.steam_entities(kind).filter(|entity| {
            entity
                .get("steamSyncStatus")
                .is_none_or(|status| status != "Retired")
        })
    }
    pub fn entity(&self, kind: EntityKind, id: &str) -> Option<&Value> {
        self.entities(kind)
            .iter()
            .find(|e| e["id"].as_str() == Some(id))
    }
    pub fn display_name(&self, entity: &Value, language: &str) -> String {
        let text = self.catalog.text(
            entity["displayNameLocalizationId"].as_str().unwrap_or(""),
            language,
        );
        if text.is_empty() {
            entity["internalName"]
                .as_str()
                .or(entity["apiName"].as_str())
                .unwrap_or("")
                .into()
        } else {
            resolve_achievement_text(entity, text)
        }
    }
    pub fn description(&self, entity: &Value, language: &str) -> String {
        resolve_achievement_text(
            entity,
            self.catalog.text(
                entity["descriptionLocalizationId"].as_str().unwrap_or(""),
                language,
            ),
        )
    }
    pub fn app_id(&self) -> u32 {
        self.project["steamAppId"].as_u64().unwrap_or(0) as u32
    }
    pub fn validate(&self) -> Result<()> {
        if !matches!(
            self.project["format"].as_str(),
            Some("com.achimeld.project" | "com.achiever.project")
        ) || self.project["schemaVersion"] != 1
        {
            return Err("Unsupported project format or schema".into());
        }
        if self.project["projectId"]
            .as_str()
            .is_none_or(|s| !valid_id(s))
        {
            return Err("Invalid project ID".into());
        }
        if self.project["displayName"]
            .as_str()
            .is_none_or(|s| s.trim().is_empty() || s.len() > 1024)
        {
            return Err("A project name is required".into());
        }
        if self.project["steamAppId"]
            .as_u64()
            .is_none_or(|id| id > u32::MAX.into())
        {
            return Err("Invalid Steam App ID".into());
        }
        let mut all_ids = BTreeSet::new();
        if let Some(value) = self.project.get("styleViewerPreview") {
            serde_json::from_value::<crate::style_viewer::StyleViewerPreview>(value.clone())
                .map_err(|_| "Invalid style preview preferences")?
                .validate()?;
        }
        let styles: Vec<crate::IconStyle> =
            serde_json::from_value(self.project["iconStyles"].clone())
                .map_err(|_| "Project icon styles must be an array of styles")?;
        if styles.len() > 128 {
            return Err("Too many project icon styles".into());
        }
        let mut style_ids = BTreeSet::new();
        for style in styles {
            style.validate()?;
            if !style_ids.insert(style.id.to_ascii_lowercase()) {
                return Err("Duplicate project style identity".into());
            }
        }
        for kind in [EntityKind::Achievement, EntityKind::Stat] {
            let entities = self.project[kind.key()]
                .as_array()
                .ok_or("Missing entity array")?;
            if entities.len() > 20_000 {
                return Err("Too many project entities".into());
            }
            let mut names = BTreeSet::new();
            for e in entities {
                if let Some(status) = e.get("steamSyncStatus")
                    && (kind != EntityKind::Achievement
                        || !matches!(status.as_str(), Some("Active" | "Unsynced" | "Retired")))
                {
                    return Err("Invalid achievement synchronization status".into());
                }
                if let Some(art) = e.get("artImage")
                    && (!art.is_boolean()
                        || (art == true
                            && (kind != EntityKind::Achievement
                                || crate::style_achievements::is_style_achievement(e))))
                {
                    return Err("Art image identity must be a distinct local icon boolean".into());
                }
                if crate::art_images::is_art_image(e)
                    && (!e["progressStatId"].is_null() || !e["unlockValue"].is_null())
                {
                    return Err("Art images cannot have achievement progress".into());
                }
                if let Some(local) = e.get("styleAchievement")
                    && (!local.is_boolean() || (local == true && kind != EntityKind::Achievement))
                {
                    return Err("Icon style node identity must be an achievement boolean".into());
                }
                if let Some(link) = e.get("iconStyleId")
                    && (!crate::style_achievements::is_style_achievement(e)
                        || link
                            .as_str()
                            .is_none_or(|id| !style_ids.contains(&id.to_ascii_lowercase())))
                {
                    return Err("Icon style node must reference an existing project style".into());
                }
                let id = e["id"].as_str().ok_or("Missing entity ID")?;
                if !valid_id(id) || !all_ids.insert(id.to_ascii_lowercase()) {
                    return Err("Invalid or duplicate entity ID".into());
                }
                let api = e["apiName"].as_str().ok_or("Missing API name")?;
                SteamApiName::new(api).map_err(|e| e.to_string())?;
                if api.trim() != api || !names.insert(api.to_ascii_lowercase()) {
                    return Err("Duplicate or noncanonical API name".into());
                }
                if !matches!(
                    e["setBy"].as_str(),
                    Some("GameClient" | "GameServer" | "OfficialGameServer")
                ) {
                    return Err("Invalid Set by value".into());
                }
                for key in ["displayNameLocalizationId", "descriptionLocalizationId"] {
                    if key == "descriptionLocalizationId" && kind == EntityKind::Stat {
                        continue;
                    }
                    let id = e[key].as_str().ok_or("Missing localization ID")?;
                    LocalizationId::new(id).map_err(|e| e.to_string())?;
                    if !self.catalog.rows.contains_key(id) {
                        return Err("Missing project localization row".into());
                    }
                }
                if kind == EntityKind::Stat {
                    if !matches!(e["type"].as_str(), Some("Int" | "Float" | "AverageRate")) {
                        return Err("Invalid stat type".into());
                    }
                    let min = decimal(&e["minimum"])?;
                    let max = decimal(&e["maximum"])?;
                    let default = decimal(&e["defaultValue"])?;
                    if min.zip(max).is_some_and(|(a, b)| a > b)
                        || min.zip(default).is_some_and(|(a, b)| a > b)
                        || default.zip(max).is_some_and(|(a, b)| a > b)
                    {
                        return Err("Stat minimum, default and maximum are inconsistent".into());
                    }
                    decimal(&e["maximumChange"])?;
                    decimal(&e["averageRateWindowSeconds"])?;
                } else {
                    if crate::style_achievements::is_style_achievement(e)
                        && (!e["progressStatId"].is_null() || decimal(&e["unlockValue"])?.is_none())
                    {
                        return Err(
                            "Icon style nodes require an unlock value and no progress stat".into(),
                        );
                    }
                    if let Some(id) = e["progressStatId"].as_str()
                        && self.entity(EntityKind::Stat, id).is_none()
                    {
                        return Err("Achievement references a missing progress stat".into());
                    }
                    decimal(&e["unlockValue"])?;
                }
                crate::validation::entity(e)?;
            }
        }
        for (id, row) in &self.catalog.rows {
            LocalizationId::new(id).map_err(|e| e.to_string())?;
            if row.get("Context").is_none_or(|s| s.trim().is_empty())
                || !row.contains_key("English")
            {
                return Err("Localization requires Context and an English field".into());
            }
        }
        crate::validation::project(&self.project)?;
        crate::art_images::validate(self)?;
        Ok(())
    }
    pub(crate) fn apply(&mut self, command: &Command, depth: u8) -> Result<()> {
        if depth > 16 {
            return Err("Command transaction is too deeply nested".into());
        }
        match command {
            Command::Preferences(_) => return Err("Preferences belong to application state".into()),
            Command::SaveIconStyle(style) => {
                style.validate()?;
                let mut styles = self.icon_styles();
                if let Some(old) = styles.iter_mut().find(|s| s.id == style.id) {
                    let mut next = style.clone();
                    if old.recipe != next.recipe && old.preview_png == next.preview_png {
                        next.preview_png = None;
                    }
                    *old = next;
                } else {
                    styles.push(style.clone());
                }
                self.project["iconStyles"] = json!(styles);
            }
            Command::DeleteIconStyle(id) => {
                let mut styles = self.icon_styles();
                styles.retain(|s| s.id != *id);
                self.project["iconStyles"] = json!(styles);
            }
            Command::Batch(commands) => {
                for c in commands {
                    self.apply(c, depth + 1)?;
                }
            }
            Command::Add(kind) => {
                self.add(*kind, None, None, None, None)?;
            }
            Command::AddAt { kind, position } => {
                self.add(*kind, None, None, Some(*position), None)?;
            }
            Command::AddStyleAchievementAt { position } => {
                self.add(
                    EntityKind::Achievement,
                    None,
                    None,
                    Some(*position),
                    Some(LocalIconKind::Style),
                )?;
                let style_name = crate::style_achievements::next_style_name(self);
                let entity = self
                    .entities_mut(EntityKind::Achievement)?
                    .last_mut()
                    .ok_or("Icon style node was not created")?;
                entity["apiName"] = json!(format!(
                    "STYLE_{}",
                    entity["id"].as_str().unwrap_or("").replace('-', "")
                ));
                entity["internalName"] = json!("Icon style node");
                entity["unlockValue"] = json!(10);
                entity["unlockValueKind"] = json!("Float");
                // Each node owns a distinct project-style identity, even for equal recipes.
                entity["icon"] = json!({"layers":[{"id":new_id(),"displayName":"General image","kind":"AchievementImage","generalImageIndex":0,"visible":true,"opacity":1.0,"effects":[]}]});
                let style = crate::style_achievements::new_node_style(entity, &style_name);
                let row = entity["displayNameLocalizationId"]
                    .as_str()
                    .unwrap_or("")
                    .to_owned();
                self.catalog
                    .rows
                    .get_mut(&row)
                    .ok_or("Missing icon style node name")?
                    .insert("English".into(), "Icon style node".into());
                self.apply(&Command::SaveIconStyle(style), depth + 1)?;
            }
            Command::AddArtImageAt { position } => {
                self.add(
                    EntityKind::Achievement,
                    None,
                    None,
                    Some(*position),
                    Some(LocalIconKind::Art),
                )?;
                let entity = self
                    .entities_mut(EntityKind::Achievement)?
                    .last_mut()
                    .ok_or("Art image was not created")?;
                entity["apiName"] = json!(format!(
                    "ART_{}",
                    entity["id"].as_str().unwrap_or("").replace('-', "")
                ));
                entity["internalName"] = json!("Art image");
                entity["icon"] = json!({"layers":[]});
                let row = entity["displayNameLocalizationId"]
                    .as_str()
                    .unwrap_or("")
                    .to_owned();
                self.catalog
                    .rows
                    .get_mut(&row)
                    .ok_or("Missing art image name")?
                    .insert("English".into(), "Art image".into());
            }
            Command::ReplaceEntity { kind, id, value } => {
                if crate::art_images::is_art_image(value)
                    != self
                        .entity(*kind, id)
                        .is_some_and(crate::art_images::is_art_image)
                {
                    return Err("An edit cannot change art image identity".into());
                }
                if value["id"].as_str() != Some(id) {
                    return Err("An edit cannot change entity identity".into());
                }
                if crate::style_achievements::is_style_achievement(value)
                    != self
                        .entity(*kind, id)
                        .is_some_and(crate::style_achievements::is_style_achievement)
                {
                    return Err("An edit cannot change icon style node identity".into());
                }
                *self
                    .entities_mut(*kind)?
                    .iter_mut()
                    .find(|e| e["id"].as_str() == Some(id))
                    .ok_or("Entity no longer exists")? = value.clone();
            }
            Command::Delete { kind, ids } => {
                for id in ids {
                    if self.entity(*kind, id).is_none() {
                        return Err("Entity no longer exists".into());
                    }
                }
                let removed: Vec<_> = self
                    .entities(*kind)
                    .iter()
                    .filter(|e| ids.contains(&e["id"].as_str().unwrap_or("").to_owned()))
                    .cloned()
                    .collect();
                self.entities_mut(*kind)?
                    .retain(|e| !ids.contains(&e["id"].as_str().unwrap_or("").to_owned()));
                if *kind == EntityKind::Achievement {
                    crate::art_images::unlink_deleted(&mut self.project, ids);
                }
                if *kind == EntityKind::Stat {
                    for e in self.entities_mut(EntityKind::Achievement)? {
                        if e["progressStatId"]
                            .as_str()
                            .is_some_and(|s| ids.iter().any(|id| id == s))
                        {
                            e["progressStatId"] = Value::Null;
                            e["unlockValue"] = Value::Null;
                        }
                    }
                }
                for e in removed {
                    if !crate::style_achievements::is_local_icon(&e)
                        && self.project["syncBaselines"]
                            .as_array()
                            .is_some_and(|v| v.iter().any(|b| b["entityId"] == e["id"]))
                    {
                        self.project["tombstones"].as_array_mut().ok_or("Missing tombstone array")?.push(json!({"entityType": if *kind == EntityKind::Achievement {"Achievement"} else {"Stat"},"entityId":e["id"],"apiName":e["apiName"]}));
                    }
                }
            }
            Command::Duplicate { kind, id } => {
                let source = self
                    .entity(*kind, id)
                    .ok_or("Entity no longer exists")?
                    .clone();
                self.add(*kind, Some(source), None, None, None)?;
                if self
                    .entities(*kind)
                    .last()
                    .is_some_and(crate::style_achievements::is_style_achievement)
                {
                    let style_name = crate::style_achievements::next_style_name(self);
                    let entity = self
                        .entities_mut(*kind)?
                        .last_mut()
                        .ok_or("Missing duplicated icon style node")?;
                    let style = crate::style_achievements::new_node_style(entity, &style_name);
                    self.apply(&Command::SaveIconStyle(style), depth + 1)?;
                }
            }
            Command::ReorderLayers {
                achievement_id,
                order,
            } => {
                let entity = self
                    .entities_mut(EntityKind::Achievement)?
                    .iter_mut()
                    .find(|e| e["id"].as_str() == Some(achievement_id))
                    .ok_or("Achievement no longer exists")?;
                let layers = entity["icon"]["layers"]
                    .as_array_mut()
                    .ok_or("Icon has no layers")?;
                let unique: BTreeSet<_> = order.iter().collect();
                if order.len() != layers.len()
                    || unique.len() != order.len()
                    || order
                        .iter()
                        .any(|id| !layers.iter().any(|l| l["id"].as_str() == Some(id)))
                {
                    return Err(
                        "Layer reorder must contain every existing layer exactly once".into(),
                    );
                }
                let source = layers.clone();
                *layers = order
                    .iter()
                    .map(|id| {
                        source
                            .iter()
                            .find(|l| l["id"].as_str() == Some(id))
                            .cloned()
                            .expect("validated layer identity")
                    })
                    .collect();
            }
            Command::Move {
                kind,
                id,
                column,
                row,
            } => {
                let entity = self
                    .entities_mut(*kind)?
                    .iter_mut()
                    .find(|e| e["id"].as_str() == Some(id))
                    .ok_or("Entity no longer exists")?;
                entity["canvasColumn"] = json!(column);
                entity["canvasRow"] = json!(row);
            }
            Command::StyleViewerPreview(value) => {
                value.validate()?;
                self.project["styleViewerPreview"] = json!(value);
            }
            Command::CanvasCamera {
                kind,
                zoom,
                offset_x,
                offset_y,
            } => {
                let camera = camera_value(*zoom, *offset_x, *offset_y)?;
                self.project[match kind {
                    EntityKind::Achievement => "achievementsCamera",
                    EntityKind::Stat => "statsCamera",
                }] = camera;
            }
            Command::SetLocalization { id, language, text } => {
                if !LANGUAGES.contains(&language.as_str()) {
                    return Err("Unsupported language".into());
                }
                if text.len() > 1_048_576 {
                    return Err("Localization text is too long".into());
                }
                self.catalog
                    .rows
                    .get_mut(id)
                    .ok_or("Localization row no longer exists")?
                    .insert(language.clone(), text.clone());
            }
            Command::ProjectSettings {
                name,
                app_id,
                atlas,
                assets,
            } => {
                if self.app_id() != *app_id {
                    self.project["lastRemoteSnapshot"] = Value::Null;
                    self.project["lastVerifiedAtUtc"] = Value::Null;
                    self.project["syncBaselines"] = json!([]);
                    self.project["tombstones"] = json!([]);
                }
                self.project["displayName"] = json!(name);
                self.project["steamAppId"] = json!(app_id);
                if let Some(v) = atlas {
                    self.project["atlas"] = v.clone();
                }
                if let Some(v) = assets {
                    self.project["assets"] = v.clone();
                }
            }
            Command::DuplicateArtwork {
                atlas,
                id,
                new_id,
                name,
            } => {
                crate::art_bank::duplicate(&mut self.project, *atlas, id, new_id, name)?;
            }
            Command::DeleteArtwork { atlas, id } => {
                crate::art_bank::delete(&mut self.project, *atlas, id)?;
            }
            Command::SetRemoteSnapshot(snapshot) => {
                let id = snapshot
                    .pointer("/appId/value")
                    .and_then(Value::as_u64)
                    .or_else(|| snapshot["appId"].as_u64())
                    .ok_or("Missing snapshot App ID")?;
                if id == 0 || id != u64::from(self.app_id()) {
                    return Err("Remote snapshot belongs to another Steam App ID".into());
                }
                self.project["lastRemoteSnapshot"] = snapshot.clone();
            }
            Command::ApplySynchronization(document)
            | Command::ApplyAutomation(document)
            | Command::ImportArtwork(document) => {
                if document.project["projectId"] != self.project["projectId"]
                    || document.app_id() != self.app_id()
                {
                    return Err("Synchronization belongs to another project".into());
                }
                document.validate()?;
                if matches!(command, Command::ApplySynchronization(_)) {
                    crate::style_achievements::require_preserved(self, document)?;
                }
                *self = (**document).clone();
            }
        }
        Ok(())
    }
    fn entities_mut(&mut self, kind: EntityKind) -> Result<&mut Vec<Value>> {
        self.project[kind.key()]
            .as_array_mut()
            .ok_or_else(|| "Missing entity array".into())
    }
    pub(crate) fn add_named(
        &mut self,
        preferences: &crate::Preferences,
        position: Option<[i32; 2]>,
    ) -> Result<()> {
        self.add(
            EntityKind::Achievement,
            None,
            Some(preferences),
            position,
            None,
        )
    }
    fn add(
        &mut self,
        kind: EntityKind,
        source: Option<Value>,
        preferences: Option<&crate::Preferences>,
        near: Option<[i32; 2]>,
        local: Option<LocalIconKind>,
    ) -> Result<()> {
        let n = self.entities(kind).len();
        let local_icon = local.is_some()
            || source
                .as_ref()
                .is_some_and(crate::style_achievements::is_local_icon);
        let prefix = if kind == EntityKind::Achievement {
            "ACHIEVEMENT"
        } else {
            "STAT"
        };
        let steam_count = self.steam_entities(kind).count();
        let mut i = steam_count + 1;
        let base = source
            .as_ref()
            .and_then(|e| e["apiName"].as_str())
            .unwrap_or(prefix)
            .to_owned();
        let api = loop {
            let candidate = if let Some(preferences) = preferences {
                preferences.achievement_api_name(i)?
            } else {
                format!("{base}_{i:03}")
            };
            if !self.entities(kind).iter().any(|e| {
                e["apiName"]
                    .as_str()
                    .is_some_and(|s| s.eq_ignore_ascii_case(&candidate))
            }) {
                break candidate;
            }
            i += 1;
        };
        let display = source
            .as_ref()
            .map(|e| self.display_name(e, "English"))
            .unwrap_or_else(|| {
                format!(
                    "{} {i}",
                    if kind == EntityKind::Achievement {
                        "Achievement"
                    } else {
                        "Stat"
                    }
                )
            });
        let loc = if let Some(row) = source
            .as_ref()
            .and_then(|e| e["displayNameLocalizationId"].as_str())
            .and_then(|id| self.catalog.rows.get(id))
            .cloned()
        {
            let id = localization_id(&self.catalog);
            self.catalog.rows.insert(id.clone(), row);
            id
        } else {
            self.catalog.add(prefix, &display)
        };
        let description_row = source
            .as_ref()
            .and_then(|e| e["descriptionLocalizationId"].as_str())
            .and_then(|id| self.catalog.rows.get(id))
            .cloned();
        let position = source.as_ref().and_then(|e| {
            Some((
                e["canvasColumn"].as_i64()?,
                e["canvasRow"]
                    .as_i64()?
                    .saturating_add(i64::from(crate::canvas::cell_span(kind)[1])),
            ))
        });
        let tile = (0..=20000usize)
            .find(|tile| {
                !self
                    .entities(EntityKind::Achievement)
                    .iter()
                    .any(|e| e["tileIndex"].as_u64() == Some(*tile as u64))
            })
            .unwrap_or(n);
        let icon = if self.project["assets"]["atlases"]
            .as_array()
            .is_some_and(|a| !a.is_empty())
        {
            Value::Null
        } else {
            json!({"layers":[]})
        };
        let mut entity = source.unwrap_or_else(|| if kind == EntityKind::Achievement { json!({"hidden":false,"setBy":"GameClient","progressStatId":null,"unlockValue":null,"unlockValueKind":"Integer","tileIndex":tile,"icon":icon,"atlasId":null,"standaloneImageId":null,"generalImageEntries":[]}) } else { json!({"type":"Int","setBy":"GameClient","incrementOnly":false,"minimum":null,"maximum":null,"defaultValue":null,"maximumChange":null,"aggregated":false,"averageRateWindowSeconds":null}) });
        if kind == EntityKind::Achievement && !local_icon {
            entity["steamSyncStatus"] = json!("Unsynced");
        }
        if let Some(local) = local {
            entity[match local {
                LocalIconKind::Style => "styleAchievement",
                LocalIconKind::Art => "artImage",
            }] = json!(true);
        }
        entity["id"] = json!(new_id());
        entity["apiName"] = json!(api);
        entity["internalName"] = json!(display);
        entity["displayNameLocalizationId"] = json!(loc);
        entity["displayOrder"] = json!(
            self.entities(kind)
                .iter()
                .filter(|entity| local_icon || !crate::style_achievements::is_local_icon(entity))
                .filter_map(|e| e["displayOrder"].as_u64())
                .max()
                .map_or(0, |n| n + 1)
        );
        let automatic = crate::canvas::auto_position(kind, n);
        let near =
            near.unwrap_or_else(|| position.map_or(automatic, |(x, y)| [x as i32, y as i32]));
        let position = crate::canvas::vacant_position(kind, self.entities(kind), near);
        entity["canvasColumn"] = json!(position[0]);
        entity["canvasRow"] = json!(position[1]);
        if kind == EntityKind::Achievement {
            let description = if let Some(row) = description_row {
                let id = localization_id(&self.catalog);
                self.catalog.rows.insert(id.clone(), row);
                id
            } else {
                self.catalog.add("Achievement description", "Description")
            };
            entity["descriptionLocalizationId"] = json!(description);
            if let Some(layers) = entity
                .get_mut("icon")
                .and_then(|v| v.get_mut("layers"))
                .and_then(Value::as_array_mut)
            {
                let ids: BTreeMap<_, _> = layers
                    .iter()
                    .filter_map(|l| l["id"].as_str().map(|s| (s.to_owned(), new_id())))
                    .collect();
                for layer in layers {
                    for key in ["id", "parentFolderId", "maskLayerId", "maskSourceLayerId"] {
                        if let Some(id) = layer[key].as_str().and_then(|s| ids.get(s)) {
                            layer[key] = json!(id);
                        }
                    }
                }
            }
            // Local icons have UUID identities and never consume achievement numbers.
            entity["internalId"] = if local_icon {
                json!(0)
            } else {
                json!(
                    self.project["nextAchievementInternalId"]
                        .as_u64()
                        .unwrap_or(steam_count as u64 + 1)
                )
            };
            if !local_icon {
                self.project["nextAchievementInternalId"] = json!(
                    entity["internalId"]
                        .as_u64()
                        .unwrap_or(1)
                        .checked_add(1)
                        .ok_or("Achievement counter is exhausted")?
                );
            }
        }
        self.entities_mut(kind)?.push(entity);
        Ok(())
    }
}

pub(crate) fn valid_id(id: &str) -> bool {
    id.len() == 36
        && id.bytes().enumerate().all(|(i, b)| {
            if [8, 13, 18, 23].contains(&i) {
                b == b'-'
            } else {
                b.is_ascii_hexdigit()
            }
        })
}
fn decimal(value: &Value) -> Result<Option<Decimal>> {
    if value.is_null() {
        return Ok(None);
    }
    Decimal::from_str(&value.to_string())
        .map(Some)
        .map_err(|_| "Invalid decimal value".into())
}

#[cfg(test)]
mod achievement_text_tests {
    use super::*;
    use crate::Editor;

    #[test]
    fn english_fields_allow_clearing_and_retyping() {
        let mut editor = Editor::default();
        editor
            .execute("Add", Command::Add(EntityKind::Achievement), None)
            .unwrap();
        let entity = editor.document().entities(EntityKind::Achievement)[0].clone();
        for key in ["displayNameLocalizationId", "descriptionLocalizationId"] {
            for text in ["", "a", "Replacement"] {
                editor
                    .execute(
                        "Text",
                        Command::SetLocalization {
                            id: entity[key].as_str().unwrap().into(),
                            language: "English".into(),
                            text: text.into(),
                        },
                        None,
                    )
                    .unwrap();
                editor.document().validate().unwrap();
            }
        }
    }

    #[test]
    fn unlock_templates_follow_values_languages_and_history_without_rewriting_the_catalog() {
        let mut editor = Editor::default();
        editor
            .execute("Add", Command::Add(EntityKind::Achievement), None)
            .unwrap();
        let mut entity = editor.document().entities(EntityKind::Achievement)[0].clone();
        let id = entity["id"].as_str().unwrap().to_owned();
        let name = entity["displayNameLocalizationId"]
            .as_str()
            .unwrap()
            .to_owned();
        let description = entity["descriptionLocalizationId"]
            .as_str()
            .unwrap()
            .to_owned();
        for (row, language, text) in [
            (&name, "English", "Find {0} sticks"),
            (&name, "Polish", "Znajdź {0} patyków"),
            (
                &description,
                "English",
                "Collect {0} sticks, then keep {0}. {1} stays.",
            ),
        ] {
            editor
                .execute(
                    "Text",
                    Command::SetLocalization {
                        id: row.clone(),
                        language: language.into(),
                        text: text.into(),
                    },
                    None,
                )
                .unwrap();
        }
        assert_eq!(
            editor.document().display_name(&entity, "English"),
            "Find {0} sticks"
        );
        let catalog = editor.document().catalog().clone();
        for (value, number) in [(json!(5), "5"), (json!(12.50), "12.5"), (json!(0), "0")] {
            entity["unlockValue"] = value;
            entity["unlockValueKind"] = json!("Float");
            editor
                .execute(
                    "Unlock",
                    Command::ReplaceEntity {
                        kind: EntityKind::Achievement,
                        id: id.clone(),
                        value: entity.clone(),
                    },
                    None,
                )
                .unwrap();
            let document = editor.document();
            let current = document.entity(EntityKind::Achievement, &id).unwrap();
            assert_eq!(
                document.display_name(current, "English"),
                format!("Find {number} sticks")
            );
            assert_eq!(
                document.display_name(current, "Polish"),
                format!("Znajdź {number} patyków")
            );
            assert_eq!(
                document.display_name(current, "Japanese"),
                format!("Find {number} sticks")
            );
            assert_eq!(
                document.description(current, "Polish"),
                format!("Collect {number} sticks, then keep {number}. {{1}} stays.")
            );
            assert_eq!(document.catalog(), &catalog);
        }
        assert!(editor.undo());
        let document = editor.document();
        assert_eq!(
            document.display_name(
                document.entity(EntityKind::Achievement, &id).unwrap(),
                "English"
            ),
            "Find 12.5 sticks"
        );
        assert!(editor.redo());
        let document = editor.document();
        assert_eq!(
            document.display_name(
                document.entity(EntityKind::Achievement, &id).unwrap(),
                "English"
            ),
            "Find 0 sticks"
        );
        assert_eq!(document.catalog(), &catalog);
    }

    #[test]
    fn unlock_text_is_literal_without_a_value_and_uses_exact_decimal_digits() {
        for (input, expected) in [
            ("1.2300", "1.23"),
            ("-0.000", "0"),
            (
                "0.0000000000000000000000000001",
                "0.0000000000000000000000000001",
            ),
            (
                "79228162514264337593543950335",
                "79228162514264337593543950335",
            ),
        ] {
            let number: Value = serde_json::from_str(input).unwrap();
            assert_eq!(
                resolve_achievement_text(&json!({"unlockValue":number}), "{0}/{0} {1}"),
                format!("{expected}/{expected} {{1}}")
            );
        }
        assert_eq!(
            resolve_achievement_text(&json!({}), "Find {0} sticks"),
            "Find {0} sticks"
        );
        assert_eq!(
            resolve_achievement_text(&json!({"unlockValue":5}), "No placeholder: {1} {0:000}"),
            "No placeholder: {1} {0:000}"
        );
    }
}

#[derive(Debug, Clone)]
pub enum Command {
    StyleViewerPreview(crate::style_viewer::StyleViewerPreview),
    Preferences(crate::Preferences),
    SaveIconStyle(crate::IconStyle),
    DeleteIconStyle(String),
    Batch(Vec<Command>),
    Add(EntityKind),
    AddAt {
        kind: EntityKind,
        position: [i32; 2],
    },
    /// A local canvas card, with its own recipe and no Steam identity or sorting participation.
    AddArtImageAt {
        position: [i32; 2],
    },
    AddStyleAchievementAt {
        position: [i32; 2],
    },
    ReplaceEntity {
        kind: EntityKind,
        id: String,
        value: Value,
    },
    Delete {
        kind: EntityKind,
        ids: Vec<String>,
    },
    Duplicate {
        kind: EntityKind,
        id: String,
    },
    ReorderLayers {
        achievement_id: String,
        order: Vec<String>,
    },
    Move {
        kind: EntityKind,
        id: String,
        column: i32,
        row: i32,
    },
    CanvasCamera {
        kind: EntityKind,
        zoom: f64,
        offset_x: f64,
        offset_y: f64,
    },
    SetLocalization {
        id: String,
        language: String,
        text: String,
    },
    ProjectSettings {
        name: String,
        app_id: u32,
        atlas: Option<Value>,
        assets: Option<Value>,
    },
    DuplicateArtwork {
        atlas: bool,
        id: String,
        new_id: String,
        name: String,
    },
    DeleteArtwork {
        atlas: bool,
        id: String,
    },
    SetRemoteSnapshot(Value),
    ApplySynchronization(Box<Document>),
    /// Validated local editor automation, committed through the shared history.
    ApplyAutomation(Box<Document>),
    /// Host-prepared, owned artwork and its complete destination edit, in one undo step.
    ImportArtwork(Box<Document>),
}
