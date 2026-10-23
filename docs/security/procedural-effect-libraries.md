# Procedural effect libraries

Explosion Explorer and Particle Explorer keep self-contained named presets in the
host-owned editor state. A preset stores one procedural layer: its complete typed
settings (including gradient stops, alpha, shape components and particle color
maps), opacity, blend, transforms, visibility and layer effects. Presets do not
store filesystem references or copy project-specific folder/mask connections.

`effect_presets::EffectPreset::validate` permits only the two closed effect kinds
and a fixed set of layer fields, and reuses icon recipe validation. Settings and
embedded colors remain validated by their existing models. A library has at most
128 entries per kind; files have an 8 MiB limit and an explicit format/version/kind.
Names, identities, duplicate identities and individual recipe sizes are checked.
Malformed or mismatched collections cannot partially replace the saved library.
Particle Explorer also imports the bounded legacy single-effect format.

Presentation sends closed ImportEffects/ExportEffects intents without paths.
The existing standard Windows picker owns path selection. The supervised host
bounds both metadata length and the actual read, decodes and validates the whole
collection, then persists the new library atomically through EditorState. Imported
entries receive fresh library identities and retain independent gradient data.
Export writes and syncs a temporary sibling before publishing with a hard link;
it fails rather than replacing an existing destination. Native picker cancellation
does not apply an effect or modify the library. No worker operation, process recipe,
or browser capability is added.

Applying a preset changes only its approved settings fields on the currently
edited matching layer. The target layer ID, display name, parent and mask links
are retained. The ordinary inspector/history path commits the recipe changes,
keeping undo/redo. Library edits remain separate from project history.

Only the selected draft requests an image preview. One dedicated pool of two
workers uses the existing bounded queue/cache and progressive renderer. A stable
temporary document keeps the previous successful image while obsolete work is
cancelled. Layer effects still use composition depth/work/cancellation limits
under SEC-RENDER-001. Collection persistence uses a fixed local settings path;
the UI never performs file access.

Evidence: `procedural_collections_embed_gradients_maps_shapes_and_layer_properties`,
`procedural_collection_validation_rejects_external_references_limits_and_partial_payloads`,
`procedural_effect_collections_persist_atomically_and_preview_without_editing_projects`,
and the explorer save/repair/narrow-panel regressions. Native screenshot helpers
write visual evidence outside source. These are bounded source and synthetic
fixtures, not installed Windows enforcement or live Steam qualification; full
audit reports retain any failed, skipped or source-stability checks explicitly.
