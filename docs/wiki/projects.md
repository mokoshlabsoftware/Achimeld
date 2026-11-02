# Projects and files

An Achimeld project holds achievement definitions, stats, icon recipes and references to its artwork. Localized text is saved with its companion catalog, and owned artwork is stored in the project's sibling Art Data Bank folder.

Keep these files together when moving or backing up your work.

## Create open and save

Use File > New project to start a project. Use File > Open project or Open Recent to return to existing work.

Use Save for the current destination. Save as chooses a new destination and rebases artwork references. Choose unused destination filenames when prompted.

| Action | Shortcut |
| --- | --- |
| New project | Ctrl+N |
| Open project | Ctrl+O |
| Save | Ctrl+S |
| Save as | Ctrl+Shift+S |

Dialogs and text focus can change which shortcuts are available. If you have unsaved changes before opening, creating or exiting, use the Save, Discard changes or Cancel choice deliberately. Cancelling a required save also cancels the pending action.

## Project settings

File > Project settings contains the project name, Steam App ID, atlas grid and icon output settings.

You can also edit the Project name in Sync > Edit Steam App ID. Saving that dialog updates the name and App ID together. After a successful validation, Achimeld uses the pinned Steam app's public name when it is available and the project still has the default Untitled project name. A name you chose yourself is retained.

The atlas grid uses tile width and height, offsets, spacing and traversal order. These determine how a tile index resolves to artwork.

Output width and height determine icon dimensions. Unachieved icon settings set desaturation from 50% to 100%, opacity, and an optional dark-to-light gradient map with strength limited to 50%. The gradient map is off by default. Legacy Tint settings become an equivalent constant-color gradient when opened. Inspect the resulting pair before exporting or synchronizing.

The Steam App ID identifies the intended game. A local App ID field and a successful Steam connection are separate things.

## Artwork ownership

Imported artwork is copied into project-owned storage. Saved projects use a sibling `<project>.art-data-bank` directory with project-relative references.

Copy the project, its companion catalog and its artwork folder as a set. Avoid placing working projects inside disposable application build folders.

Art Data Bank provides source inspection, reload and resolution controls. Resizing an atlas changes its tile grid, so recheck the tile dimensions and references afterward.

## Create achievements from images

Right-click an empty area of the Achievements canvas and choose Import achievements from images to select multiple image files. Dropping multiple supported images onto that canvas opens the same approval window. Review each proposed achievement name and description, edit them as needed, then choose Import. Cancel leaves the project unchanged. A single-image drop keeps the existing direct import behavior.

Each approved image creates one achievement with a General image and icon layer, and the source artwork is copied into the Art Data Bank. The batch is one undoable edit. Import up to 128 supported images at a time; the prepared artwork is limited to 128 MiB. Photoshop layer files are handled by the separate layer import workflow.

## Undo and recovery

Undo and redo are shared local edit history. They do not replace saving and are not a promise to reverse an external Steam operation.

Project saves maintain paired project/catalog recovery information. If saving is interrupted or files were changed externally, preserve the files and follow the recovery message rather than overwriting them blindly.

If the editor reports invalid pending fields, correct them or use Revert to working version to discard the invalid drafts while retaining accepted project changes.

