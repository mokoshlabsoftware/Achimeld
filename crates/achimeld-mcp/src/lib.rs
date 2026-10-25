//! Achimeld's stdio MCP facade. No browser, process launch, or remote mutation tools.
#![forbid(unsafe_code)]
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::io::{BufRead, Write};
#[cfg(any(windows, test))]
mod lifetime;

pub const MAXIMUM_BYTES: usize = 16 * 1024 * 1024;
/// Leaves room for CSV text to be escaped in both the tool result and MCP response.
pub const MAXIMUM_INLINE_TRANSLATION_BYTES: usize = 2 * 1024 * 1024;
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Request {
    pub operation: String,
    pub arguments: Value,
    pub project_id: Option<String>,
    pub expected_revision: Option<u64>,
    pub request_id: Option<String>,
}
pub fn failure(code: &str, message: &str) -> Value {
    json!({"success":false,"errorCode":code,"error":message})
}
pub fn success(data: Value) -> Value {
    json!({"success":true,"data":data})
}

pub fn tools() -> Vec<Value> {
    let mut result = Vec::new();
    // A closed operation registry also supplies input validation on the receiving host.
    for (name, description, required, fields) in [
        (
            "list_instances",
            "List running Achimeld windows; choose processId for subsequent calls.",
            "",
            "",
        ),
        (
            "get_scene",
            "Read the open editor, projectId, revision, nodes, selection and undo state.",
            "",
            "",
        ),
        (
            "get_node",
            "Read a complete node, localization and icon recipe.",
            "nodeId",
            "nodeId:s",
        ),
        (
            "list_icon_styles",
            "Read saved project icon styles and their layer recipes.",
            "",
            "",
        ),
        (
            "list_style_collections",
            "List built-in and loaded style collections with IDs, names, required General Image slots and editability. Does not import styles into the project.",
            "",
            "",
        ),
        (
            "get_icon_style",
            "Read a collection style's complete layer recipe before cloning it. IDs are scoped to collectionId.",
            "collectionId styleId",
            "collectionId:s styleId:s",
        ),
        (
            "export_style_preview",
            "Render a collection style using Achimeld's own layer renderer, including Artifact3d and the default cup for empty General Image inputs. Write one PNG to an unused absolute local-drive destinationFilePath, without importing or applying the style.",
            "collectionId styleId destinationFilePath",
            "collectionId:s styleId:s destinationFilePath:s width:i height:i",
        ),
        (
            "create_style_collection",
            "Create an empty editable collection in Style Viewer, independently of project history. Export it to persist it.",
            "name",
            "name:s",
        ),
        (
            "create_icon_style",
            "Create a portable style in an editable collection. Supply exactly one recipe or sourceStyleId plus sourceCollectionId. Optional edits replace existing scalar properties by JSON pointer, e.g. /layers/0/opacity. Clones layer/effect IDs and remaps masks/folders. Colors use #RRGGBB or #AARRGGBB. General artwork uses AchievementImage slots. Does not apply or import it into the project.",
            "collectionId name",
            "collectionId:s name:s recipe:o sourceCollectionId:s sourceStyleId:s edits:a",
        ),
        (
            "import_icon_style",
            "Copy a portable collection style into project styles in one undo step, with new independent IDs. Does not apply it to any achievement.",
            "collectionId styleId",
            "collectionId:s styleId:s",
        ),
        (
            "export_style_collection",
            "Save a collection to an unused absolute local-drive .achv destinationFilePath with its owned companion files. Existing files are refused. Does not save the open project.",
            "collectionId destinationFilePath",
            "collectionId:s destinationFilePath:s",
        ),
        (
            "get_achievement_translations",
            "Read achievement names and descriptions as UTF-8 CSV, up to 2 MiB inline. Keep ID, Context, English and {0} placeholders unchanged. Export to a file for larger catalogs.",
            "",
            "",
        ),
        (
            "export_achievement_translations",
            "Export the achievement translation CSV to an absolute local-drive, unused destinationFilePath ending in .csv. Refuses project companion filenames. Does not save the project.",
            "destinationFilePath",
            "destinationFilePath:s",
        ),
        (
            "import_achievement_translations",
            "Import exactly one of csv (UTF-8 content up to 2 MiB) or filePath (absolute local-drive .csv path up to 64 MiB). Validate all IDs, Context, English and {0} placeholders before one undo step. Blank cells preserve current translations. Save the project in the desktop afterwards.",
            "",
            "csv:s filePath:s",
        ),
        (
            "create_achievement",
            "Create and select an achievement near column/row without overlapping cards, in one undo step. Text is English.",
            "apiName displayName description column row",
            "apiName:s displayName:s description:s column:i row:i",
        ),
        (
            "create_stat",
            "Create and select a stat near column/row without overlapping cards, in one undo step.",
            "apiName displayName type column row",
            "apiName:s displayName:s type:s column:i row:i",
        ),
        (
            "update_node",
            "Patch API name or English text; preserve omitted values and translations.",
            "nodeId",
            "nodeId:s apiName:s displayName:s description:s hidden:b",
        ),
        (
            "move_nodes",
            "Move 1–1000 nodes atomically in one undo step.",
            "nodes",
            "nodes:a",
        ),
        (
            "select_node",
            "Open the node's canvas, clear its filter and select it.",
            "nodeId",
            "nodeId:s",
        ),
        (
            "configure_stat",
            "Replace numeric configuration; omitted numbers reset to null, flags to false, setBy to GameClient.",
            "statId",
            "statId:s minimum:n maximum:n defaultValue:n incrementOnly:b setBy:s maximumChange:n aggregated:b averageRateWindowSeconds:n",
        ),
        (
            "link_achievement_stat",
            "Link to a stat and unlock threshold; omit both statId and unlockValue to unlink.",
            "achievementId",
            "achievementId:s statId:s unlockValue:n",
        ),
        (
            "add_general_image",
            "Append exactly one existing imageAssetId, atlasId with tileIndex, or absolute filePath. File artwork is copied into the Art Data Bank.",
            "achievementId",
            "achievementId:s imageAssetId:s atlasId:s tileIndex:i filePath:s displayName:s",
        ),
        (
            "apply_icon_style",
            "Apply a saved style with independent layer/effect IDs, preserving folder/mask links and general images.",
            "achievementId styleId",
            "achievementId:s styleId:s",
        ),
        (
            "export_achievement_icons",
            "Export color and grayscale _gray icon pairs. Absolute destinationDirectory; Png/Jpeg/Bmp/Tiff; width/height 1–4096, default 256. Existing matching files are replaced. Does not save the project.",
            "destinationDirectory",
            "destinationDirectory:s format:s width:i height:i",
        ),
        (
            "undo",
            "Undo the latest shared editor change, including manual changes.",
            "",
            "",
        ),
        ("redo", "Redo the latest undone editor change.", "", ""),
    ] {
        let read = matches!(
            name,
            "list_instances"
                | "get_scene"
                | "get_node"
                | "list_icon_styles"
                | "list_style_collections"
                | "get_icon_style"
                | "get_achievement_translations"
        );
        let mut properties = serde_json::Map::new();
        let mut required: Vec<String> = required.split_whitespace().map(str::to_owned).collect();
        if name != "list_instances" {
            properties.insert(
                "processId".into(),
                json!({"type":"integer","minimum":1,"maximum":u32::MAX}),
            );
            required.push("processId".into());
        }
        if !read {
            for key in ["projectId", "requestId"] {
                properties.insert(key.into(), json!({"type":"string","format":"uuid"}));
                required.push(key.into());
            }
            properties.insert(
                "expectedRevision".into(),
                json!({"type":"integer","minimum":0}),
            );
            required.push("expectedRevision".into());
        }
        for field in fields.split_whitespace() {
            let (key, kind) = field.split_once(':').expect("Static schema field");
            let mut schema = json!({"type":match kind {"s"=>"string","i"=>"integer","b"=>"boolean","a"=>"array","o"=>"object",_=>"number"}});
            if kind == "n" || (!required.iter().any(|r| r == key) && kind == "s") {
                schema["type"] = json!([schema["type"].as_str().unwrap(), "null"]);
            }
            if key == "nodes" {
                schema = json!({"type":"array","minItems":1,"maxItems":1000,"items":{"type":"object","additionalProperties":false,"required":["nodeId","column","row"],"properties":{"nodeId":{"type":"string"},"column":{"type":"integer"},"row":{"type":"integer"}}}});
            }
            if key == "edits" {
                schema = json!({"type":"array","maxItems":128,"items":{"type":"object","additionalProperties":false,"required":["path","value"],"properties":{"path":{"type":"string","maxLength":512},"value":{"type":["string","number","boolean","null"]}}}});
            }
            if key == "recipe" {
                schema["additionalProperties"] = json!(true);
            }
            if key == "type" {
                schema["enum"] = json!(["Int", "Float", "AverageRate"]);
            }
            if key == "setBy" {
                schema["enum"] = json!(["GameClient", "GameServer", "OfficialGameServer"]);
            }
            if key == "format" {
                schema["enum"] = json!(["Png", "Jpeg", "Bmp", "Tiff"]);
            }
            if matches!(key, "width" | "height") {
                schema["minimum"] = json!(1);
                schema["maximum"] = json!(if name == "export_style_preview" {
                    1024
                } else {
                    4096
                });
            }
            if key == "csv" {
                schema["maxLength"] = json!(MAXIMUM_INLINE_TRANSLATION_BYTES);
            }
            properties.insert(key.into(), schema);
        }
        result.push(json!({"name":name,"description":format!("{description} {}", if read { "" } else { "Supply current projectId/expectedRevision and a fresh GUID requestId. Identical retries return the cached result." }),"inputSchema":{"type":"object","additionalProperties":false,"properties":properties,"required":required},"annotations":{"readOnlyHint":read,"destructiveHint":!read,"openWorldHint":false}}));
    }
    result
}

