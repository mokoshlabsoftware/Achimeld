//! Pure preparation of the original Achiever editor operations. The host owns I/O and concurrency.
use crate::{Command, Document, EntityKind, append_general_image, general_image_entries, new_id};
use rust_decimal::Decimal;
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    str::FromStr,
};

pub fn node(document: &Document, id: &str) -> Result<(EntityKind, Value), String> {
    for kind in [EntityKind::Achievement, EntityKind::Stat] {
        if let Some(entity) = document.entity(kind, id) {
            let mut localization = serde_json::Map::new();
            for key in ["displayNameLocalizationId", "descriptionLocalizationId"] {
                if let Some(id) = entity[key].as_str() {
                    localization.insert(key.into(), json!(document.catalog().rows.get(id)));
                }
            }
            return Ok((
                kind,
                json!({"kind":kind,"definition":entity,"localization":localization}),
            ));
        }
    }
    Err("Unknown node ID".into())
}
pub fn summaries(document: &Document, kind: EntityKind) -> Value {
    json!(document.entities(kind).iter().map(|e| json!({"id":e["id"],"kind":kind,"apiName":e["apiName"],"displayName":document.display_name(e,"English"),"column":e["canvasColumn"],"row":e["canvasRow"]})).collect::<Vec<_>>())
}
fn string<'a>(args: &'a Value, key: &str) -> Result<&'a str, String> {
    args[key].as_str().ok_or_else(|| format!("Missing {key}"))
}
fn position(args: &Value, key: &str) -> Result<i32, String> {
    args[key]
        .as_i64()
        .and_then(|n| i32::try_from(n).ok())
        .ok_or_else(|| format!("Invalid {key}"))
}
fn replace(document: &mut Document, kind: EntityKind, entity: Value) -> Result<(), String> {
    document.apply(
        &Command::ReplaceEntity {
            kind,
            id: string(&entity, "id")?.into(),
            value: entity,
        },
        0,
    )
}
fn text(document: &mut Document, entity: &Value, key: &str, value: &str) -> Result<(), String> {
    document.apply(
        &Command::SetLocalization {
            id: string(entity, key)?.into(),
            language: "English".into(),
            text: value.into(),
        },
        0,
    )
}

struct StatDraft {
    api_name: String,
    display_name: String,
    kind: String,
    position: Option<[i32; 2]>,
    settings: Option<Value>,
}

fn stat_drafts(args: &Value) -> Result<Vec<StatDraft>, String> {
    let mut drafts = Vec::new();
    match &args["stats"] {
        Value::String(text) => parse_stat_text(text, &mut drafts)?,
        Value::Array(entries) => {
            for entry in entries {
                drafts.push(stat_draft(entry)?);
            }
        }
        _ => return Err("Missing stats".into()),
    }
    if !(1..=1000).contains(&drafts.len()) {
        return Err("Create 1-1000 stats".into());
    }
    Ok(drafts)
}

fn parse_stat_text(text: &str, drafts: &mut Vec<StatDraft>) -> Result<(), String> {
    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line.contains('|') {
            drafts.push(stat_pipe(line)?);
            continue;
        }
        for part in line.split([',', ';']) {
            let name = part.trim();
            if name.is_empty() {
                return Err(format!("Invalid stat list entry: {line}"));
            }
            drafts.push(named_stat(name, name, "Int", None)?);
        }
    }
    Ok(())
}

fn stat_pipe(line: &str) -> Result<StatDraft, String> {
    let parts: Vec<&str> = line.split('|').map(str::trim).collect();
    if !(1..=3).contains(&parts.len()) || parts[0].is_empty() {
        return Err(format!("Invalid stat list entry: {line}"));
    }
    let display = parts
        .get(1)
        .copied()
        .filter(|part| !part.is_empty())
        .unwrap_or(parts[0]);
    let kind = parts
        .get(2)
        .copied()
        .filter(|part| !part.is_empty())
        .unwrap_or("Int");
    named_stat(parts[0], display, kind, None)
}

