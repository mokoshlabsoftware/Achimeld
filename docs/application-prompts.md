# Application prompt windows

This is the official format for Achimeld's in-app user prompts. Use it for confirmations, warnings, errors, questions, and acknowledgements. The reference is the **Unsaved changes** dialog implemented in [`app_dialog.rs`](../crates/achimeld-app/src/app_dialog.rs), including its `unsaved_prompt` layout.

## Structure and appearance

Every prompt has three full-width sections in this order:

| Section | Required appearance |
| --- | --- |
| Header | Solid black background. Place the relevant icon at the left with a 24-unit inset, vertically centered. Center the title independently across the full header, horizontally and vertically. Use the application's heading font. |
| Body | Solid black background. Center the message horizontally, including each line when text wraps. Keep comfortable, balanced space around it. Use the application's body font. |
| Bottom actions | Gray button bodies form one continuous, full-width row. Each action fills the row's height with square corners, without outer padding or gaps between adjacent buttons. Center each icon and label together. |

Only the bottom action row has a gray background. The header and body use opaque black, even when the surrounding panels are gray. This explicit black fill is the prompt-specific exception to deriving all surface colors from the application palette. Text, dividers, gray action fills, and interaction colors continue to use the application palette.

Use a centered in-app modal with a dimmed backdrop, square corners, and no outer content padding. Keep the thin palette-colored outer edge and one-physical-pixel horizontal dividers between the three sections. Padding belongs inside the header and body; it must never surround the action row. Do not place a second framework title bar, close button, or floating action panel around this layout.

## Sizing and alignment

The reference prompt uses logical UI units:

- Start at 468 units wide, expand to fit the actions' natural combined width, and constrain the result to the available application width with 16 units of space on each side.
- The header icon is 24 units square, inset 24 units from the left edge. The title stays centered in the full header independently of the icon. Header height is the greater of the title height and 24 units, plus 24 units of vertical breathing room.
- The body has 24 units of horizontal padding and 20 units of vertical padding. Wrap text within the remaining width and center every wrapped line. A short message stays centered vertically through balanced top and bottom padding.
- Action icons are 20 units square, with an 8-unit icon-to-label gap. Preserve icon aspect ratios. Use the shared `bar_height` calculation for the action row; do not introduce a separate height or extra vertical padding.
- Distribute the entire action width proportionally to each action's natural icon-and-label width. Round shared boundaries to physical pixels and give the last action the remainder, so there are no seams.

For longer messages or necessary form controls, adapt the body height and provide bounded scrolling when needed. Keep the same header, black body, centered explanatory text, and bottom action row. Preserve readable, responsive control layouts and leave all actions reachable at supported font sizes and display scales.

Prompts are fixed, unresizable surfaces: show complete localized field labels, regardless of Preferences > App style > Abbreviate field labels. Follow the [Field label and abbreviation policy](field-labels.md). Measure label widths and wrap long labels on the left of their fields; adapt the body height and scrolling instead of shortening labels or relying on a tooltip. For example, the Steam target editor must display **Steam App ID**, never **S. A. ID**.

## Buttons, icons, and behavior

Use the shared [`buttons.rs`](../crates/achimeld-ui/src/buttons.rs) implementation with `.bar()` for bottom actions. Their resting gray fill comes from the panel palette color. Preserve filled hover and keyboard-focus inversion, independently reversible animation, disabled behavior, and visible labels. Button bodies have no outlines. Icons follow the animated text color. Generate any new prompt/action artwork with the `imagegen` skill, retain its source artwork, and embed the final assets in the application.

Use direct, specific titles, messages, and action labels. The reference prompt has the title **Unsaved changes**, the centered question **Save this project before continuing?**, and the actions **Save**, **Discard changes**, and **Cancel**, in that order. Below the question, show the active project's display name in the application's bold font, then its complete saved file path in smaller, muted text. For a project without a saved file, show **Not saved yet** in place of the path. Center and wrap both lines within the body, with bounded scrolling for unusually long names or paths. Preserve the meaning and ordering of the actions needed by each prompt. Dismissal or Escape must retain the existing cancellation semantics; it must never silently choose a destructive or committing action.

Application prompts are rendered inside Achimeld with its own fonts, colors, icons, and buttons. Standard Windows file/folder selection dialogs remain the picker format. Changing prompt presentation does not authorize new filesystem access, protocol operations, worker capabilities, or changes to validation and ownership. Preserve pending-action, Save/Discard/Cancel, undo, and error-retry behavior.

Selecting **Validate** for a project without a saved file shows **Saved project required** with the message **You need to save your project before validating the Steamworks destination. Use File > Save, then select Validate again.** The single **Close** action and Escape dismiss the prompt. This error never opens a file picker, saves the project, or starts Steam validation; saving and retrying remain explicit user actions.

## Review

Check the rendered prompt at ordinary and increased display scales: black header and body, a left-aligned header icon with an independently centered title, a centered wrapped message, a gray action row with no gaps, and legible hover/focus/disabled states. Verify cancellation and the existing action paths after changing behavior. The existing native prompt screenshot fixture is `desktop::dialog_tests::native_app_prompt_screenshot`; set `ACHIMELD_SCREENSHOT_VALIDATION` to preview the saved-project error. Generated screenshots belong in the external build output tree.