pub fn validate_arguments(operation: &str, args: &Value) -> Result<(), String> {
    let tool = tools()
        .into_iter()
        .find(|t| t["name"] == operation)
        .ok_or("Unknown editor operation")?;
    validate_schema(&tool["inputSchema"], args)?;
    if operation == "import_achievement_translations" {
        if args["csv"].is_string() == args["filePath"].is_string() {
            return Err("Supply exactly one of csv or filePath".into());
        }
        if args["csv"]
            .as_str()
            .is_some_and(|csv| csv.len() > MAXIMUM_INLINE_TRANSLATION_BYTES)
        {
            return Err("Inline translation CSV exceeds 2 MiB; use filePath".into());
        }
    }
    Ok(())
}
fn validate_schema(schema: &Value, value: &Value) -> Result<(), String> {
    let matches = |kind: &str| match kind {
        "object" => value.is_object(),
        "array" => value.is_array(),
        "string" => value.is_string(),
        "integer" => value.as_i64().is_some() || value.as_u64().is_some(),
        "number" => value.is_number(),
        "boolean" => value.is_boolean(),
        "null" => value.is_null(),
        _ => false,
    };
    if !schema["type"].as_str().map(matches).unwrap_or_else(|| {
        schema["type"]
            .as_array()
            .is_some_and(|types| types.iter().any(|t| matches(t.as_str().unwrap_or(""))))
    }) {
        return Err("Argument has an invalid type".into());
    }
    if let Some(choices) = schema["enum"].as_array()
        && !choices.contains(value)
    {
        return Err("Unsupported argument value".into());
    }
    if let Some(text) = value.as_str()
        && schema["maxLength"]
            .as_u64()
            .is_some_and(|maximum| text.chars().count() as u64 > maximum)
    {
        return Err("String argument exceeds its length limit".into());
    }
    if let Some(number) = value.as_f64()
        && (schema["minimum"].as_f64().is_some_and(|n| number < n)
            || schema["maximum"].as_f64().is_some_and(|n| number > n))
    {
        return Err("Argument is outside its limits".into());
    }
    if let Some(object) = value.as_object() {
        if schema["required"]
            .as_array()
            .into_iter()
            .flatten()
            .any(|key| !object.contains_key(key.as_str().unwrap_or("")))
        {
            return Err("Missing required argument".into());
        }
        for (key, value) in object {
            if schema["additionalProperties"] == true {
                continue;
            }
            let field = schema["properties"]
                .get(key)
                .ok_or_else(|| format!("Unknown argument: {key}"))?;
            validate_schema(field, value)?;
        }
    }
    if let Some(items) = value.as_array() {
        if items.len() < schema["minItems"].as_u64().unwrap_or(0) as usize
            || items.len() > schema["maxItems"].as_u64().unwrap_or(u64::MAX) as usize
        {
            return Err("Invalid batch size".into());
        }
        for item in items {
            validate_schema(&schema["items"], item)?;
        }
    }
    Ok(())
}