fn named_stat(
    api: &str,
    display: &str,
    kind: &str,
    settings: Option<Value>,
) -> Result<StatDraft, String> {
    achimeld_core::SteamApiName::new(api).map_err(|error| error.to_string())?;
    if api.trim() != api {
        return Err("Duplicate or noncanonical API name".into());
    }
    if display.trim().is_empty()
        || display.chars().count() > 1024
        || display.chars().any(char::is_control)
    {
        return Err(format!("Invalid display name for {api}"));
    }
    if !matches!(kind, "Int" | "Float" | "AverageRate") {
        return Err(format!("Invalid stat type for {api}"));
    }
    Ok(StatDraft {
        api_name: api.to_owned(),
        display_name: display.to_owned(),
        kind: kind.to_owned(),
        position: None,
        settings,
    })
}

fn stat_draft(entry: &Value) -> Result<StatDraft, String> {
    if let Some(name) = entry.as_str() {
        let name = name.trim();
        return named_stat(name, name, "Int", None);
    }
    let Some(object) = entry.as_object() else {
        return Err("Invalid stat entry".into());
    };
    for key in object.keys() {
        if !matches!(
            key.as_str(),
            "apiName"
                | "displayName"
                | "type"
                | "column"
                | "row"
                | "minimum"
                | "maximum"
                | "defaultValue"
                | "maximumChange"
                | "averageRateWindowSeconds"
                | "incrementOnly"
                | "aggregated"
                | "setBy"
        ) {
            return Err(format!("Unknown stat field: {key}"));
        }
    }
    let api = entry["apiName"].as_str().ok_or("Missing apiName")?;
    let display = match entry.get("displayName") {
        None => api,
        Some(Value::String(name)) => name.as_str(),
        _ => return Err(format!("Invalid display name for {api}")),
    };
    let kind = match entry.get("type") {
        None => "Int",
        Some(Value::String(kind)) => kind.as_str(),
        _ => return Err(format!("Invalid stat type for {api}")),
    };
    let mut draft = named_stat(api, display, kind, Some(entry.clone()))?;
    let column = optional_i32(entry, "column")?;
    let row = optional_i32(entry, "row")?;
    draft.position = match (column, row) {
        (None, None) => None,
        (Some(column), Some(row)) => Some([column, row]),
        _ => return Err("Supply both column and row, or omit both".into()),
    };
    Ok(draft)
}

fn optional_i32(entry: &Value, key: &str) -> Result<Option<i32>, String> {
    match entry.get(key) {
        None => Ok(None),
        Some(value) => value
            .as_i64()
            .and_then(|number| i32::try_from(number).ok())
            .map(Some)
            .ok_or_else(|| format!("Invalid {key}")),
    }
}

fn apply_stat_settings(entity: &mut Value, entry: &Value, api: &str) -> Result<(), String> {
    for key in [
        "minimum",
        "maximum",
        "defaultValue",
        "maximumChange",
        "averageRateWindowSeconds",
    ] {
        if let Some(value) = entry.get(key) {
            if !value.is_null() && !value.is_number() {
                return Err(format!("Invalid {key} for {api}"));
            }
            entity[key] = value.clone();
        }
    }
    for key in ["incrementOnly", "aggregated"] {
        if let Some(value) = entry.get(key) {
            entity[key] = json!(
                value
                    .as_bool()
                    .ok_or_else(|| format!("Invalid {key} for {api}"))?
            );
        }
    }
    if let Some(value) = entry.get("setBy") {
        let set_by = value
            .as_str()
            .ok_or_else(|| format!("Invalid setBy for {api}"))?;
        if !matches!(set_by, "GameClient" | "GameServer" | "OfficialGameServer") {
            return Err(format!("Invalid setBy for {api}"));
        }
        entity["setBy"] = json!(set_by);
    }
    Ok(())
}

