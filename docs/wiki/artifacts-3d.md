# 3D artifact layers

3D artifact creation is currently hidden from the layer picker, and no default 3D object style is bundled. Existing 3D artifact layers remain editable. Artifact preset > Replace with preset replaces their parts with Person, Cup, Shield, Sword or Potion. Undo restores the previous parts.

Presets remain editable assemblies.

## Parts and transforms

Add, select, rename, hide, duplicate, reorder and remove up to 64 parts. Each part has independent X/Y/Z size, position and rotation, material color, Matte/Glossy/Unlit shading and glossy roughness.

The artifact has its own position, three-axis angle, camera and lighting. Part-local transforms apply before the artifact transform.

X points right, Y up and Z toward the camera. Dimensions use the 512-unit icon design space. Perspective and orthographic cameras follow the [3D text](text-3d.md) convention.

## Primitive controls

| Primitive | Shape controls |
| --- | --- |
| Cube | Rounding 0-1, from sharp faces toward an ellipsoid |
| Cone | Angular start and sweep |
| Torus | Tube radius, angular start and sweep |
| Cylinder / capsule | Rounding 0-1, from sharp cylinder through capsule forms toward an ellipsoid |
| Sphere | Bottom/top cuts -1 to 1, angular start/sweep and optional wall thickness |

Angular sweeps run 0.5-360 degrees and close their end faces. A torus lies in local X/Z with thickness along Y; rotating X by 90 degrees gives a front-facing handle.

Sphere cuts occur before size and rotation. A cut at zero produces a hemisphere. Positive wall thickness makes a hollow shell with a closed rim; zero thickness is solid. Size controls final outer bounds, including rounded ends.

## Preset examples

The cup uses a hollow hemisphere bowl, flattened hemisphere base, stem and partial torus handles. The shield combines a face, boss and braces. The sword combines blade, tip, guard, grip and pommel. The potion combines bottle parts, liquid body, neck, rim and stopper; its material does not simulate glass refraction.

Use these structures as starting points for your own silhouette.

## Output

Full color produces shaded RGB with transparent coverage. Alpha mask produces white artwork retaining the material alpha and silhouette. Both use visible surfaces, cut caps and internal walls.

Ordinary layer opacity, masks, effects, style copying and export still apply. Reordering parts is not a substitute for changing their depth: the 3D view uses depth-tested surfaces.

If a part disappears, check its visibility, alpha, transforms, cuts and camera position before changing the surrounding icon recipe.

