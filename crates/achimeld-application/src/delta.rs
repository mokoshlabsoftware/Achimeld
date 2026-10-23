use crate::document::Result;
use crate::{Catalog, Document};
use serde::Serialize;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Serialize)]
enum Part {
    Key(String),
    Index(usize),
}
#[derive(Serialize)]
enum JsonChange {
    Member {
        parent: Vec<Part>,
        key: String,
        before: Option<Value>,
        after: Option<Value>,
    },
    Replace {
        path: Vec<Part>,
        before: Value,
        after: Value,
    },
    Splice {
        path: Vec<Part>,
        index: usize,
        before: Vec<Value>,
        after: Vec<Value>,
    },
}
#[derive(Serialize)]
enum CatalogChange {
    Row {
        id: String,
        before: Option<BTreeMap<String, String>>,
        after: Option<BTreeMap<String, String>>,
    },
    Field {
        id: String,
        key: String,
        before: Option<String>,
        after: Option<String>,
    },
}
#[derive(Serialize, Default)]
pub(crate) struct Delta {
    project: Vec<JsonChange>,
    catalog: Vec<CatalogChange>,
}
impl Delta {
    pub(crate) fn camera(key: &str, before: Option<Value>, after: Value) -> Self {
        Self {
            project: vec![JsonChange::Member {
                parent: Vec::new(),
                key: key.to_owned(),
                before,
                after: Some(after),
            }],
            catalog: Vec::new(),
        }
    }
    pub(crate) fn camera_before(&self, camera_key: &str) -> Option<&Option<Value>> {
        if self.catalog.is_empty()
            && let [
                JsonChange::Member {
                    parent,
                    key,
                    before,
                    ..
                },
            ] = self.project.as_slice()
            && parent.is_empty()
            && key == camera_key
        {
            Some(before)
        } else {
            None
        }
    }
    /// Canvas geometry changes presentation only; every other document change
    /// conservatively invalidates previews, including unknown compatible fields.
    pub(crate) fn affects_previews(&self) -> bool {
        self.project.iter().any(|change| {
            let (path, member) = match change {
                JsonChange::Member { parent, key, .. } => (parent.as_slice(), Some(key.as_str())),
                JsonChange::Replace { path, .. } | JsonChange::Splice { path, .. } => {
                    (path.as_slice(), None)
                }
            };
            let keys: Vec<_> = path
                .iter()
                .map(|part| match part {
                    Part::Key(key) => Some(key.as_str()),
                    Part::Index(_) => None,
                })
                .chain(member.map(Some))
                .collect();
            !matches!(
                keys.as_slice(),
                [
                    Some("achievementsCamera" | "statsCamera" | "styleViewerPreview"),
                    ..
                ] | [
                    Some("achievements" | "stats"),
                    None,
                    Some("canvasColumn" | "canvasRow")
                ]
            )
        }) || !self.catalog.is_empty()
    }
    pub(crate) fn between(before: &Document, after: &Document) -> Result<Self> {
        let mut delta = Self::default();
        diff(
            before.project(),
            after.project(),
            &mut Vec::new(),
            &mut delta.project,
        )?;
        let ids: BTreeSet<_> = before
            .catalog()
            .rows
            .keys()
            .chain(after.catalog().rows.keys())
            .collect();
        for id in ids {
            match (before.catalog().rows.get(id), after.catalog().rows.get(id)) {
                (Some(a), Some(b)) if a != b => {
                    let keys: BTreeSet<_> = a.keys().chain(b.keys()).collect();
                    for key in keys {
                        if a.get(key) != b.get(key) {
                            delta.catalog.push(CatalogChange::Field {
                                id: id.clone(),
                                key: key.clone(),
                                before: a.get(key).cloned(),
                                after: b.get(key).cloned(),
                            });
                        }
                    }
                }
                (a, b) if a != b => delta.catalog.push(CatalogChange::Row {
                    id: id.clone(),
                    before: a.cloned(),
                    after: b.cloned(),
                }),
                _ => {}
            }
        }
        Ok(delta)
    }
    pub(crate) fn apply(&self, document: &mut Document, forward: bool) -> Result<()> {
        let (project, catalog) = document.history_parts();
        if forward {
            for change in &self.project {
                apply_json(project, change, true)?;
            }
            for change in &self.catalog {
                apply_catalog(catalog, change, true)?;
            }
        } else {
            for change in self.catalog.iter().rev() {
                apply_catalog(catalog, change, false)?;
            }
            for change in self.project.iter().rev() {
                apply_json(project, change, false)?;
            }
        }
        document.validate()
    }
}
fn diff(
    before: &Value,
    after: &Value,
    path: &mut Vec<Part>,
    changes: &mut Vec<JsonChange>,
) -> Result<()> {
    if before == after {
        return Ok(());
    }
    if path.len() > 64 {
        return Err("Edit is too deeply nested".into());
    }
    match (before, after) {
        (Value::Object(a), Value::Object(b)) => {
            let keys: BTreeSet<_> = a.keys().chain(b.keys()).collect();
            for key in keys {
                match (a.get(key), b.get(key)) {
                    (Some(a), Some(b)) => {
                        path.push(Part::Key(key.clone()));
                        diff(a, b, path, changes)?;
                        path.pop();
                    }
                    (a, b) => changes.push(JsonChange::Member {
                        parent: path.clone(),
                        key: key.clone(),
                        before: a.cloned(),
                        after: b.cloned(),
                    }),
                }
            }
        }
        (Value::Array(a), Value::Array(b)) if a.len() == b.len() => {
            for (i, (a, b)) in a.iter().zip(b).enumerate() {
                path.push(Part::Index(i));
                diff(a, b, path, changes)?;
                path.pop();
            }
        }
        (Value::Array(a), Value::Array(b)) => {
            let prefix = a.iter().zip(b).take_while(|(a, b)| a == b).count();
            let suffix = a[prefix..]
                .iter()
                .rev()
                .zip(b[prefix..].iter().rev())
                .take_while(|(a, b)| a == b)
                .count();
            changes.push(JsonChange::Splice {
                path: path.clone(),
                index: prefix,
                before: a[prefix..a.len() - suffix].to_vec(),
                after: b[prefix..b.len() - suffix].to_vec(),
            });
        }
        _ => changes.push(JsonChange::Replace {
            path: path.clone(),
            before: before.clone(),
            after: after.clone(),
        }),
    }
    Ok(())
}
fn at<'a>(value: &'a mut Value, path: &[Part]) -> Result<&'a mut Value> {
    let mut current = value;
    for part in path {
        current = match part {
            Part::Key(key) => current.get_mut(key),
            Part::Index(index) => current.get_mut(*index),
        }
        .ok_or("History path no longer matches the document")?;
    }
    Ok(current)
}
fn apply_json(value: &mut Value, change: &JsonChange, forward: bool) -> Result<()> {
    match change {
        JsonChange::Member {
            parent,
            key,
            before,
            after,
        } => {
            let (expected, replacement) = if forward {
                (before, after)
            } else {
                (after, before)
            };
            let map = at(value, parent)?
                .as_object_mut()
                .ok_or("History object is missing")?;
            if map.get(key) != expected.as_ref() {
                return Err("History field no longer matches the document".into());
            }
            if let Some(value) = replacement {
                map.insert(key.clone(), value.clone());
            } else {
                map.remove(key);
            }
        }
        JsonChange::Replace {
            path,
            before,
            after,
        } => {
            let (expected, replacement) = if forward {
                (before, after)
            } else {
                (after, before)
            };
            let target = at(value, path)?;
            if target != expected {
                return Err("History value no longer matches the document".into());
            }
            *target = replacement.clone();
        }
        JsonChange::Splice {
            path,
            index,
            before,
            after,
        } => {
            let (expected, replacement) = if forward {
                (before, after)
            } else {
                (after, before)
            };
            let array = at(value, path)?
                .as_array_mut()
                .ok_or("History array is missing")?;
            let end = index
                .checked_add(expected.len())
                .ok_or("History range overflow")?;
            if array.get(*index..end) != Some(expected.as_slice()) {
                return Err("History array no longer matches the document".into());
            }
            array.splice(*index..end, replacement.iter().cloned());
        }
    }
    Ok(())
}
fn apply_catalog(catalog: &mut Catalog, change: &CatalogChange, forward: bool) -> Result<()> {
    match change {
        CatalogChange::Row { id, before, after } => {
            let (expected, replacement) = if forward {
                (before, after)
            } else {
                (after, before)
            };
            if catalog.rows.get(id) != expected.as_ref() {
                return Err("History catalog row no longer matches".into());
            }
            if let Some(row) = replacement {
                catalog.rows.insert(id.clone(), row.clone());
            } else {
                catalog.rows.remove(id);
            }
        }
        CatalogChange::Field {
            id,
            key,
            before,
            after,
        } => {
            let (expected, replacement) = if forward {
                (before, after)
            } else {
                (after, before)
            };
            let row = catalog
                .rows
                .get_mut(id)
                .ok_or("History catalog row is missing")?;
            if row.get(key) != expected.as_ref() {
                return Err("History catalog field no longer matches".into());
            }
            if let Some(value) = replacement {
                row.insert(key.clone(), value.clone());
            } else {
                row.remove(key);
            }
        }
    }
    Ok(())
}