fn add_stat(document: &mut Document, draft: &StatDraft) -> Result<Value, String> {
    let requested = draft.position.unwrap_or_else(|| {
        crate::canvas::auto_position(EntityKind::Stat, document.entities(EntityKind::Stat).len())
    });
    if requested
        .iter()
        .any(|number| !(-100_000..=100_000).contains(number))
    {
        return Err("Invalid canvas position".into());
    }
    let position = crate::canvas::vacant_position(
        EntityKind::Stat,
        document.entities(EntityKind::Stat),
        requested,
    );
    document.apply(
        &Command::AddAt {
            kind: EntityKind::Stat,
            position,
        },
        0,
    )?;
    let mut entity = document
        .entities(EntityKind::Stat)
        .last()
        .ok_or("Creation failed")?
        .clone();
    entity["apiName"] = json!(draft.api_name);
    entity["internalName"] = json!(draft.display_name);
    entity["type"] = json!(draft.kind);
    if let Some(settings) = &draft.settings {
        apply_stat_settings(&mut entity, settings, &draft.api_name)?;
    }
    text(
        document,
        &entity,
        "displayNameLocalizationId",
        &draft.display_name,
    )?;
    let id = string(&entity, "id")?.to_owned();
    let column = entity["canvasColumn"].clone();
    let row = entity["canvasRow"].clone();
    replace(document, EntityKind::Stat, entity)?;
    Ok(json!({
        "nodeId": id,
        "apiName": draft.api_name,
        "displayName": draft.display_name,
        "type": draft.kind,
        "column": column,
        "row": row
    }))
}

pub struct Prepared {
    pub document: Document,
    pub result: Value,
    pub selection: Option<(EntityKind, String)>,
}

