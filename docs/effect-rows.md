# Effect rows and light effects

Expanded FX rows retain a generated type icon at every supported width. Mini
values are controlled by the default-on application preference documented in
[Preferences](preferences.md). Controls are admitted as complete icon/value
pairs; the effect name appears when it fits, followed by parameter labels only
when their complete group fits. The selector remains a separate drag target so
editing a number or swatch does not reorder the effect or its owning layer.

Inner light is the existing directional inner-alpha light. Its renderer consumes
offsets, cutout, blur and independent soft-light settings; width and placement
never controlled it. Its property pane now exposes those actual settings and
newly created instances use visible white light. Existing recipes are unchanged.
Outer light dilates the coverage, softens it, removes existing coverage and
composites the remaining colored light outside the artwork. Transparent padding
avoids canvas-edge extension. Width and blur are bounded to 0–256 recipe units,
with bounded scaled radii and cancellation checks around linear mask/filter work.

The compatible recipe addition is the `OuterLight` effect kind, appended at
numeric identity 10 after LensBlur (9). Existing enum identities and field shapes
are unchanged. It reuses width, blur, color, opacity and blend mode. The managed
recipe/validator, project mapper and legacy enum normalization accept it; older
consumers may reject this new kind. Synchronization signatures already include
these fields. No worker operation, path authority or process recipe changes.

Source regressions cover light coverage, opacity, visibility, color alpha,
scaled output and cancellation; application/managed fixtures cover validation
and serialization, and UI regressions cover inline editing, undo, layout and
tree placement. These checks do not qualify an installed package or live Steam.