#[derive(Default)]
pub struct Protocol {
    initialized: bool,
    ready: bool,
}
impl Protocol {
    pub fn respond(
        &mut self,
        message: Value,
        call: &mut impl FnMut(&str, &Value) -> Result<Value, String>,
    ) -> Option<Value> {
        let id = message.get("id").cloned();
        let error = |code, text: &str| json!({"jsonrpc":"2.0","id":id.clone().unwrap_or(Value::Null),"error":{"code":code,"message":text}});
        if !message.is_object()
            || message["jsonrpc"] != "2.0"
            || !message["method"].is_string()
            || id
                .as_ref()
                .is_some_and(|v| !v.is_string() && v.as_i64().is_none() && v.as_u64().is_none())
        {
            return Some(error(-32600, "Invalid request"));
        }
        let method = message["method"].as_str().unwrap();
        if id.is_none() {
            if method == "notifications/initialized" && self.initialized {
                self.ready = true;
            }
            return None;
        }
        let result = match method {
            "initialize" if !self.initialized => {
                let Some(requested) = message["params"]["protocolVersion"].as_str() else {
                    return Some(error(-32602, "Missing protocolVersion"));
                };
                let version = if ["2024-11-05", "2025-03-26", "2025-06-18", "2025-11-25"]
                    .contains(&requested)
                {
                    requested
                } else {
                    "2025-11-25"
                };
                self.initialized = true;
                json!({"protocolVersion":version,"capabilities":{"tools":{"listChanged":false}},"serverInfo":{"name":"achimeld","version":env!("CARGO_PKG_VERSION")},"instructions":"Controls an already open supervised Achimeld editor. Call list_instances then get_scene. Local edits share undo history. No project save/open or Steam writes are exposed."})
            }
            "ping" => json!({}),
            _ if !self.ready => return Some(error(-32000, "Initialize the MCP connection first")),
            "tools/list" => json!({"tools":tools()}),
            "tools/call" => {
                let Some(name) = message["params"]["name"].as_str() else {
                    return Some(error(-32602, "Missing tool name"));
                };
                let arguments = message["params"]
                    .get("arguments")
                    .cloned()
                    .unwrap_or(json!({}));
                if let Err(e) = validate_arguments(name, &arguments) {
                    return Some(error(-32602, &e));
                }
                match call(name, &arguments) {
                    Ok(data) => {
                        json!({"content":[{"type":"text","text":data.to_string()}],"isError":false})
                    }
                    Err(e) => json!({"content":[{"type":"text","text":e}],"isError":true}),
                }
            }
            _ => return Some(error(-32601, "Method not found")),
        };
        Some(json!({"jsonrpc":"2.0","id":id.unwrap(),"result":result}))
    }
}

