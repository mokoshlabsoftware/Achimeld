# Image drops and achievement modes

The original editor was inspected in `C:/Projects/Achiever/achiever`, especially `IconFileDrop`, `ImageDropTarget`, `ImageDropDialog`, `PhotoshopImportPicker`, and the working/inspector import methods. That checkout is reference material, never a build input.

| Drop location | Result |
| --- | --- |
| Empty achievement canvas | Create one achievement per imported image immediately, using the first file's name, unique API names, and General artwork used by its icon. Place the cards near the drop point without overlapping any existing or newly created card. Select the last created achievement and clear the filter. |
| Achievement card, achievement inspector, floating Layers panel | Offer General images or Icon layers; default to layers. A handled drop never falls through into canvas creation. |
| General images explorer, including a layer's General image chooser | Append directly to General images without an import prompt; PSD/PSB use the saved composite. When a General image layer is selected, point it at the first newly imported image. |
| Art Data Bank | Import owned artwork without creating achievements or changing the active tab. |
| Stats, other views, toolbars, covered or disabled editors | No achievement/artwork import. |

Drops accept PNG, JPEG, BMP, GIF, TIFF, WebP, PSD and PSB. Accepted drops bring Achimeld to the foreground. Raster images honor orientation metadata. Outside General images, Photoshop files offer the saved composite or selected raster layers, preserving transparency and document coordinates; creating achievements permits one selected layer per document. Extraction currently supports 8/16-bit RGB and grayscale with raw, PackBits and ZIP channels. Other color modes/bit depths require conversion to RGB/PNG; this is not a complete Photoshop renderer. The saved composite carries authored blend/effect appearance. Individual raster extraction does not recreate adjustment layers, vector content, layer masks or Photoshop effects.

The host processes at most one import batch, accepts at most 32 input files, enforces file/raster/batch budgets, and retains owned content-addressed PNGs in the existing Art Data Bank. Dialog cancellation and a failed preparation never partially mutate the document. A successful batch is one undo step; project replacement cancels obsolete work. Windows OLE drag positions are sampled by the supervised host because normal pointer events can stop during a file drag. Presentation receives only coordinates and closed destination intents; no worker operations or IPC shapes are added.

Single clicks select cards. Double-clicking a card's text/body enters General; double-clicking its icon enters Icon with Layers. Icon mode always shows Layers. Closing Layers leaves Icon mode. Double-clicking empty canvas clears the current selection and returns to General, including from the darkened icon canvas. Narrow inspector bars retain `Gen`, `Loc`, `Ico` labels, falling back to text without icons when necessary.

A short right-button press and release opens the achievement/stat canvas menu on the background, a card, or its artwork. Create achievement/Create stat uses the original click location and searches the entire canvas, including filtered cards, for the nearest free grid position. Delete selection acts on the highlighted group; right-clicking an unselected card selects it first. Achievement cards also offer Icon layers. Right-button dragging continues to pan without opening a menu. The upper action row retains Undo, Redo, and Sort; creation/deletion use the context menu and existing edit shortcuts.

Regression evidence covers target routing and cancellation, background/card/icon double clicks, tab labels, atomic import/undo/redo, source ownership after save, stale project rejection, folder insertion, and synthetic PSD/PSB transparency/offset/compression/truncation cases. Installed Windows and live Steam qualification remain separate.
