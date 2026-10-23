# Explosion layer source audit — 2026-10-17

Scope: Explosion recipe validation and deterministic layout, deltoid and legacy triangle SDFs and polar projection, the shared gradient helper and sliders, inspector controls, project/history persistence, managed mapping/validation/fingerprints, and progressive style previews. The source is the working tree; the exact commit and per-file hashes belong to the external `Audit.ps1 -Offline -Fixtures` evidence report, rather than an assumed clean commit. Other work in this checkout can invalidate that report's source-stability check.

## Confirmed findings and fixes

| Boundary / invariant | Pre-fix path and consequence | Fix and executable evidence |
| --- | --- | --- |
| Render correctness / SEC-RENDER-001 | Ordinary recipes with overlapping inward/outward halves added both coverages, strengthening soft edges. | [SDF compositor](../../crates/achimeld-imaging/src/explosion.rs) sums coverage only for opposite-facing halves sharing a base. Overlaps otherwise use a union. `explosion_overlapping_halves_do_not_add_opacity` compares coincident and offset overlaps, including transparency. |
| Render correctness | Flip originally pivoted around the wide base, moving the triangle into the other half's space. | Shared placement now swaps base and tip within the existing bounds. `explosion_flip_swaps_wide_and_pointed_ends_within_existing_bounds` checks both families, offsets and heights; `explosion_flips_reflect_in_place_for_both_triangle_sets` checks reflected SDF coverage with random and global scaling. The construction diagram uses the same placement. |
| Bounded rendering / SEC-RENDER-001 | Direct internal calls to the estimate helper could overflow unchecked dimension/work multiplication. Normal host dimension validation already limits this entry path; an external project-to-overflow exploit is not established. | Checked multiplication rejects overflow and empty dimensions before allocation. `explosion_rejects_overflowing_work_and_cancels_during_gradient_generation` exercises maximum `u32` dimensions and zero width. |
| Cancellation / SEC-RENDER-001 | Cancellation after gradient generation began was observed only after the complete gradient bitmap had been filled. | The shared gradient helper now checks before allocation and at each row. The same regression requests cancellation during gradient generation; the existing composition and progressive preview checks still apply. |
| Render correctness | Large triangles could meet the outer polar clip with a hard, aliased circumference. | Pixel coverage antialiases the outer circle. `explosion_scaled_triangles_antialias_the_outer_polar_boundary` checks partial edge coverage and transparent pixels beyond the clip. |
| Inspector responsiveness | The long construction header widened narrow inspectors. Numeric-width estimates omitted fractional digits, and range controls could put their value after a full-width track inside a horizontal property row. | Compact construction/action labels, measured range labels, explicit stacked fallbacks, and fractional numeric-width allowance. `explosion_deltoid_shapes_fit_narrow_inspectors_without_dirtying_recipes` checks the Shapes foldout at narrow/wide widths and unchanged recipe data. |

## Added recipe controls and compatibility

New layers use optional `explosion.deltoid`: one shared width, independent `topHeightMin/Max` and `bottomHeightMin/Max`, a shared offset and sharpness, and common global/random scale. Heights are bounded to 0–100% of strip height, width to 0–200%, deltoid offset to ±150%, and sharpness to 0–100%. The single shape samples one uniform scale factor for its width and both heights. Its SDF measures distance to the four perimeter edges, with no internal waist edge. A zero vertex height yields a triangle; zero width or both heights zero yield empty geometry. Angular bucket count, object-count bounds, cancellation and checked work accounting apply to both render paths.

`explosion_deltoids_share_width_scale_and_waist_with_independent_vertex_heights` checks determinism, shared scale, symmetry, spacing, bounds validation and sparse legacy compatibility. `explosion_deltoid_sdf_has_one_symmetric_perimeter_and_independent_tips` checks symmetry, waist continuity, independent height behavior and degenerate geometry. Managed fixtures cover deltoid round trips, validation and fingerprints for every property. History/export, disk persistence and progressive preview regressions exercise deltoids. The native inspector and gallery helpers provide visual evidence outside source.

Older recipes without `deltoid` retain their previous layout, renderer and managed fingerprint. The Shape foldout offers an explicit **Use deltoid** action; merely opening it is read-only, covered by `explosion_legacy_shape_conversion_requires_an_explicit_edit`. Legacy fields remain accepted, validated and retained without influencing deltoid geometry. New Rust and managed layer constructors create deltoids; layer IDs, operations and authority boundaries are unchanged.

Legacy triangle controls remain part of the compatible storage model:

Each `explosion.out` / `explosion.in` record adds `flip`, `scale`, `scaleMin` and `scaleMax`. Flip swaps the wide and pointed ends within the triangle's existing bounds, preserving its center, size and offset. Scale multiplies width and height uniformly, anchored at the unflipped base. Each generated triangle independently samples its range, multiplied by that set's global scale. Both scale controls are bounded to 1–200%, so the resulting multiplier is bounded to 0.01–400%. Object count is now 1–512 (see the 2026-10-18 extension below).

