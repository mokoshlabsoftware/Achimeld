# 3D artifact layers

Add **3D artifact** in the layer picker. It starts with an editable person. **Artifact preset > Replace with preset…** replaces its parts with Person, Cup, Shield, Sword or Potion. Undo restores the previous collection. Presets contain ordinary parts, with no baked artwork or hidden preset geometry.

The parts list supports adding, selecting, renaming, hiding, duplicating, reordering and removing up to 64 parts. Every part has independent X/Y/Z size, position and rotation, material color, Matte/Glossy/Unlit shading and glossy roughness. The artifact has its own position and three-axis angle, perspective/orthographic camera and lighting, following the 3D text camera convention. X points right, Y up and Z toward the camera. Dimensions use the 512-unit icon design space.

Primitives:

- **Cube**: rounding 0–1, from sharp faces through chamfered edges to an ellipsoid.
- **Cone**: a solid cone with an angular start and sweep.
- **Torus**: tube radius, angular start and sweep; 180° makes a half ring. Its ring lies in local X/Z with thickness along Y. Rotate X by 90° for a front-facing handle.
- **Cylinder / capsule**: rounding 0–1, from a sharp cylinder through chamfered edges and capsule forms to an ellipsoid. Size controls the final outer bounds, including rounded ends.
- **Sphere**: bottom/top cuts from −1 to 1 along local Y, angular start/sweep and optional wall thickness. A cut at zero gives a hemisphere; positive wall thickness makes a hollow shell with a closed rim. Zero thickness is solid.

Angular sweeps are 0.5–360°, with closed end faces. Sphere cuts happen before the part's size and rotation. Parts have their own local transforms, followed by the artifact transform. Preset dimensions remain independently editable after insertion.

The person has a head, torso, two hands and two feet. The cup has a hollow lower hemisphere as its bowl, a flattened upper hemisphere as its base, a cylinder stem and two partial torus handles. The shield has a face, boss and two braces; the sword has a blade, tip, guard, grip and pommel; the potion has a bottle, potion body, neck, rim and stopper. These are stylized primitive assemblies; the potion material does not simulate glass refraction.

**FullColor** produces shaded RGB with transparent coverage; **AlphaMask** produces white artwork with the same material alpha and silhouette. Both use depth-tested surfaces, including cut caps and internal bowl walls. Existing layer opacity, masks, transforms, effects, export, style copying and undo apply normally.

The optional schema-1 `artifact3d` field and appended `Artifact3d` layer kind are preserved by both Rust project storage and the managed mapper. Managed validation and synchronization fingerprints include every part and camera field. Existing enum values and worker command shapes are retained. The renderer performs no file I/O and uses the host's existing bounded quick/detail preview queues, cancellation and texture filtering. Mesh resolution follows preview size; a raster work budget bounds extreme overlaps/camera crossings.

Validation covers presets, malformed settings and bounds, rendering/occlusion/ranges/cuts/rounding, alpha masks, camera/material changes, cancellation, normal layer composition, project save/reload/undo and responsive inspector layout. The ignored `artifact3d_contact_sheet` fixture writes `artifact3d-preview.png` into the external current build directory and records first-preview and detail rendering times. Source/fixture results do not establish installed Windows enforcement or live Steam qualification.
