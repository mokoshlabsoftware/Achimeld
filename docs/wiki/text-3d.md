# 3D text layers

Choose 3D text in the layer picker. Enter text or link it to the achievement unlock value, then choose its system font, size and front color.

The layer renders with transparency and keeps ordinary opacity, blending, 2D transforms, masks and effects.

## Geometry

| Control | Meaning |
| --- | --- |
| Extrusion | Depth extending equally forward and backward from the text plane |
| Push | -64 to 64 design units; negative thins strokes, positive thickens them |
| Emboss | Beveled rim up to 64 units wide/high on both extrusion ends |
| Smooth normals | Softens shading on curved walls/rims without changing silhouette |
| Object X/Y/Z position and angle | Places and rotates the 3D object before the ordinary layer transform |

Font size, extrusion, camera distance, view height and object position use the 512-unit design space, independently of export dimensions. The text limit is 256 characters and font size up to 512.

Strong negative Push can remove thin strokes. Positive Push can close letter holes. Emboss adds to width and depth rather than shrinking the flat face.

## Camera

The camera looks down negative Z. X points right, Y up and positive Z toward the camera. Object rotations apply X, then Y, then Z before translation.

Perspective uses camera distance and focal length. Increasing focal length magnifies; increasing distance makes the object appear smaller.

Orthographic uses view height for visible extent; distance or object Z does not produce perspective size changes. Both projections clip geometry too close to the camera.

## Material and light

Choose Matte, Glossy or Unlit. Matte/Glossy use directional and ambient light. Glossy adds highlights controlled by Roughness and Reflectivity (0-1; default 0.55). Unlit shows the material colors directly.

Front color supplies faces and emboss; Side color supplies straight extrusion walls. Letter shadows controls self-shadow strength from 0 to 1. It does not create a ground plane. Unlit and mask outputs ignore lighting/shadows.

## Output

| Output | Visible content |
| --- | --- |
| Full color | Shaded colored text |
| Full mask | White coverage of the visible complete object |
| Face mask | Visible flat letter faces |
| Emboss mask | Visible beveled rim; empty at zero Emboss |
| Emboss mask - face | Complete object minus flat faces, retaining rim and visible extrusion walls |

Mask outputs ignore material colors/alpha but retain geometry occlusion. Hidden faces do not show through. Ordinary layer opacity, masks, blending and effects still apply.

At zero Emboss, Emboss mask - face can still contain visible extrusion walls. This differs from an isolated rim mask.

Inspect thin letters and holes at export size, and check camera/position before assuming an empty output is a font failure.

