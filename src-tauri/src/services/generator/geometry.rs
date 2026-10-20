//! Small geometry helpers: line simplification, polygon area, point tests.

use crate::models::map::Point;

/// Perpendicular distance from `p` to the segment `a`–`b`.
fn segment_distance(p: Point, a: Point, b: Point) -> f64 {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let len2 = dx * dx + dy * dy;
    if len2 == 0.0 {
        return ((p.x - a.x).powi(2) + (p.y - a.y).powi(2)).sqrt();
    }
    let t = (((p.x - a.x) * dx + (p.y - a.y) * dy) / len2).clamp(0.0, 1.0);
    let (cx, cy) = (a.x + t * dx, a.y + t * dy);
    ((p.x - cx).powi(2) + (p.y - cy).powi(2)).sqrt()
}

/// Douglas–Peucker simplification of an open polyline: drops points that
/// are closer than `tolerance` to the line through their neighbours. The
/// first and last points are always kept.
///
/// Written with an explicit stack instead of recursion so a 100k-point
/// coastline cannot overflow the call stack.
pub fn simplify(points: &[Point], tolerance: f64) -> Vec<Point> {
    if points.len() < 3 {
        return points.to_vec();
    }
    let mut keep = vec![false; points.len()];
    keep[0] = true;
    keep[points.len() - 1] = true;
    let mut stack = vec![(0, points.len() - 1)];
    while let Some((start, end)) = stack.pop() {
        let mut best = (0.0, 0);
        for i in start + 1..end {
            let d = segment_distance(points[i], points[start], points[end]);
            if d > best.0 {
                best = (d, i);
            }
        }
        if best.0 > tolerance {
            keep[best.1] = true;
            stack.push((start, best.1));
            stack.push((best.1, end));
        }
    }
    points
        .iter()
        .zip(keep)
        .filter_map(|(p, k)| k.then_some(*p))
        .collect()
}

/// Simplifies a closed ring (first point NOT repeated at the end). The ring
/// is split at the point farthest from the first one, so both halves are
/// simplified as open lines and nothing collapses.
pub fn simplify_ring(ring: &[Point], tolerance: f64) -> Vec<Point> {
    if ring.len() < 4 {
        return ring.to_vec();
    }
    let first = ring[0];
    let far = (1..ring.len())
        .max_by(|&a, &b| {
            let da = (ring[a].x - first.x).powi(2) + (ring[a].y - first.y).powi(2);
            let db = (ring[b].x - first.x).powi(2) + (ring[b].y - first.y).powi(2);
            da.total_cmp(&db)
        })
        .unwrap_or(ring.len() / 2);
    let mut a = simplify(&ring[..=far], tolerance);
    let mut second: Vec<Point> = ring[far..].to_vec();
    second.push(first);
    let b = simplify(&second, tolerance);
    // `a` ends with ring[far] and `b` starts with it and ends with `first`;
    // join them without duplicating those two points.
    a.extend_from_slice(&b[1..b.len() - 1]);
    a
}

/// Shoelace formula: positive for one winding direction, negative for the
/// other. `abs()` gives the enclosed area.
pub fn signed_area(ring: &[Point]) -> f64 {
    let n = ring.len();
    let mut sum = 0.0;
    for i in 0..n {
        let (a, b) = (ring[i], ring[(i + 1) % n]);
        sum += a.x * b.y - b.x * a.y;
    }
    sum / 2.0
}

/// Even-odd rule point-in-polygon test.
pub fn contains(ring: &[Point], x: f64, y: f64) -> bool {
    let mut inside = false;
    let n = ring.len();
    let mut j = n.wrapping_sub(1);
    for i in 0..n {
        let (a, b) = (ring[i], ring[j]);
        if (a.y > y) != (b.y > y) && x < (b.x - a.x) * (y - a.y) / (b.y - a.y) + a.x {
            inside = !inside;
        }
        j = i;
    }
    inside
}

/// Rounds to 0.1 map px: shorter JSON, and the writer stores what it gets.
pub fn round_point(x: f64, y: f64) -> Point {
    Point {
        x: (x * 10.0).round() / 10.0,
        y: (y * 10.0).round() / 10.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(x: f64, y: f64) -> Point {
        Point { x, y }
    }

    #[test]
    fn simplify_drops_collinear_points() {
        let line: Vec<Point> = (0..=10).map(|i| p(f64::from(i), 0.0)).collect();
        assert_eq!(simplify(&line, 0.1), vec![p(0.0, 0.0), p(10.0, 0.0)]);
        let bent = vec![p(0.0, 0.0), p(5.0, 3.0), p(10.0, 0.0)];
        assert_eq!(simplify(&bent, 1.0).len(), 3);
    }

    #[test]
    fn ring_simplification_keeps_a_square() {
        let mut ring = Vec::new();
        for i in 0..10 {
            ring.push(p(f64::from(i), 0.0));
        }
        for i in 0..10 {
            ring.push(p(10.0, f64::from(i)));
        }
        for i in 0..10 {
            ring.push(p(10.0 - f64::from(i), 10.0));
        }
        for i in 0..10 {
            ring.push(p(0.0, 10.0 - f64::from(i)));
        }
        let simple = simplify_ring(&ring, 0.5);
        assert_eq!(simple.len(), 4);
        assert!((signed_area(&simple).abs() - 100.0).abs() < 1e-9);
    }

    #[test]
    fn point_in_polygon() {
        let square = vec![p(0.0, 0.0), p(4.0, 0.0), p(4.0, 4.0), p(0.0, 4.0)];
        assert!(contains(&square, 2.0, 2.0));
        assert!(!contains(&square, 5.0, 2.0));
    }
}
