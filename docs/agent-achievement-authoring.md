# Achievement and icon authoring for agents

This guide is embedded in MCP at `achimeld://guides/achievement-authoring`, advertised by `resources/list` and read with `resources/read`. It applies to desktop and explicitly requested headless sessions.

## Resolve meaning before editing

- `displayName` is the achievement's player-facing name, not lettering drawn on its icon.
- `description` is the achievement's description, not icon lettering.
- `apiName` is its unique technical identifier, not a visible icon caption.
- Icon text is optional artwork content in the icon recipe or supplied image. It can be literal lettering or dynamic text. Names and descriptions do not authorize adding it.

When "text" could mean either the name or lettering, ask: "Should these words be achievement names, text drawn on the icons, or both?" Wait before making dependent edits. Do not ask again when the user already distinguished them. For "random names and descriptions, with test1 through test5 on the icons", keep the random record names separate from those five literal icon strings.

No icon text is a complete, valid choice. If the user only requests an image, do not add lettering. If they explicitly request no text, inspect reused recipes for visible text layers and dynamic lettering and inspect the final render for baked-in text too. Preserve achievement names and descriptions independently. When editing existing artwork, preserve unrelated content unless the request calls for its removal.

## Choose artwork or native styles

Achimeld MCP is not an image-generation service. For a request such as "a black-and-white icon with an hourglass", the agent obtains or generates the artwork using its own available image tools, with no lettering unless requested, then imports the resulting local image through `add_general_image`. Do not claim Achimeld generated it. If image generation and suitable artwork are unavailable, explain the missing capability and ask for artwork or an acceptable alternative. Do not silently substitute an unrelated built-in style.

In an Achimeld task, requests for "styles" or "icon styles" normally mean Achimeld's native capability, even without the word Achimeld. Use the native style workflow unless the user clearly requests generated image artwork instead. Discover collections and inspect complete recipes with `list_style_collections` and `get_icon_style`; do not assume a named hourglass recipe or a particular layer exists. Create and preview a portable style in a collection first. Native recipe authoring and rendering are distinct from AI image generation. A native style may still need externally supplied General Images. If the requested native geometry cannot be represented, report that limitation.

## Work through the tools

1. Use visible desktop by default; use headless only when explicitly requested. Call `list_instances`, then `get_scene`. Verify the intended project. Use recent-project discovery or an exact supplied `.achv` path to open it; ask if its identity is ambiguous. Independent sessions do not share unsaved state.
2. Separate the requested record names, descriptions, unique API names, optional icon lettering and artwork/colors. Generate random values on the agent side only where requested. Do not overwrite existing records to satisfy a creation request.
3. Call `create_achievement` for each record, retaining returned node IDs. Its text arguments set record metadata, not artwork. Positions resolve to available grid locations.
4. For supplied/generated artwork, call `add_general_image` for the target achievement and use the returned slot index in a suitable inspected style. Import owns a copy in the Art Data Bank. Merely attaching an image does not prove the applied recipe displays it.
5. For native styles, create an editable collection and use `create_icon_style` with an inspected source recipe or a valid complete recipe. Scalar edits must reference existing properties; do not guess indices or field names. Colors are `#RRGGBB` or `#AARRGGBB`. Literal icon text must not be replaced with an achievement-name or unlock-value binding. Preserve required masks, links and other layers. Use `export_style_preview` and inspect the actual result.
6. When the user requested application to a project, use `import_icon_style`, then `apply_icon_style` with the returned project style ID and target achievement. Attach required General Image slots first. Collection export alone neither applies a style nor saves a project.
7. Read back records and export final achievement icons with `export_achievement_icons` to an appropriate output directory. Inspect text or its absence, subject, colors and legibility at icon size. Collection previews alone cannot verify achievement-specific images and dynamic values. Report failed exports or unavailable visual inspection honestly.
8. Call `save_project` and verify success. A first save needs an unused absolute local-drive `.achv` path; later saves omit it. Save before headless disconnect, which terminates the host and loses unsaved changes. Report the saved project and completed records without claiming Steam synchronization or publication.

Every mutation needs `processId`, `projectId`, the latest `expectedRevision` and a fresh GUID `requestId`. Carry forward returned revisions. On `stale_scene`, reread and reconcile; on `editor_busy`, inspect status and resolve or wait for the current operation. Retry an uncertain request with exactly the same ID and arguments. After restart or cache loss, inspect the scene before creating replacements so retries do not duplicate achievements. Export preview destinations must be unused filenames; final icon export can replace matching output files.