Missing new fields mean no flip and 100% scaling. Scale draws follow the original geometry/position draws, preserving existing seeded layouts. Default new controls preserve managed triangle fingerprints. Non-default controls participate in fingerprints, project saves, undo/redo, preview/export, and the construction diagram. Layout goldens, invalid/reversed ranges, independent samples, bounds-preserving flips, managed fixtures and project round trips cover these contracts. Existing worker operations, layer enum identities, filesystem ownership and process recipes remain unchanged.

## Shape and blur extension — 2026-10-18

The exact and ranged object counts accept 1–512. Fixed `shapeX` and `shapeY` each accept 1–10 and multiply sampled dimensions without consuming new random draws. Fixed `deltoid.ellipse` accepts 0–1 and continuously changes the kite into an ellipse while retaining its top and bottom bounds. The zero morph retains the original exact four-edge distance calculation; rounded profiles use a gradient-corrected superellipse distance approximation. Neutral defaults preserve old geometry and managed fingerprints. `explosion_shape_axes_preserve_random_samples_and_maximum_count_round_trips` covers random-sequence compatibility, 512-object storage and invalid bounds; `explosion_morph_outline_matches_distance_field_and_ellipse_bounds` compares the shared construction outline and render field, including asymmetric and flat-ended shapes.

Fixed `radialBlur` and `angularBlur` each accept 0–1. When enabled, the pre-polar construction texture is capped at 262,144 texels, with dimensions capped at 2048×512, before three separable box passes. Horizontal sampling wraps; vertical out-of-bounds samples are transparent. Colors accumulate premultiplied by alpha. Zero blur keeps the direct inverse-polar path. Texture generation, blur and output check cancellation at row or line boundaries. Checked work estimation charges candidate complexity, texture passes and output against the existing 128,000,000-unit layer cap before allocation; rounded candidates cost more than straight edges. `explosion_512_rounded_objects_render_and_keep_work_and_cancellation_limits` and `explosion_construction_blur_has_independent_axes_seam_and_zero_identity` exercise dense geometry, caps, cancellation, independent directions, seam continuity and identity.

Rust and managed validation, project/history round trips and self-contained preset records retain all new controls. Managed fixtures reject nonfinite/out-of-range values and preserve neutral fingerprints. Native inspector regressions cover narrow widths without editing sparse recipes. The separate [Radial blur adjustment](../radial-blur.md) appends layer ID 21, charges its sampling to the shared composition budget and uses existing adjustment mask/dependency entry points. No storage, worker protocol, browser or process-launch authority is added.

## Bottom widening extension — 2026-10-18

`bottomPower` accepts finite 0–9 extra width, `bottomStretch` accepts finite 0–1 construction height, and `bottomCurve` accepts only Linear, Quadratic, or Logarithmic. Rust recipe parsing and managed receiving validation enforce the same bounds. Missing fields retain zero power, 50% stretch, and Linear. No random draws are added. The inverse sampler divides each object's periodic horizontal displacement by a bounded 1–10 multiplier; vertical coordinates and the unaffected upper strip retain their original samples. This operates before blur in both construction and polar outputs, including legacy triangles. SDF colors retain the original normalized shape coordinate field under deformation, rather than claiming exact Euclidean distance to the widened boundary.

Angular culling includes each shape's maximum widened width and feather, and work accounting charges the expanded candidate set plus curve evaluation before allocation. Existing pixel, object, intermediate-texture, composition, and cancellation limits remain in force. Focused regressions cover neutral pixel identity with/without blur, increased central coverage, upper-region identity, all three curves, extreme stretch, seed stability, periodic seams, culling versus unculled sampling, and cancellation during active widening. Project/history/preview/export and preset collection fixtures exercise non-neutral controls; managed fixtures cover validation, fingerprints, and JSON round trips. Inspector regressions cover the dropdown, disabled input, sparse read-only behavior, and narrow layouts. Generated curve artwork and exact prompts are retained with the source assets. This adds recipe parameters only; storage ownership, process recipes, worker operation identities, and launch paths are unchanged.

## Limits and exclusions

The gradient selector also accepts SDF, using the existing per-shape signed
distance normalized by its inradius, independently of the opacity/softness mask.
Its stops use the shared renderer's color and alpha interpolation. The SDF mode
keeps row cancellation and existing work limits; spatial Linear/Radial behavior
is unchanged. Managed SDF enum identity is appended without renumbering older
kinds, and is only accepted for explosion gradients.

New layer constructors enable `coverCanvas` and use a 100% overall scale. The
radius is the output canvas half-diagonal, enclosing square and rectangular icon
corners. Missing flags preserve the older shorter-half-edge radius and scale;
both managed fingerprints and Rust/managed round trips include the new flag.
Distance/scale/softness, gradient alpha/angle behavior, default polar sizing,
project/history/export parity and managed validation have focused regressions.
See [procedural libraries](procedural-effect-libraries.md) for embedded colors
and collection I/O.

This is local source and synthetic fixture evidence, not independent attestation or proof that every rendering defect is absent. Angular buckets, count/range validation, per-layer work accounting and the shared composition budget remain bounded; absolute CPU/RSS and full primitive preemption are unqualified. Installed Windows enforcement, live Steam qualification and dependency-advisory freshness are separate checks with their own prerequisites. Any skipped checks or source changes during execution remain explicit in the audit report.
