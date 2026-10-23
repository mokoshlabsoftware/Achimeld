//! Closed parametric meshes, including angular caps and hollow sphere rims.
use super::*;
use achimeld_application::artifact3d::Primitive;
use std::f64::consts::{PI, TAU};

fn quad(mesh: &mut Vec<[Vertex; 3]>, v: [Vertex; 4]) {
    mesh.push([v[0], v[1], v[2]]);
    mesh.push([v[0], v[2], v[3]]);
}

pub(super) fn mesh(part: &ArtifactPart, segments: usize) -> Vec<[Vertex; 3]> {
    match part.primitive {
        Primitive::Cube => cube(part.rounding, segments / 4),
        Primitive::Torus => torus(part, segments),
        _ => lathe(part, segments),
    }
}

fn cube(rounding: f64, segments: usize) -> Vec<[Vertex; 3]> {
    let mut grid = if rounding == 0.0 {
        vec![-1.0, 1.0]
    } else {
        (0..=segments)
            .map(|i| 2.0 * i as f64 / segments as f64 - 1.0)
            .collect::<Vec<_>>()
    };
    if rounding > 0.0 {
        grid.extend([-1.0 + rounding, 1.0 - rounding]);
    }
    grid.sort_by(f64::total_cmp);
    grid.dedup_by(|a, b| (*a - *b).abs() < 1e-10);
    let steps = grid.len() - 1;
    let mut mesh = Vec::new();
    for axis in 0..3 {
        for side in [-1.0, 1.0] {
            let vertex = |a: usize, b: usize| {
                let mut p = [0.0; 3];
                p[axis] = side;
                p[(axis + 1) % 3] = grid[a];
                p[(axis + 2) % 3] = grid[b];
                // Round only the edge region, keeping broad planar faces at small
                // values. At one the inner box vanishes into a sphere.
                let inner = p.map(|v| v.clamp(-1.0 + rounding, 1.0 - rounding));
                let delta = sub(p, inner);
                let l1: f64 = delta.iter().map(|v| v.abs()).sum();
                let chamfer = mul(delta, 1.0 / l1.max(1e-12));
                let normal = unit(delta);
                let offset = add(mul(chamfer, 1.0 - rounding), mul(normal, rounding));
                let mut flat = [0.0; 3];
                flat[axis] = side;
                Vertex {
                    p: add(inner, mul(offset, rounding)),
                    n: if rounding == 0.0 { flat } else { normal },
                }
            };
            for a in 0..steps {
                for b in 0..steps {
                    quad(
                        &mut mesh,
                        [
                            vertex(a, b),
                            vertex(a + 1, b),
                            vertex(a + 1, b + 1),
                            vertex(a, b + 1),
                        ],
                    );
                }
            }
        }
    }
    mesh
}

#[derive(Clone, Copy)]
struct Profile {
    r: f64,
    y: f64,
    nr: f64,
    ny: f64,
}
fn profile(r: f64, y: f64, nr: f64, ny: f64) -> Profile {
    Profile { r, y, nr, ny }
}

