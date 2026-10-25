# Numeric slider and knob standard

The reference is the **Particle count** control in layer properties: a label on
the left, a thin horizontal rail, a small solid round handle, and an editable
numeric field on the right. This is the default throughout Achimeld, including
tools, previews, preferences and layer properties. “Knob” here means that small
slider handle, not a rotary dial.

## Appearance and layout

- Use the shared implementation in [sliders.rs](../crates/achimeld-ui/src/sliders.rs).
  `value_slider` supplies the labeled row and numeric field; `range_track` is
  the lower-level track for specialized layouts; `discrete_slider` supports
  ordered choices such as 64, 128, 256 and 512 pixel resolutions.
- The track has a 12.5-point interaction height, a 4-point rail and a solid
  5-point-radius handle. These are logical points, scaled by display density;
  larger fonts must not turn the handle into a large outlined circle.
- Derive the rail, handle, disabled appearance and focus indicator from the
  application palette. Keep the handle solid and the same color on both ends
  of the rail. Keyboard focus uses a visible rail indicator.
- Place the numeric field to the right, with its units and appropriate precision.
  Follow [Field labels](field-labels.md): the label remains to the left.
  At narrow widths, the track and number may stack inside the control column;
  the label must not move above that column or overlap the controls.
- Keep geometry stable on hover, focus and drag. Use existing field styling for
  the numeric editor; this is not a new button style.

## Interaction and data

Retain pointer dragging, track clicks, keyboard adjustment, direct numeric entry,
disabled behavior, bounds, units and precision. Discrete tracks show the actual
value in the numeric editor, not an internal index. Numeric entry selects the
nearest supported choice. Merely showing a control must not modify saved data.
Retain the caller's undo grouping, validation and rendering cancellation.

Do not add raw `egui::Slider` controls outside the shared implementation.
A new slider should not duplicate the rail or handle painter.

## Specialized controls

The recessed **Opacity** control in layer rows and its compact popup remains
supported through [opacity.rs](../crates/achimeld-ui/src/opacity.rs). It uses the
same rail and handle inside its labeled recessed container. That container is
an opacity-only exception, not the default for arbitrary numeric settings.
The ordinary Opacity property row uses the standard label/track/number layout.

Particle minimum/maximum controls retain two small solid handles and a draggable
interval in [particle_sliders.rs](../crates/achimeld-ui/src/particle_sliders.rs).
Their independent endpoints and interval movement are necessary semantics.
Gradient stops, curve points, color-selection surfaces, canvas gizmos, panel
dividers and scrollbars are not scalar numeric knobs and retain their specific
interactions. Standalone numeric entry fields do not all need to become sliders.

## Application-wide audit — 2026-10-25

Reviewed application source in crates, bridge, build, installer and tools;
third-party framework internals are not application control call sites.
All application-owned raw scalar slider call sites were in the Rust UI.

| Surface | Result |
| --- | --- |
| Particle and explosion properties | Already standard; now call the dedicated shared slider module. |
| Layer opacity, effects, grayscale/alpha picker, gradient-map track and Preferences field preview | Already standard; share the same module. |
| Adjustment layers (brightness, contrast, hue, saturation, selective color, unsharp controls) | Replaced framework slider with standard control. |
| 3D text and 3D artifact material/shape sliders | Replaced framework sliders with standard control; retained text hints. |
| Style Viewer banner height, resolution and display size | Replaced all three framework sliders; resolution retains its four supported sizes. |
| Layer-row opacity and particle intervals | Retained the specialized variants described above. |
| Bridge, build, installer and tools | No additional scalar slider UI found. |

The only application `egui::Slider::new` constructor belongs in `sliders.rs`.
Check ordinary and enlarged fonts, narrow fields, display scaling, keyboard focus,
disabled controls and numeric entry when changing this implementation.