pub fn prepare(original: &Document, operation: &str, args: &Value) -> Result<Prepared, String> {
    let mut document = original.clone();
    let mut selection = None;
    let result = match operation {
        "create_achievement" | "create_stat" => {
            let kind = if operation == "create_achievement" {
                EntityKind::Achievement
            } else {
                EntityKind::Stat
            };
            let requested = [position(args, "column")?, position(args, "row")?];
            if requested.iter().any(|n| !(-100_000..=100_000).contains(n)) {
                return Err("Invalid canvas position".into());
            }
            let position = crate::canvas::vacant_position(kind, document.entities(kind), requested);
            document.apply(&Command::AddAt { kind, position }, 0)?;
            let mut entity = document
                .entities(kind)
                .last()
                .ok_or("Creation failed")?
                .clone();
            entity["apiName"] = json!(string(args, "apiName")?);
            entity["internalName"] = json!(string(args, "displayName")?);
            text(
                &mut document,
                &entity,
                "displayNameLocalizationId",
                string(args, "displayName")?,
            )?;
            if kind == EntityKind::Achievement {
                text(
                    &mut document,
                    &entity,
                    "descriptionLocalizationId",
                    string(args, "description")?,
                )?;
            } else {
                entity["type"] = json!(string(args, "type")?);
            }
            let id = string(&entity, "id")?.to_owned();
            replace(&mut document, kind, entity)?;
            selection = Some((kind, id.clone()));
            json!({"nodeId":id})
        }
        "create_stats" => {
            let drafts = stat_drafts(args)?;
            if document.entities(EntityKind::Stat).len() + drafts.len() > 20_000 {
                return Err("Too many project entities".into());
            }
            let mut created = Vec::with_capacity(drafts.len());
            for draft in &drafts {
                if document.entities(EntityKind::Stat).iter().any(|entity| {
                    entity["apiName"]
                        .as_str()
                        .is_some_and(|name| name.eq_ignore_ascii_case(&draft.api_name))
                }) {
                    return Err(format!("Duplicate API name: {}", draft.api_name));
                }
                created.push(add_stat(&mut document, draft)?);
            }
            let id = created
                .last()
                .and_then(|entry| entry["nodeId"].as_str())
                .ok_or("Creation failed")?
                .to_owned();
            selection = Some((EntityKind::Stat, id));
            json!({"createdCount": created.len(), "stats": created})
        }
        "update_node" => {
            let id = string(args, "nodeId")?;
            let (kind, _) = node(&document, id)?;
            let mut entity = document.entity(kind, id).ok_or("Unknown node")?.clone();
            if kind == EntityKind::Stat
                && (!args["description"].is_null() || !args["hidden"].is_null())
            {
                return Err("Description and hidden apply only to achievements".into());
            }
            if let Some(api) = args["apiName"].as_str() {
                entity["apiName"] = json!(api);
            }
            if let Some(name) = args["displayName"].as_str() {
                text(&mut document, &entity, "displayNameLocalizationId", name)?;
                entity["internalName"] = json!(name);
            }
            if let Some(description) = args["description"].as_str() {
                text(
                    &mut document,
                    &entity,
                    "descriptionLocalizationId",
                    description,
                )?;
            }
            if let Some(hidden) = args["hidden"].as_bool() {
                entity["hidden"] = json!(hidden);
            }
            replace(&mut document, kind, entity)?;
            json!({"nodeId":id})
        }
        "move_nodes" => {
            let nodes = args["nodes"].as_array().ok_or("Missing nodes")?;
            if nodes.is_empty() || nodes.len() > 1000 {
                return Err("Move 1–1000 nodes".into());
            }
            let mut seen = BTreeSet::new();
            for entry in nodes {
                let id = string(entry, "nodeId")?;
                if !seen.insert(id) {
                    return Err("Duplicate node in move batch".into());
                }
                let (kind, _) = node(&document, id)?;
                document.apply(
                    &Command::Move {
                        kind,
                        id: id.into(),
                        column: position(entry, "column")?,
                        row: position(entry, "row")?,
                    },
                    0,
                )?;
            }
            json!({"movedCount":nodes.len()})
        }
        "configure_stat" => {
            let id = string(args, "statId")?;
            let mut entity = document
                .entity(EntityKind::Stat, id)
                .ok_or("Unknown stat ID")?
                .clone();
            for key in [
                "minimum",
                "maximum",
                "defaultValue",
                "maximumChange",
                "averageRateWindowSeconds",
            ] {
                entity[key] = args[key].clone();
            }
            for key in ["incrementOnly", "aggregated"] {
                entity[key] = json!(args[key].as_bool().unwrap_or(false));
            }
            entity["setBy"] = json!(args["setBy"].as_str().unwrap_or("GameClient"));
            replace(&mut document, EntityKind::Stat, entity)?;
            json!({"nodeId":id})
        }
        "link_achievement_stat" => {
            let id = string(args, "achievementId")?;
            let mut entity = document
                .entity(EntityKind::Achievement, id)
                .ok_or("Unknown achievement ID")?
                .clone();
            if args["statId"].is_null() != args["unlockValue"].is_null() {
                return Err("Supply both statId and unlockValue, or omit both".into());
            }
            entity["progressStatId"] = args["statId"].clone();
            entity["unlockValue"] = args["unlockValue"].clone();
            entity["unlockValueKind"] = json!(if args["statId"]
                .as_str()
                .and_then(|id| document.entity(EntityKind::Stat, id))
                .is_some_and(|s| s["type"] != "Int")
            {
                "Float"
            } else {
                "Integer"
            });
            replace(&mut document, EntityKind::Achievement, entity)?;
            json!({"nodeId":id})
        }
        "apply_icon_style" => {
            let id = string(args, "achievementId")?;
            let mut entity = document
                .entity(EntityKind::Achievement, id)
                .ok_or("Unknown achievement ID")?
                .clone();
            let style = document
                .icon_styles()
                .into_iter()
                .find(|s| s.id == args["styleId"].as_str().unwrap_or(""))
                .ok_or("Unknown project icon style")?;
            let mut recipe = style.recipe;
            let layers = recipe["layers"]
                .as_array_mut()
                .ok_or("Invalid style recipe")?;
            let ids: BTreeMap<_, _> = layers
                .iter()
                .map(|l| Ok((string(l, "id")?.to_owned(), new_id())))
                .collect::<Result<_, String>>()?;
            let general = general_image_entries(&entity);
            for layer in layers {
                if layer["kind"] == "AchievementImage" {
                    let index = if layer["generalImageIndex"].is_null() {
                        0
                    } else {
                        layer["generalImageIndex"]
                            .as_u64()
                            .ok_or("Invalid general image index")? as usize
                    };
                    if general.get(index).is_none() {
                        return Err(format!(
                            "Style needs general image {index}; attach it first"
                        ));
                    }
                    layer["generalImageIndex"] = json!(index);
                }
                for key in ["id", "parentFolderId", "maskLayerId", "maskSourceLayerId"] {
                    if let Some(mapped) = layer[key].as_str().and_then(|id| ids.get(id)) {
                        layer[key] = json!(mapped);
                    }
                }
                if let Some(effects) = layer["effects"].as_array_mut() {
                    for effect in effects {
                        effect["id"] = json!(new_id());
                    }
                }
            }
            entity["icon"] = recipe;
            replace(&mut document, EntityKind::Achievement, entity)?;
            json!({"nodeId":id,"styleId":args["styleId"]})
        }
        "add_general_image" => {
            if !args["filePath"].is_null() {
                return Err("File artwork must be retained by the supervised host".into());
            }
            let id = string(args, "achievementId")?;
            let mut entity = document
                .entity(EntityKind::Achievement, id)
                .ok_or("Unknown achievement ID")?
                .clone();
            let image = args["imageAssetId"].as_str();
            let atlas = args["atlasId"].as_str();
            if image.is_some() == atlas.is_some() {
                return Err("Supply exactly one imageAssetId, atlasId or filePath".into());
            }
            let (key, asset) = if let Some(image) = image {
                ("images", image)
            } else {
                ("atlases", atlas.unwrap())
            };
            if !document.project()["assets"][key]
                .as_array()
                .into_iter()
                .flatten()
                .any(|e| e["id"] == asset)
            {
                return Err("Unknown artwork ID".into());
            }
            let tile = args["tileIndex"].as_i64().unwrap_or(0);
            if tile < 0 || tile > i32::MAX.into() {
                return Err("Invalid tile index".into());
            }
            let general_id = append_general_image(
                &mut entity,
                json!({"imageId":image,"atlasId":atlas,"tileIndex":tile}),
            );
            replace(&mut document, EntityKind::Achievement, entity)?;
            json!({"nodeId":id,"generalImageId":general_id,"imageAssetId":image,"atlasId":atlas})
        }
        _ => return Err("Unsupported local editor operation".into()),
    };
    document.validate()?;
    validate_links(&document)?;
    Ok(Prepared {
        document,
        result,
        selection,
    })
}
fn decimal(value: &Value) -> Result<Option<Decimal>, String> {
    if value.is_null() {
        Ok(None)
    } else {
        Decimal::from_str(&value.to_string())
            .map(Some)
            .map_err(|_| "Invalid decimal".into())
    }
}
fn validate_links(document: &Document) -> Result<(), String> {
    for stat in document.entities(EntityKind::Stat) {
        for key in ["minimum", "maximum", "defaultValue", "maximumChange"] {
            if let Some(value) = decimal(&stat[key])?
                && ((stat["type"] == "Int" && value.fract() != Decimal::ZERO)
                    || (key == "maximumChange" && value < Decimal::ZERO))
            {
                return Err("Invalid stat numeric configuration".into());
            }
        }
        if decimal(&stat["averageRateWindowSeconds"])?.is_some_and(|n| n <= Decimal::ZERO) {
            return Err("Average rate window must be positive".into());
        }
    }
    for entity in document.entities(EntityKind::Achievement) {
        if let Some(stat_id) = entity["progressStatId"].as_str() {
            let stat = document
                .entity(EntityKind::Stat, stat_id)
                .ok_or("Missing linked stat")?;
            let value = decimal(&entity["unlockValue"])?
                .ok_or("A linked achievement requires an unlock threshold")?;
            if (stat["type"] == "Int" && value.fract() != Decimal::ZERO)
                || decimal(&stat["minimum"])?.is_some_and(|min| value < min)
                || decimal(&stat["maximum"])?.is_some_and(|max| value > max)
            {
                return Err("Unlock threshold must fit the linked stat's type and range".into());
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Editor;
    #[test]
    fn style_checks_the_actual_general_image_index_before_committing() {
        let mut editor = Editor::default();
        let achievement = achievement(&mut editor, "STYLE");
        let image = new_id();
        let style = new_id();
        let mut assets = editor.document().project()["assets"].clone();
        assets["images"] = json!([{"id":image,"displayName":"Art","image":{"kind":"RelativeToProject","path":"art.png"},"kind":"User"}]);
        editor
            .execute(
                "Assets",
                Command::ProjectSettings {
                    name: "Fixture".into(),
                    app_id: 0,
                    atlas: None,
                    assets: Some(assets),
                },
                None,
            )
            .unwrap();
        apply(
            &mut editor,
            "add_general_image",
            json!({"achievementId":achievement,"imageAssetId":image}),
        );
        let mut saved = crate::IconStyle {
            preview_png: None,
            id: style.clone(),
            name: "Ability".into(),
            recipe: json!({"layers":[{"id":new_id(),"kind":"AchievementImage","generalImageIndex":1}]}),
        };
        editor
            .execute("Style", Command::SaveIconStyle(saved.clone()), None)
            .unwrap();
        assert!(
            prepare(
                editor.document(),
                "apply_icon_style",
                &json!({"achievementId":achievement,"styleId":style})
            )
            .is_err()
        );
        saved.recipe["layers"][0]["generalImageIndex"] = json!(0);
        editor
            .execute("Fix style", Command::SaveIconStyle(saved), None)
            .unwrap();
        apply(
            &mut editor,
            "apply_icon_style",
            json!({"achievementId":achievement,"styleId":style}),
        );
        assert_eq!(
            editor
                .document()
                .entity(EntityKind::Achievement, &achievement)
                .unwrap()["icon"]["layers"][0]["generalImageIndex"],
            0
        );
    }
    fn apply(editor: &mut Editor, operation: &str, args: Value) -> Value {
        let prepared = prepare(editor.document(), operation, &args).unwrap();
        editor
            .execute(
                operation,
                Command::ApplyAutomation(Box::new(prepared.document)),
                None,
            )
            .unwrap();
        prepared.result
    }
    fn achievement(editor: &mut Editor, name: &str) -> String {
        apply(editor,"create_achievement",json!({"apiName":name,"displayName":name,"description":"Earn this achievement","column":-4,"row":3}))["nodeId"].as_str().unwrap().into()
    }
    #[test]
    fn creates_with_position_localization_and_one_shared_undo_step() {
        let mut editor = Editor::default();
        let id = achievement(&mut editor, "FIRST");
        let entity = editor
            .document()
            .entity(EntityKind::Achievement, &id)
            .unwrap();
        assert_eq!(entity["canvasColumn"], -4);
        assert_eq!(editor.document().display_name(entity, "English"), "FIRST");
        let mut duplicate = json!({"apiName":"FIRST","displayName":"Duplicate","description":"Description","column":0,"row":0});
        assert!(prepare(editor.document(), "create_achievement", &duplicate).is_err());
        duplicate["apiName"] = json!("SECOND");
        assert!(prepare(editor.document(), "create_achievement", &duplicate).is_ok());
        assert!(editor.undo());
        assert!(
            editor
                .document()
                .entities(EntityKind::Achievement)
                .is_empty()
        );
        assert!(editor.redo());
        assert!(
            editor
                .document()
                .entity(EntityKind::Achievement, &id)
                .is_some()
        );
    }
    #[test]
    fn patches_preserve_translations_and_invalid_moves_leave_source_untouched() {
        let mut editor = Editor::default();
        let id = achievement(&mut editor, "FIRST");
        let row = editor
            .document()
            .entity(EntityKind::Achievement, &id)
            .unwrap()["displayNameLocalizationId"]
            .as_str()
            .unwrap()
            .to_owned();
        editor
            .execute(
                "Polish",
                Command::SetLocalization {
                    id: row.clone(),
                    language: "Polish".into(),
                    text: "Pierwszy".into(),
                },
                None,
            )
            .unwrap();
        apply(
            &mut editor,
            "update_node",
            json!({"nodeId":id,"displayName":"Updated"}),
        );
        assert_eq!(editor.document().catalog().text(&row, "Polish"), "Pierwszy");
        let before = editor.document().clone();
        assert!(prepare(&before,"move_nodes",&json!({"nodes":[{"nodeId":id,"column":20,"row":1},{"nodeId":new_id(),"column":0,"row":1}]})).is_err());
        assert_eq!(*editor.document(), before);
    }
    #[test]
    fn linked_thresholds_and_stat_range_changes_validate_atomically() {
        let mut editor = Editor::default();
        let achievement = achievement(&mut editor, "MASTER");
        let stat = apply(
            &mut editor,
            "create_stat",
            json!({"apiName":"COUNT","displayName":"Counter","type":"Int","column":0,"row":0}),
        )["nodeId"]
            .as_str()
            .unwrap()
            .to_owned();
        apply(
            &mut editor,
            "configure_stat",
            json!({"statId":stat,"minimum":0,"maximum":100,"defaultValue":0,"incrementOnly":true}),
        );
        assert!(
            prepare(
                editor.document(),
                "link_achievement_stat",
                &json!({"achievementId":achievement,"statId":stat,"unlockValue":0.5})
            )
            .is_err()
        );
        apply(
            &mut editor,
            "link_achievement_stat",
            json!({"achievementId":achievement,"statId":stat,"unlockValue":100}),
        );
        assert!(
            prepare(
                editor.document(),
                "configure_stat",
                &json!({"statId":stat,"maximum":50})
            )
            .is_err()
        );
        apply(
            &mut editor,
            "link_achievement_stat",
            json!({"achievementId":achievement}),
        );
        assert!(
            editor
                .document()
                .entity(EntityKind::Achievement, &achievement)
                .unwrap()["progressStatId"]
                .is_null()
        );
        assert!(editor.undo());
        assert_eq!(
            editor
                .document()
                .entity(EntityKind::Achievement, &achievement)
                .unwrap()["progressStatId"],
            stat
        );
    }
    #[test]
    fn style_assignment_clones_layer_effect_and_graph_identities() {
        let mut editor = Editor::default();
        let first = achievement(&mut editor, "FIRST");
        let second = achievement(&mut editor, "SECOND");
        let style = new_id();
        let folder = new_id();
        let layer = new_id();
        let effect = new_id();
        editor.execute("Style",Command::SaveIconStyle(crate::IconStyle{preview_png:None,id:style.clone(),name:"Saved".into(),recipe:json!({"layers":[{"id":folder,"kind":"Folder","maskLayerId":layer},{"id":layer,"kind":"Solid","parentFolderId":null,"solidColorHex":"#FFFFFFFF","effects":[{"id":effect,"kind":"Blur","radius":1.0}]}]})}),None).unwrap();
        for id in [&first, &second] {
            apply(
                &mut editor,
                "apply_icon_style",
                json!({"achievementId":id,"styleId":style}),
            );
        }
        let a = &editor
            .document()
            .entity(EntityKind::Achievement, &first)
            .unwrap()["icon"]["layers"];
        let b = &editor
            .document()
            .entity(EntityKind::Achievement, &second)
            .unwrap()["icon"]["layers"];
        assert_ne!(a[0]["id"], b[0]["id"]);
        assert_ne!(a[1]["effects"][0]["id"], b[1]["effects"][0]["id"]);
        assert_eq!(a[0]["maskLayerId"], a[1]["id"]);
        assert_ne!(a[1]["id"], layer);
    }
    #[test]
    fn bulk_stat_creation_is_one_undo_step() {
        let mut editor = Editor::default();
        let existing = apply(
            &mut editor,
            "create_stat",
            json!({"apiName":"EXISTING","displayName":"Existing","type":"Int","column":0,"row":0}),
        )["nodeId"]
            .as_str()
            .unwrap()
            .to_owned();
        let before = editor.document().clone();
        assert!(
            prepare(
                editor.document(),
                "create_stats",
                &json!({"stats":"EXISTING"})
            )
            .is_err()
        );
        assert_eq!(*editor.document(), before);
        let batch = json!({"stats":[
            "KILLS",
            {"apiName":"DISTANCE","displayName":"Distance","type":"Float","minimum":0,"maximum":10.5,"defaultValue":1,"incrementOnly":true,"setBy":"GameServer"},
            {"apiName":"SPEED","displayName":"Speed","type":"AverageRate","averageRateWindowSeconds":30,"aggregated":true,"column":40,"row":20}
        ]});
        let prepared = prepare(editor.document(), "create_stats", &batch).unwrap();
        let last = prepared.result["stats"].as_array().unwrap().last().unwrap()["nodeId"]
            .as_str()
            .unwrap();
        assert_eq!(
            prepared.selection,
            Some((EntityKind::Stat, last.to_owned()))
        );
        assert_eq!(prepared.result["createdCount"], 3);
        editor
            .execute(
                "create_stats",
                Command::ApplyAutomation(Box::new(prepared.document)),
                None,
            )
            .unwrap();
        {
            let stat = |name: &str| {
                editor
                    .document()
                    .entities(EntityKind::Stat)
                    .iter()
                    .find(|entity| entity["apiName"] == name)
                    .unwrap()
                    .clone()
            };
            let kills = stat("KILLS");
            let distance = stat("DISTANCE");
            let speed = stat("SPEED");
            assert_eq!(editor.document().display_name(&kills, "English"), "KILLS");
            assert_eq!(kills["type"], "Int");
            assert!(kills["minimum"].is_null());
            let packed = crate::canvas::auto_position(EntityKind::Stat, 1);
            assert_eq!(kills["canvasColumn"], packed[0]);
            assert_eq!(kills["canvasRow"], packed[1]);
            assert_eq!(
                editor.document().display_name(&distance, "English"),
                "Distance"
            );
            assert_eq!(distance["type"], "Float");
            assert_eq!(distance["minimum"], 0);
            assert_eq!(distance["maximum"], 10.5);
            assert_eq!(distance["defaultValue"], 1);
            assert_eq!(distance["incrementOnly"], true);
            assert_eq!(distance["setBy"], "GameServer");
            assert_eq!(speed["type"], "AverageRate");
            assert_eq!(speed["averageRateWindowSeconds"], 30);
            assert_eq!(speed["aggregated"], true);
            assert_eq!(speed["canvasColumn"], 40);
            assert_eq!(speed["canvasRow"], 20);
        }
        assert!(editor.undo());
        assert_eq!(editor.document().entities(EntityKind::Stat).len(), 1);
        assert!(
            editor
                .document()
                .entity(EntityKind::Stat, &existing)
                .is_some()
        );
        let text = apply(
            &mut editor,
            "create_stats",
            json!({"stats":"# comment\nKILLS\nDEATHS | Deaths | Float\nWINS, SCORE"}),
        );
        assert_eq!(text["createdCount"], 4);
        {
            let stat = |name: &str| {
                editor
                    .document()
                    .entities(EntityKind::Stat)
                    .iter()
                    .find(|entity| entity["apiName"] == name)
                    .unwrap()
                    .clone()
            };
            assert_eq!(stat("DEATHS")["type"], "Float");
            assert_eq!(
                editor.document().display_name(&stat("DEATHS"), "English"),
                "Deaths"
            );
            assert_eq!(stat("WINS")["type"], "Int");
            assert_eq!(stat("SCORE")["type"], "Int");
            assert_eq!(
                stat("KILLS")["canvasColumn"],
                crate::canvas::auto_position(EntityKind::Stat, 1)[0]
            );
        }
        assert!(editor.undo());
        assert_eq!(editor.document().entities(EntityKind::Stat).len(), 1);
        let unchanged = editor.document().clone();
        for stats in [
            json!({"stats":[{"apiName":"FRACTION","minimum":0.5}]}),
            json!({"stats":[{"apiName":"PARTIAL","column":1}]}),
            json!({"stats":"\n# only\n"}),
            json!({"stats":["KILLS","kills"]}),
            json!({"stats":"BAD | Bad | String"}),
            json!({"stats":[{"apiName":"EXTRA","path":"C:\\x"}]}),
        ] {
            assert!(prepare(editor.document(), "create_stats", &stats).is_err());
            assert_eq!(*editor.document(), unchanged);
        }
        let many: Vec<_> = (0..1001).map(|index| format!("S{index}")).collect();
        assert!(prepare(editor.document(), "create_stats", &json!({"stats": many})).is_err());
        assert_eq!(*editor.document(), unchanged);
    }
}