fn lathe(part: &ArtifactPart, segments: usize) -> Vec<[Vertex; 3]> {
    let mut path = Vec::new();
    match part.primitive {
        Primitive::Cone => {
            path.extend([
                profile(0.0, 1.0, 2.0, 1.0),
                profile(1.0, -1.0, 2.0, 1.0),
                profile(1.0, -1.0, 0.0, -1.0),
                profile(0.0, -1.0, 0.0, -1.0),
            ]);
        }
        Primitive::Cylinder => {
            let r = part.rounding;
            path.push(profile(0.0, 1.0, 0.0, 1.0));
            path.push(profile(1.0 - r, 1.0, 0.0, 1.0));
            if r > 0.0 {
                for i in 0..=segments / 4 {
                    let t = PI / 2.0 * (1.0 - i as f64 / (segments / 4) as f64);
                    let (s, c) = t.sin_cos();
                    let blend = |v: f64| v / (s + c) * (1.0 - r) + v * r;
                    path.push(profile(
                        1.0 - r + r * blend(c),
                        1.0 - r + r * blend(s),
                        c,
                        s,
                    ));
                }
            } else {
                path.push(profile(1.0, 1.0, 1.0, 0.0));
            }
            path.push(profile(1.0, -1.0 + r, 1.0, 0.0));
            if r > 0.0 {
                for i in 0..=segments / 4 {
                    let t = PI / 2.0 * i as f64 / (segments / 4) as f64;
                    let (s, c) = t.sin_cos();
                    let blend = |v: f64| v / (s + c) * (1.0 - r) + v * r;
                    path.push(profile(
                        1.0 - r + r * blend(c),
                        -1.0 + r - r * blend(s),
                        c,
                        -s,
                    ));
                }
            }
            path.push(profile(1.0 - r, -1.0, 0.0, -1.0));
            path.push(profile(0.0, -1.0, 0.0, -1.0));
        }
        Primitive::Sphere => {
            let (lo, hi) = (part.cut_bottom, part.cut_top);
            sphere_profile(&mut path, 1.0, hi, lo, segments / 2, 1.0);
            let inner = 1.0 - part.wall_thickness;
            let hollow = part.wall_thickness > 0.0 && lo < inner && hi > -inner;
            if hollow {
                let (ilo, ihi) = (lo.max(-inner), hi.min(inner));
                // Cut planes get flat annular caps. Internal poles connect
                // naturally without filling the bowl's open interior.
                if lo > -inner {
                    cap(&mut path, lo, 1.0, inner, -1.0);
                } else {
                    cap(&mut path, lo, 1.0, 0.0, -1.0);
                }
                sphere_profile(&mut path, inner, ilo, ihi, segments / 2, -1.0);
                if hi < inner {
                    cap(&mut path, hi, inner, 1.0, 1.0);
                } else {
                    path.push(profile(0.0, hi, 0.0, 1.0));
                    cap(&mut path, hi, 0.0, 1.0, 1.0);
                }
            } else {
                cap(&mut path, lo, 1.0, 0.0, -1.0);
                path.push(profile(0.0, hi, 0.0, 1.0));
                cap(&mut path, hi, 0.0, 1.0, 1.0);
            }
            // Close the profile even at a clipped polar endpoint.
            if let Some(first) = path.first().copied() {
                path.push(first);
            }
        }
        _ => unreachable!(),
    }
    let steps = ((segments as f64 * part.sweep_angle / 360.0).ceil() as usize).max(2);
    let angle =
        |i: usize| (part.start_angle + part.sweep_angle * i as f64 / steps as f64).to_radians();
    let vertex = |p: Profile, a: f64| {
        let (s, c) = a.sin_cos();
        Vertex {
            p: [p.r * c, p.y, p.r * s],
            n: unit([p.nr * c, p.ny, p.nr * s]),
        }
    };
    let mut mesh = Vec::new();
    for pair in path.windows(2) {
        if (pair[0].r - pair[1].r).abs() + (pair[0].y - pair[1].y).abs() < 1e-10 {
            continue;
        }
        for i in 0..steps {
            quad(
                &mut mesh,
                [
                    vertex(pair[0], angle(i)),
                    vertex(pair[1], angle(i)),
                    vertex(pair[1], angle(i + 1)),
                    vertex(pair[0], angle(i + 1)),
                ],
            );
        }
    }
    if part.sweep_angle < 360.0 {
        // Ear clipping handles concave hollow-sphere cross sections without
        // filling their cavities. Budget is fixed by the profile resolution.
        for (a, sign) in [(angle(0), 1.0), (angle(steps), -1.0)] {
            let (s, c) = a.sin_cos();
            let n = [s * sign, 0.0, -c * sign];
            for triangle in cap_triangles(&path) {
                mesh.push(triangle.map(|i| Vertex {
                    p: [path[i].r * c, path[i].y, path[i].r * s],
                    n,
                }));
            }
        }
    }
    mesh
}

fn sphere_profile(
    path: &mut Vec<Profile>,
    radius: f64,
    from: f64,
    to: f64,
    steps: usize,
    normal: f64,
) {
    let (a, b) = (
        (from / radius).clamp(-1.0, 1.0).asin(),
        (to / radius).clamp(-1.0, 1.0).asin(),
    );
    for i in 0..=steps {
        let (s, c) = (a + (b - a) * i as f64 / steps as f64).sin_cos();
        path.push(profile(radius * c, radius * s, normal * c, normal * s));
    }
}
fn cap(path: &mut Vec<Profile>, y: f64, from: f64, to: f64, normal: f64) {
    let radius = |r: f64| (r * r - y * y).max(0.0).sqrt();
    path.push(profile(radius(from), y, 0.0, normal));
    path.push(profile(radius(to), y, 0.0, normal));
}

