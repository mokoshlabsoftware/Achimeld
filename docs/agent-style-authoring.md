# Achievement style authoring with MCP

Use the verified Achimeld MCP server described in [Local editor MCP](mcp-server.md).
The editor must already be open. Work in collections first; import into a project
only when the user wants a project style. Collection work does not edit canvas
achievements or enter project undo history.

## A short working loop

1. `list_instances`, then `get_scene` to choose the editor and get its identity.
2. `list_style_collections` to discover collection/style IDs and required General
   Image indices. Built-in collections are read-only. IDs are scoped to a collection;
   use both IDs, even when names or source style IDs coincide.
3. `get_icon_style` with `collectionId` and `styleId` to inspect the complete recipe.
   Start with an existing style whose geometry and effects fit the task.
4. `create_style_collection` with a short readable `name` for the new work.
5. `create_icon_style` with that target `collectionId`, a descriptive `name`,
   `sourceCollectionId`, `sourceStyleId`, and optional `edits`.
6. `export_style_preview` with the target collection/style IDs and an unused absolute
   local-drive PNG path. Inspect the actual native-rendered result, then create a
   revised sibling if needed. This also renders editable `Artifact3d` layers.
7. `export_style_collection` to an unused absolute local-drive `.achv` path to keep
   the collection. The editor remembers its path. Move the `.achv`, companion
   `.achimeld.localization.csv`, and sibling `.art-data-bank` together.
8. When requested, `import_icon_style` copies one collection recipe into the open
   project's styles in one undo step. It does not apply the recipe. To apply it,
   attach the required images using `add_general_image`, then `apply_icon_style`
   using the **returned project style ID**. Save the project through the desktop.

Every writing tool, including export, needs `processId`, `projectId`,
`expectedRevision`, and a fresh GUID `requestId`. Use the revision returned by the
last tool response; collection edits also advance it. `get_scene` refreshes the
pins after manual edits. Retry an uncertain request with exactly the same ID and
arguments. Follow `stale_scene` and `editor_busy` guidance from the main MCP guide.
An export that already succeeded can be retried identically while cached. A new
export request must use a new filename.

## Property edits

`edits` is a list of at most 128 replacements of existing **scalar** values, using
JSON pointers relative to the recipe. Read the recipe instead of guessing indices.
For example, when the first layer actually has those settings:

```json
{
  "collectionId": "<editable collection ID>",
  "name": "Aurora shine",
  "sourceCollectionId": "<source collection ID>",
  "sourceStyleId": "<source style ID>",
  "edits": [
    {"path": "/layers/0/gradient/stops/0/colorHex", "value": "#FF082F49"},
    {"path": "/layers/0/gradient/stops/1/colorHex", "value": "#FF36C9B3"},
    {"path": "/layers/4/opacity", "value": 0.55}
  ]
}
```

Add the mutation pins to this example. Color strings are **ARGB**, `#AARRGGBB`,
or opaque `#RRGGBB`; `#FF` at the beginning is alpha, not red. Keep neutral white
highlights and dark outlines readable while changing related background, overlay,
shape, text and effect colors together. Change opacity, gradient angle, scale or
light strength modestly, then inspect the preview at achievement-icon size.
Preserve layer order, hidden masks, folder links, blend modes and dynamic text
unless the user asks to change them. A hidden mask may still be essential.

The server allocates new style/layer/effect identities and remaps folder and mask
links. Do not edit identity/link/type fields through property patches. For new
layers, different types or structure, supply a complete `recipe` instead of a
source style. Its layer IDs must be valid distinct GUIDs and all internal links
must resolve. Arrays and objects cannot be replaced through scalar patches.

Portable styles use `kind: "AchievementImage"` and zero-based `generalImageIndex`
to draw achievement-specific artwork. Project image/atlas IDs and filesystem
references are rejected in created/imported portable recipes. A style can use more
than one slot; read `generalImageIndices` before applying. Missing slots are not
silently populated when applying to a real achievement.

For a style already on a canvas achievement, use `get_node` and pass its
`definition.icon` as `recipe`, after replacing any project-specific image layers
with appropriate `AchievementImage` slots. MCP deliberately does not open/save
ordinary projects or edit arbitrary files.

## Native 3D cup and defaults

The **Preview objects** collection contains the editable achievement cup made with
the existing `Artifact3d` Cup preset. Inspect it with `get_icon_style`; its bowl,
base, stem and handles remain ordinary editable parts. Clone it and adjust part
materials, camera rotation and lighting, then `export_style_preview` to produce a
transparent PNG with Achimeld's renderer. No external mesh or image generator is
required. Preview exports default to 512 × 512; each dimension is bounded to 1024.

Style Viewer uses the embedded cup PNG as fallback artwork for empty preview
recipes. Explicit image slots override the fallback; clearing slots remains
transparent. The cup and collection recipes are separate from project artwork and
styles until explicitly imported. The source asset is
`crates/achimeld-application/assets/styles/achievement-cup.png`.

## Updating shipped presets

The extracted originals are in `assets/styles/originals.json` in the application
crate. `provenance.json` records source filenames, SHA-256 hashes and original
identities. Preserve these recipes and put creative variants in `variants.json`.
No original project or external application assemblies are build inputs.

Run the developer authoring fixture explicitly to reproduce the actual MCP
JSON-RPC → Windows pipe → host workflow:

```powershell
./build/Rust.ps1 -CargoArguments @('test','-p','achimeld-app','--bin','achimeld-app','author_default_styles_through_mcp','--locked','--offline','--','--ignored','--nocapture')
```

It prints a fresh external output directory containing the collection, native cup
PNG, recipes, and MCP transcript. Review the files before copying curated assets
into source. This fixture is ignored during ordinary tests because it deliberately
creates a new authoring result. Build/test outputs stay outside the repository.
Run the relevant regression tests and `build/Audit.ps1 -Offline -Fixtures` after
source changes, and follow the repository working-version rules once the coherent
change is complete.