pub fn serve(
    input: &mut impl BufRead,
    output: &mut impl Write,
    mut call: impl FnMut(&str, &Value) -> Result<Value, String>,
) -> Result<(), String> {
    let mut protocol = Protocol::default();
    loop {
        let mut line = Vec::new();
        let count = std::io::Read::take(&mut *input, MAXIMUM_BYTES as u64 + 1)
            .read_until(b'\n', &mut line)
            .map_err(|e| e.to_string())?;
        if count == 0 {
            return Ok(());
        }
        if count > MAXIMUM_BYTES {
            return Err("MCP message exceeds 16 MiB".into());
        }
        let response = if achimeld_security::wire::check_json_depth(&line).is_err() {
            Some(
                json!({"jsonrpc":"2.0","id":null,"error":{"code":-32700,"message":"Invalid JSON"}}),
            )
        } else {
            match serde_json::from_slice(&line) {
                Ok(message) => protocol.respond(message, &mut call),
                Err(_) => Some(
                    json!({"jsonrpc":"2.0","id":null,"error":{"code":-32700,"message":"Invalid JSON"}}),
                ),
            }
        };
        if let Some(response) = response {
            let mut bytes = serde_json::to_vec(&response).map_err(|e| e.to_string())?;
            if bytes.len() > MAXIMUM_BYTES {
                return Err("MCP response exceeds 16 MiB".into());
            }
            bytes.push(b'\n');
            output
                .write_all(&bytes)
                .and_then(|_| output.flush())
                .map_err(|e| e.to_string())?;
        }
    }
}