fn cap_triangles(path: &[Profile]) -> Vec<[usize; 3]> {
    let mut ids: Vec<usize> = Vec::new();
    for (i, p) in path.iter().enumerate() {
        if ids
            .last()
            .is_none_or(|last| (path[*last].r - p.r).abs() + (path[*last].y - p.y).abs() > 1e-9)
        {
            ids.push(i);
        }
    }
    if ids.len() > 1 {
        let (a, b) = (path[ids[0]], path[*ids.last().unwrap()]);
        if (a.r - b.r).abs() + (a.y - b.y).abs() < 1e-9 {
            ids.pop();
        }
    }
    let edge = |a: usize, b: usize, c: usize| {
        (path[b].r - path[a].r) * (path[c].y - path[a].y)
            - (path[b].y - path[a].y) * (path[c].r - path[a].r)
    };
    let area: f64 = ids
        .iter()
        .zip(ids.iter().cycle().skip(1))
        .take(ids.len())
        .map(|(a, b)| path[*a].r * path[*b].y - path[*b].r * path[*a].y)
        .sum();
    let sign = area.signum();
    let mut out = Vec::new();
    while ids.len() >= 3 {
        let ear = (0..ids.len()).find(|i| {
            let (a, b, c) = (
                ids[(*i + ids.len() - 1) % ids.len()],
                ids[*i],
                ids[(*i + 1) % ids.len()],
            );
            edge(a, b, c) * sign > 1e-10
                && !ids.iter().any(|p| {
                    ![a, b, c].contains(p)
                        && edge(a, b, *p) * sign >= -1e-10
                        && edge(b, c, *p) * sign >= -1e-10
                        && edge(c, a, *p) * sign >= -1e-10
                })
        });
        let Some(i) = ear else {
            break;
        };
        out.push([
            ids[(i + ids.len() - 1) % ids.len()],
            ids[i],
            ids[(i + 1) % ids.len()],
        ]);
        ids.remove(i);
    }
    out
}

fn torus(part: &ArtifactPart, segments: usize) -> Vec<[Vertex; 3]> {
    let steps = ((segments as f64 * part.sweep_angle / 360.0).ceil() as usize).max(2);
    let minor = (segments / 2).max(12);
    let start = part.start_angle.to_radians();
    let sweep = part.sweep_angle.to_radians();
    let vertex = |u: f64, v: f64| {
        let (su, cu) = u.sin_cos();
        let (sv, cv) = v.sin_cos();
        let radius = 1.0 - part.tube_radius + part.tube_radius * cv;
        Vertex {
            p: [radius * cu, sv, radius * su],
            n: unit([cv * cu, sv * part.tube_radius, cv * su]),
        }
    };
    let mut mesh = Vec::new();
    for i in 0..steps {
        for j in 0..minor {
            let (a, b) = (
                start + sweep * i as f64 / steps as f64,
                start + sweep * (i + 1) as f64 / steps as f64,
            );
            let (c, d) = (
                TAU * j as f64 / minor as f64,
                TAU * (j + 1) as f64 / minor as f64,
            );
            quad(
                &mut mesh,
                [vertex(a, c), vertex(b, c), vertex(b, d), vertex(a, d)],
            );
        }
    }
    if part.sweep_angle < 360.0 {
        for (a, sign) in [(start, 1.0), (start + sweep, -1.0)] {
            let (s, c) = a.sin_cos();
            let n = [s * sign, 0.0, -c * sign];
            let center = Vertex {
                p: [
                    (1.0 - part.tube_radius) * c,
                    0.0,
                    (1.0 - part.tube_radius) * s,
                ],
                n,
            };
            for j in 0..minor {
                let mut b = vertex(a, TAU * j as f64 / minor as f64);
                b.n = n;
                let mut c = vertex(a, TAU * (j + 1) as f64 / minor as f64);
                c.n = n;
                mesh.push([center, b, c]);
            }
        }
    }
    mesh
}
