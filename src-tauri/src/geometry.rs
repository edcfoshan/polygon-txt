use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct SurfaceGeometry {
    pub parts: Vec<PolygonPart>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct PolygonPart {
    pub exterior: Vec<(f64, f64)>, // (x, y)
    pub holes: Vec<Vec<(f64, f64)>>, // (x, y)
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct IndexedRing {
    pub part_index: u32,
    pub coords: Vec<(f64, f64)>, // (y, x) in TXT order
}

pub fn strip_closing_point(points: &[(f64, f64)]) -> Vec<(f64, f64)> {
    if points.len() >= 2 {
        let first = points[0];
        let last = points[points.len() - 1];
        if nearly_same_point(first, last) {
            return points[..points.len() - 1].to_vec();
        }
    }
    points.to_vec()
}

pub fn ensure_closed(points: &[(f64, f64)]) -> Vec<(f64, f64)> {
    let mut ring = strip_closing_point(points);
    if let Some(first) = ring.first().copied() {
        ring.push(first);
    }
    ring
}

pub fn signed_area(points: &[(f64, f64)]) -> f64 {
    let ring = strip_closing_point(points);
    if ring.len() < 3 {
        return 0.0;
    }
    let mut sum = 0.0;
    for i in 0..ring.len() {
        let j = (i + 1) % ring.len();
        sum += ring[i].0 * ring[j].1 - ring[j].0 * ring[i].1;
    }
    sum * 0.5
}

pub fn rotate_ring_to_northwest_start(points: &[(f64, f64)]) -> Vec<(f64, f64)> {
    let mut ring = strip_closing_point(points);
    if ring.len() < 2 {
        return ring;
    }

    let mut best_idx = 0usize;
    let mut best_y = f64::NEG_INFINITY;
    let mut best_x = f64::INFINITY;
    for (idx, &(x, y)) in ring.iter().enumerate() {
        if y > best_y || (nearly_equal(y, best_y) && x < best_x) {
            best_y = y;
            best_x = x;
            best_idx = idx;
        }
    }

    ring.rotate_left(best_idx);
    ring
}

pub fn normalize_ring_orientation(points: &[(f64, f64)], clockwise: bool) -> Vec<(f64, f64)> {
    let ring = strip_closing_point(points);
    if ring.len() < 3 {
        return ring;
    }
    let is_clockwise = signed_area(&ring) < 0.0;
    if is_clockwise == clockwise {
        ring
    } else {
        ring.into_iter().rev().collect()
    }
}

pub fn xy_to_yx(points: &[(f64, f64)]) -> Vec<(f64, f64)> {
    points.iter().map(|&(x, y)| (y, x)).collect()
}

pub fn yx_to_xy(points: &[(f64, f64)]) -> Vec<(f64, f64)> {
    points.iter().map(|&(y, x)| (x, y)).collect()
}

pub fn point_in_ring(point: (f64, f64), ring: &[(f64, f64)]) -> bool {
    let pts = strip_closing_point(ring);
    if pts.len() < 3 {
        return false;
    }
    let (px, py) = point;
    let mut inside = false;
    let mut j = pts.len() - 1;
    for i in 0..pts.len() {
        let (xi, yi) = pts[i];
        let (xj, yj) = pts[j];
        let intersects = ((yi > py) != (yj > py))
            && (px < (xj - xi) * (py - yi) / ((yj - yi).abs().max(1e-12) * (if yj >= yi { 1.0 } else { -1.0 })) + xi);
        if intersects {
            inside = !inside;
        }
        j = i;
    }
    inside
}

pub fn build_surface_from_rings(rings: &[Vec<(f64, f64)>]) -> SurfaceGeometry {
    let clean: Vec<Vec<(f64, f64)>> = rings
        .iter()
        .map(|ring| strip_closing_point(ring))
        .filter(|ring| ring.len() >= 3)
        .collect();

    if clean.is_empty() {
        return SurfaceGeometry::default();
    }

    let areas: Vec<f64> = clean.iter().map(|ring| signed_area(ring).abs()).collect();
    let mut containers: Vec<Option<usize>> = vec![None; clean.len()];

    for i in 0..clean.len() {
        let probe = clean[i][0];
        let mut best: Option<usize> = None;
        let mut best_area = f64::INFINITY;
        for j in 0..clean.len() {
            if i == j || areas[j] <= areas[i] {
                continue;
            }
            if point_in_ring(probe, &clean[j]) && areas[j] < best_area {
                best = Some(j);
                best_area = areas[j];
            }
        }
        containers[i] = best;
    }

    let mut depths = vec![0usize; clean.len()];
    for i in 0..clean.len() {
        let mut depth = 0usize;
        let mut cursor = containers[i];
        while let Some(parent) = cursor {
            depth += 1;
            cursor = containers[parent];
        }
        depths[i] = depth;
    }

    let mut parts = Vec::new();
    for i in 0..clean.len() {
        if depths[i] % 2 != 0 {
            continue;
        }
        let holes = (0..clean.len())
            .filter(|&j| containers[j] == Some(i) && depths[j] == depths[i] + 1)
            .map(|j| clean[j].clone())
            .collect();
        parts.push(PolygonPart {
            exterior: clean[i].clone(),
            holes,
        });
    }

    SurfaceGeometry { parts }
}

pub fn surface_to_indexed_rings(
    surface: &SurfaceGeometry,
    reorder_northwest: bool,
    close_rings: bool,
) -> Vec<IndexedRing> {
    let mut rings = Vec::new();
    let mut part_index = 1u32;

    for part in &surface.parts {
        let mut exterior = normalize_ring_orientation(&part.exterior, true);
        if reorder_northwest {
            exterior = rotate_ring_to_northwest_start(&exterior);
        }
        if close_rings {
            exterior = ensure_closed(&exterior);
        }
        rings.push(IndexedRing {
            part_index,
            coords: xy_to_yx(&exterior),
        });
        part_index += 1;

        for hole in &part.holes {
            let mut ring = normalize_ring_orientation(hole, false);
            if reorder_northwest {
                ring = rotate_ring_to_northwest_start(&ring);
            }
            if close_rings {
                ring = ensure_closed(&ring);
            }
            rings.push(IndexedRing {
                part_index,
                coords: xy_to_yx(&ring),
            });
            part_index += 1;
        }
    }

    rings
}

pub fn indexed_rings_to_surface(rings: &[IndexedRing]) -> SurfaceGeometry {
    let xy_rings: Vec<Vec<(f64, f64)>> = rings
        .iter()
        .map(|ring| yx_to_xy(&ring.coords))
        .collect();
    build_surface_from_rings(&xy_rings)
}

fn nearly_same_point(a: (f64, f64), b: (f64, f64)) -> bool {
    nearly_equal(a.0, b.0) && nearly_equal(a.1, b.1)
}

fn nearly_equal(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-9
}