#[cfg(windows)]
pub fn run() -> Result<(), String> {
    run_with_discovery(|| {
        achimeld_security::editor_transport::instances()
            .map(|instances| {
                instances
                    .as_array()
                    .is_some_and(|editors| !editors.is_empty())
            })
            .map_err(|error| error.to_string())
    })
}

#[cfg(windows)]
fn run_with_discovery(
    editor_available: impl FnMut() -> Result<bool, String>,
) -> Result<(), String> {
    use std::os::windows::io::AsHandle;
    // Do not retain Rust's global stdout lock on the worker: runtime exit flushes
    // stdout, which would otherwise wait forever behind idle or blocked stdio.
    let output = std::io::stdout()
        .as_handle()
        .try_clone_to_owned()
        .map_err(|_| "MCP output unavailable")?;
    lifetime::run(
        move || {
            serve_cancellable(
                std::io::BufReader::new(std::io::stdin()),
                &mut std::fs::File::from(output),
                call_editor_cancellable,
            )
        },
        editor_available,
        std::time::Duration::from_secs(2),
    )
}

#[cfg(windows)]
pub fn call_editor(operation: &str, args: &Value) -> Result<Value, String> {
    call_editor_cancellable(operation, args, &std::sync::atomic::AtomicBool::new(false))
}

#[cfg(windows)]
pub fn call_editor_cancellable(
    operation: &str,
    args: &Value,
    cancelled: &std::sync::atomic::AtomicBool,
) -> Result<Value, String> {
    use achimeld_security::editor_transport::{self, Pipe};
    use std::time::Duration;
    validate_arguments(operation, args)?;
    if operation == "list_instances" {
        return editor_transport::instances().map_err(|e| e.to_string());
    }
    let pid = args["processId"].as_u64().ok_or("Invalid processId")? as u32;
    let mut arguments = args.clone();
    let object = arguments.as_object_mut().ok_or("Invalid arguments")?;
    for key in ["processId", "projectId", "expectedRevision", "requestId"] {
        object.remove(key);
    }
    let request = Request {
        operation: operation.into(),
        arguments,
        project_id: args["projectId"].as_str().map(str::to_owned),
        expected_revision: args["expectedRevision"].as_u64(),
        request_id: args["requestId"].as_str().map(str::to_owned),
    };
    let timeout = Duration::from_secs(if operation == "export_achievement_icons" {
        600
    } else {
        20
    });
    let mut check = || {
        if cancelled.load(std::sync::atomic::Ordering::Acquire) {
            Err(achimeld_security::SecurityError("MCP request cancelled"))
        } else {
            Ok(())
        }
    };
    check().map_err(|e| e.to_string())?;
    let mut pipe = Pipe::connect(pid).map_err(|e| e.to_string())?;
    pipe.write(&request, timeout, &mut check)
        .map_err(|e| e.to_string())?;
    let response: Value = pipe.read(timeout,&mut check).map_err(|_| "Editor disconnected, cancelled or timed out. Reuse the identical requestId and arguments; read the scene to resolve uncertain completion.")?;
    let _ = pipe.write(&"ACK", Duration::from_secs(2), &mut || Ok(()));
    if response["success"] == true {
        Ok(response["data"].clone())
    } else {
        Err(format!(
            "{}: {}",
            response["errorCode"].as_str().unwrap_or("editor_error"),
            response["error"]
                .as_str()
                .unwrap_or("Editor operation failed")
        ))
    }
}

