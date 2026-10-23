use serde_json::{Value, json};
/// Migrate legacy general sources only when a user changes the list.
pub fn general_image_entries(entity: &Value) -> Vec<Value> {
    let list = entity["generalImageEntries"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    if !list.is_empty() {
        return list;
    }
    let mut list = Vec::new();
    if !entity["generalImageId"].is_null() {
        list.push(json!({"id":0,"imageId":entity["generalImageId"],"atlasId":null,"tileIndex":0}));
    }
    if !entity["generalAtlasId"].is_null() {
        list.push(json!({"id":list.len(),"imageId":null,"atlasId":entity["generalAtlasId"],"tileIndex":entity["generalTileIndex"].as_u64().unwrap_or(0)}));
    }
    list
}
pub fn store_general_image_entries(entity: &mut Value, mut list: Vec<Value>) {
    for (index, entry) in list.iter_mut().enumerate() {
        entry["id"] = json!(index);
    }
    entity["generalImageEntries"] = json!(list);
    for key in ["generalImageId", "generalAtlasId", "generalTileIndex"] {
        entity[key] = Value::Null;
    }
}
pub fn append_general_image(entity: &mut Value, source: Value) -> i64 {
    let mut list = general_image_entries(entity);
    let id = list.len() as i64;
    list.push(source);
    store_general_image_entries(entity, list);
    id
}