/// The input thread receives cancellation while a pipe call awaits the UI thread.
pub fn serve_cancellable(
    input: impl BufRead + Send + 'static,
    output: &mut impl Write,
    mut call: impl FnMut(&str, &Value, &std::sync::atomic::AtomicBool) -> Result<Value, String>,
) -> Result<(), String> {
    use std::{
        collections::BTreeMap,
        sync::{
            Arc, Mutex,
            atomic::{AtomicBool, Ordering},
            mpsc,
        },
    };
    let active: Arc<Mutex<BTreeMap<String, Arc<AtomicBool>>>> = Default::default();
    let read_active = active.clone();
    let (tx, rx) = mpsc::sync_channel(8);
    let reader = std::thread::spawn(move || {
        let mut input = input;
        let result = (|| {
            loop {
                let mut line = Vec::new();
                let count = std::io::Read::take(&mut input, MAXIMUM_BYTES as u64 + 1)
                    .read_until(b'\n', &mut line)
                    .map_err(|e| e.to_string())?;
                if count == 0 {
                    break;
                }
                if count > MAXIMUM_BYTES {
                    return Err("MCP message exceeds 16 MiB".to_owned());
                }
                let message: Value = if achimeld_security::wire::check_json_depth(&line).is_ok() {
                    serde_json::from_slice(&line).unwrap_or(Value::Null)
                } else {
                    Value::Null
                };
                if message["jsonrpc"] == "2.0"
                    && message["method"] == "notifications/cancelled"
                    && message.get("id").is_none()
                {
                    let key = message["params"]["requestId"].to_string();
                    if let Some(cancelled) = read_active
                        .lock()
                        .map_err(|_| "MCP dispatcher unavailable")?
                        .get(&key)
                    {
                        cancelled.store(true, Ordering::Release);
                    }
                    continue;
                }
                let cancelled = Arc::new(AtomicBool::new(false));
                let key = message
                    .get("id")
                    .filter(|_| message["method"] == "tools/call")
                    .map(Value::to_string);
                if let Some(key) = &key {
                    let mut requests = read_active
                        .lock()
                        .map_err(|_| "MCP dispatcher unavailable")?;
                    if requests.len() >= 8 || requests.contains_key(key) {
                        return Err("Too many pending MCP requests or duplicate request ID".into());
                    }
                    requests.insert(key.clone(), cancelled.clone());
                }
                tx.try_send((message, key, cancelled))
                    .map_err(|_| "MCP input queue is full or closed")?;
            }
            Ok::<_, String>(())
        })();
        if let Ok(requests) = read_active.lock() {
            for cancelled in requests.values() {
                cancelled.store(true, Ordering::Release);
            }
        }
        result
    });
    let mut protocol = Protocol::default();
    for (message, key, cancelled) in rx {
        let response = if message.is_null() {
            Some(
                json!({"jsonrpc":"2.0","id":null,"error":{"code":-32700,"message":"Invalid JSON"}}),
            )
        } else {
            protocol.respond(message, &mut |name, args| {
                if cancelled.load(Ordering::Acquire) {
                    Err("MCP request cancelled before execution".into())
                } else {
                    call(name, args, &cancelled)
                }
            })
        };
        if let Some(key) = key {
            active
                .lock()
                .map_err(|_| "MCP dispatcher unavailable")?
                .remove(&key);
        }
        if let Some(response) = response {
            let mut bytes = serde_json::to_vec(&response).map_err(|e| e.to_string())?;
            if bytes.len() > MAXIMUM_BYTES {
                return Err("MCP response exceeds 16 MiB".into());
            }
            bytes.push(b'\n');
            output
                .write_all(&bytes)
                .and_then(|_| output.flush())
                .map_err(|e| e.to_string())?;
        }
    }
    reader.join().map_err(|_| "MCP input thread stopped")?
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cancellation_is_received_while_an_editor_call_is_in_progress() {
        use std::sync::{atomic::Ordering, mpsc};
        struct Input {
            receiver: mpsc::Receiver<Vec<u8>>,
            current: std::io::Cursor<Vec<u8>>,
        }
        impl std::io::Read for Input {
            fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
                loop {
                    let count = std::io::Read::read(&mut self.current, buffer)?;
                    if count > 0 {
                        return Ok(count);
                    }
                    match self.receiver.recv() {
                        Ok(bytes) => self.current = std::io::Cursor::new(bytes),
                        Err(_) => return Ok(0),
                    }
                }
            }
        }
        let (input_tx, input_rx) = mpsc::channel();
        let (started_tx, started_rx) = mpsc::sync_channel(1);
        let worker = std::thread::spawn(move || {
            let mut output = Vec::new();
            serve_cancellable(
                std::io::BufReader::new(Input {
                    receiver: input_rx,
                    current: std::io::Cursor::new(Vec::new()),
                }),
                &mut output,
                |_, _, cancelled| {
                    started_tx.send(()).unwrap();
                    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
                    while !cancelled.load(Ordering::Acquire) {
                        assert!(std::time::Instant::now() < deadline);
                        std::thread::sleep(std::time::Duration::from_millis(1));
                    }
                    Err("Cancelled while waiting for editor".into())
                },
            )
            .unwrap();
            output
        });
        let messages = [
            json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25"}}),
            json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
            json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"get_scene","arguments":{"processId":1}}}),
        ];
        input_tx
            .send(
                (messages
                    .iter()
                    .map(Value::to_string)
                    .collect::<Vec<_>>()
                    .join("\n")
                    + "\n")
                    .into_bytes(),
            )
            .unwrap();
        started_rx
            .recv_timeout(std::time::Duration::from_secs(3))
            .unwrap();
        input_tx.send((json!({"jsonrpc":"2.0","method":"notifications/cancelled","params":{"requestId":2}}).to_string()+"\n").into_bytes()).unwrap();
        drop(input_tx);
        let output = worker.join().unwrap();
        let responses: Vec<Value> = std::str::from_utf8(&output)
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        assert_eq!(responses[1]["result"]["isError"], true);
    }
    #[test]
    fn stdio_lifecycle_recovers_bad_json_and_keeps_stdout_protocol_only() {
        let messages = [
            json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25"}}),
            json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
            json!({"jsonrpc":"2.0","id":2,"method":"tools/list"}),
            json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"get_scene","arguments":{"processId":123}}}),
            json!({"jsonrpc":"2.0","id":4,"method":"ping"}),
        ];
        let input = format!(
            "bad json\n{}\n",
            messages
                .iter()
                .map(Value::to_string)
                .collect::<Vec<_>>()
                .join("\n")
        );
        let mut output = Vec::new();
        let mut calls = 0;
        serve(
            &mut std::io::Cursor::new(input),
            &mut output,
            |operation, args| {
                assert_eq!(operation, "get_scene");
                assert_eq!(args["processId"], 123);
                calls += 1;
                Ok(json!({"revision":1}))
            },
        )
        .unwrap();
        let responses: Vec<Value> = std::str::from_utf8(&output)
            .unwrap()
            .lines()
            .map(|s| serde_json::from_str(s).unwrap())
            .collect();
        assert_eq!(calls, 1);
        assert_eq!(responses.len(), 5);
        assert_eq!(responses[0]["error"]["code"], -32700);
        assert_eq!(responses[1]["result"]["protocolVersion"], "2025-11-25");
        assert_eq!(
            responses[2]["result"]["tools"].as_array().unwrap().len(),
            26
        );
        assert_eq!(responses[3]["result"]["isError"], false);
    }
    #[test]
    fn schema_rejects_extra_authority_malformed_batches_and_invalid_enums() {
        assert!(
            validate_arguments(
                "get_scene",
                &json!({"processId":1,"url":"https://example.com"})
            )
            .is_err()
        );
        assert!(validate_arguments("launch", &json!({})).is_err());
        let base = json!({"processId":1,"projectId":"p","requestId":"r","expectedRevision":0});
        let mut args = base.clone();
        args["nodes"] = json!([]);
        assert!(validate_arguments("move_nodes", &args).is_err());
        args["nodes"] = json!([{"nodeId":"a","column":1,"row":2,"executable":"a.exe"}]);
        assert!(validate_arguments("move_nodes", &args).is_err());
        let mut args = base;
        args["statId"] = json!("s");
        args["setBy"] = json!("Arbitrary");
        assert!(validate_arguments("configure_stat", &args).is_err());
    }
    #[test]
    fn translation_tools_require_one_bounded_input_and_mutation_pins() {
        assert!(
            validate_arguments("get_achievement_translations", &json!({"processId":1})).is_ok()
        );
        assert!(
            validate_arguments(
                "import_achievement_translations",
                &json!({"processId":1,"csv":"data"})
            )
            .is_err()
        );
        let mut args = json!({"processId":1,"projectId":"p","expectedRevision":1,"requestId":"r"});
        assert!(validate_arguments("import_achievement_translations", &args).is_err());
        args["csv"] = json!("data");
        assert!(validate_arguments("import_achievement_translations", &args).is_ok());
        args["filePath"] = json!("C:\\translations.csv");
        assert!(validate_arguments("import_achievement_translations", &args).is_err());
        args["csv"] = Value::Null;
        assert!(validate_arguments("import_achievement_translations", &args).is_ok());
        args["filePath"] = Value::Null;
        args["csv"] = json!("ż".repeat(MAXIMUM_INLINE_TRANSLATION_BYTES / 2 + 1));
        assert!(validate_arguments("import_achievement_translations", &args).is_err());
        let csv = "\0".repeat(MAXIMUM_INLINE_TRANSLATION_BYTES);
        let data = json!({"csv":csv});
        let wrapped = json!({"jsonrpc":"2.0","id":1,"result":{"content":[{"type":"text","text":data.to_string()}],"isError":false}});
        assert!(serde_json::to_vec(&wrapped).unwrap().len() < MAXIMUM_BYTES);
    }
    #[test]
    fn style_tools_bound_patches_and_keep_file_and_project_authority_explicit() {
        assert!(validate_arguments("list_style_collections", &json!({"processId":1})).is_ok());
        let args = json!({"processId":1,"projectId":"p","requestId":"r","expectedRevision":0,"collectionId":"c","name":"Cup","recipe":{"layers":[{"id":"l","kind":"Artifact3d","artifact3d":{"parts":[]}}]}});
        assert!(validate_arguments("create_icon_style", &args).is_ok());
        let mut extra = args.clone();
        extra["executable"] = json!("anything.exe");
        assert!(validate_arguments("create_icon_style", &extra).is_err());
        for edits in [
            json!([{"path":"/layers/0","value":{}}]),
            json!([{"path":"/layers/0/opacity","value":0.7,"operation":"remove"}]),
            json!(vec![json!({"path":"/layers/0/opacity","value":0.7}); 129]),
        ] {
            let mut candidate = args.clone();
            candidate["edits"] = edits;
            assert!(validate_arguments("create_icon_style", &candidate).is_err());
        }
        let mut export = json!({"processId":1,"projectId":"p","requestId":"r","expectedRevision":0,"collectionId":"c","styleId":"s","destinationFilePath":"C:\\preview.png","width":1025});
        assert!(validate_arguments("export_style_preview", &export).is_err());
        export["width"] = json!(512);
        assert!(validate_arguments("export_style_preview", &export).is_ok());
    }
    #[test]
    fn initialization_and_tool_errors_are_distinct_from_protocol_errors() {
        let mut protocol = Protocol::default();
        let mut call = |_: &str, _: &Value| Err("editor_busy".into());
        assert_eq!(
            protocol
                .respond(
                    json!({"jsonrpc":"2.0","id":1,"method":"tools/list"}),
                    &mut call
                )
                .unwrap()["error"]["code"],
            -32000
        );
        assert_eq!(
            protocol
                .respond(
                    json!({"jsonrpc":"2.0","id":2,"method":"initialize","params":{}}),
                    &mut call
                )
                .unwrap()["error"]["code"],
            -32602
        );
        protocol.respond(json!({"jsonrpc":"2.0","id":3,"method":"initialize","params":{"protocolVersion":"future"}}),&mut call);
        protocol.respond(
            json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
            &mut call,
        );
        let response=protocol.respond(json!({"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"get_scene","arguments":{"processId":1}}}),&mut call).unwrap();
        assert_eq!(response["result"]["isError"], true);
        assert!(response.get("error").is_none());
    }
    #[test]
    fn oversized_and_deep_stdio_input_never_reaches_editor() {
        let mut called = false;
        let mut output = Vec::new();
        let input = vec![b'x'; MAXIMUM_BYTES + 1];
        assert!(
            serve(&mut std::io::Cursor::new(input), &mut output, |_, _| {
                called = true;
                Ok(Value::Null)
            })
            .is_err()
        );
        let deep = format!("{}0{}\n", "[".repeat(65), "]".repeat(65));
        serve(&mut std::io::Cursor::new(deep), &mut output, |_, _| {
            called = true;
            Ok(Value::Null)
        })
        .unwrap();
        assert!(!called);
    }
}
