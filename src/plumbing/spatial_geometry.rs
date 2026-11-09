// [LAYER: PLUMBING]
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Type aliases for spatial measurement and normalization.
pub type NormalizedCoordinate = f64;
pub type SpatialDimension = f64;
pub type AreaMeasure = f64;
pub type PixelOffset = f64;
pub type DevicePixelRatio = f64;
pub type SpatialRatio = f64;
pub type TrajectoryViolation = String;
pub type ViewportScrollDelta = (f64, f64);

/// Abstract contractual interface for 2D spatial point operations.
pub trait ISpatialPointGeometry {
    fn point_distance(&self, other: &SpatialPoint) -> f64;
    fn point_translate(&self, dx: f64, dy: f64) -> SpatialPoint;
}

/// Abstract contractual interface for 2D spatial rectangle operations.
pub trait ISpatialRectGeometry {
    fn rect_area(&self) -> f64;
    fn rect_safe_center(&self) -> SpatialPoint;
    fn rect_intersects(&self, other: &SpatialRect) -> bool;
    fn rect_iou(&self, other: &SpatialRect) -> f64;
    fn rect_occlusion_ratio(&self, occluder: &SpatialRect) -> f64;
}

/// Abstract contractual interface for viewport transforms.
pub trait IViewportTransformGeometry {
    fn check_visibility(&self, rect: &SpatialRect) -> bool;
    fn calculate_visibility_ratio(&self, rect: &SpatialRect) -> f64;
}

/// Represents a 2D coordinate point with floating point subpixel precision.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct SpatialPoint {
    pub x: f64,
    pub y: f64,
}

impl SpatialPoint {
    pub const fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }

    pub const fn zero() -> Self {
        Self { x: 0.0, y: 0.0 }
    }

    pub fn distance(&self, other: &SpatialPoint) -> f64 {
        self.distance_sq(other).sqrt()
    }

    pub fn distance_sq(&self, other: &SpatialPoint) -> f64 {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        dx * dx + dy * dy
    }

    pub fn translate(&self, dx: f64, dy: f64) -> SpatialPoint {
        SpatialPoint {
            x: self.x + dx,
            y: self.y + dy,
        }
    }

    pub fn scale(&self, factor: f64) -> SpatialPoint {
        SpatialPoint {
            x: self.x * factor,
            y: self.y * factor,
        }
    }

    pub fn to_i32(&self) -> [i32; 2] {
        [self.x.round() as i32, self.y.round() as i32]
    }

    pub fn from_i32(pt: [i32; 2]) -> Self {
        Self {
            x: pt[0] as f64,
            y: pt[1] as f64,
        }
    }
}

/// Axis-Aligned Bounding Box (AABB) with subpixel precision and geometric primitives.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct SpatialRect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl SpatialRect {
    pub const fn new(x: f64, y: f64, width: f64, height: f64) -> Self {
        Self { x, y, width, height }
    }

    pub const fn zero() -> Self {
        Self { x: 0.0, y: 0.0, width: 0.0, height: 0.0 }
    }

    pub fn from_i32_slice(b: [i32; 4]) -> Self {
        Self {
            x: b[0] as f64,
            y: b[1] as f64,
            width: b[2].max(0) as f64,
            height: b[3].max(0) as f64,
        }
    }

    pub fn to_i32_slice(&self) -> [i32; 4] {
        [
            self.x.round() as i32,
            self.y.round() as i32,
            self.width.max(0.0).round() as i32,
            self.height.max(0.0).round() as i32,
        ]
    }

    pub fn is_valid(&self) -> bool {
        self.x.is_finite()
            && self.y.is_finite()
            && self.width.is_finite()
            && self.height.is_finite()
            && self.width > 0.0
            && self.height > 0.0
    }

    pub fn area(&self) -> f64 {
        if self.width <= 0.0 || self.height <= 0.0 {
            0.0
        } else {
            self.width * self.height
        }
    }

    pub fn min_x(&self) -> f64 {
        self.x
    }

    pub fn min_y(&self) -> f64 {
        self.y
    }

    pub fn max_x(&self) -> f64 {
        self.x + self.width
    }

    pub fn max_y(&self) -> f64 {
        self.y + self.height
    }

    pub fn center(&self) -> SpatialPoint {
        SpatialPoint {
            x: self.x + self.width * 0.5,
            y: self.y + self.height * 0.5,
        }
    }

    /// Computes a safe interactive point inside the rectangle by insetting margins.
    /// `inset_ratio` is clamped to [0.0, 0.45] to guarantee the safe point stays well within bounds.
    pub fn safe_center(&self, inset_ratio: f64) -> SpatialPoint {
        let ratio = inset_ratio.clamp(0.0, 0.45);
        let inset_w = self.width * ratio;
        let inset_h = self.height * ratio;
        let safe_rect = SpatialRect {
            x: self.x + inset_w,
            y: self.y + inset_h,
            width: (self.width - inset_w * 2.0).max(1.0),
            height: (self.height - inset_h * 2.0).max(1.0),
        };
        safe_rect.center()
    }

    pub fn contains_point(&self, pt: &SpatialPoint) -> bool {
        pt.x >= self.min_x() && pt.x <= self.max_x() && pt.y >= self.min_y() && pt.y <= self.max_y()
    }

    pub fn intersects(&self, other: &SpatialRect) -> bool {
        self.min_x() < other.max_x()
            && self.max_x() > other.min_x()
            && self.min_y() < other.max_y()
            && self.max_y() > other.min_y()
    }

    pub fn intersection(&self, other: &SpatialRect) -> Option<SpatialRect> {
        let ix1 = self.min_x().max(other.min_x());
        let iy1 = self.min_y().max(other.min_y());
        let ix2 = self.max_x().min(other.max_x());
        let iy2 = self.max_y().min(other.max_y());

        if ix2 > ix1 && iy2 > iy1 {
            Some(SpatialRect {
                x: ix1,
                y: iy1,
                width: ix2 - ix1,
                height: iy2 - iy1,
            })
        } else {
            None
        }
    }

    pub fn union(&self, other: &SpatialRect) -> SpatialRect {
        let ux1 = self.min_x().min(other.min_x());
        let uy1 = self.min_y().min(other.min_y());
        let ux2 = self.max_x().max(other.max_x());
        let uy2 = self.max_y().max(other.max_y());
        SpatialRect {
            x: ux1,
            y: uy1,
            width: ux2 - ux1,
            height: uy2 - uy1,
        }
    }

    /// Computes Intersection over Union (IoU) metric in range [0.0, 1.0].
    pub fn intersection_over_union(&self, other: &SpatialRect) -> f64 {
        let inter_area = self.intersection(other).map(|r| r.area()).unwrap_or(0.0);
        let union_area = self.area() + other.area() - inter_area;
        if union_area <= 0.0 {
            0.0
        } else {
            (inter_area / union_area).clamp(0.0, 1.0)
        }
    }

    /// Computes what fraction of `self` is covered by `occluder` in range [0.0, 1.0].
    pub fn occlusion_ratio(&self, occluder: &SpatialRect) -> f64 {
        let self_area = self.area();
        if self_area <= 0.0 {
            return 1.0;
        }
        let inter_area = self.intersection(occluder).map(|r| r.area()).unwrap_or(0.0);
        (inter_area / self_area).clamp(0.0, 1.0)
    }

    pub fn translate(&self, dx: f64, dy: f64) -> SpatialRect {
        SpatialRect {
            x: self.x + dx,
            y: self.y + dy,
            width: self.width,
            height: self.height,
        }
    }

    pub fn scale(&self, factor: f64) -> SpatialRect {
        SpatialRect {
            x: self.x * factor,
            y: self.y * factor,
            width: self.width * factor,
            height: self.height * factor,
        }
    }

    pub fn clamp_to_bounds(&self, bounds: &SpatialRect) -> SpatialRect {
        let cx1 = self.min_x().clamp(bounds.min_x(), bounds.max_x());
        let cy1 = self.min_y().clamp(bounds.min_y(), bounds.max_y());
        let cx2 = self.max_x().clamp(bounds.min_x(), bounds.max_x());
        let cy2 = self.max_y().clamp(bounds.min_y(), bounds.max_y());
        SpatialRect {
            x: cx1,
            y: cy1,
            width: (cx2 - cx1).max(0.0),
            height: (cy2 - cy1).max(0.0),
        }
    }

    pub fn is_to_right_of(&self, other: &SpatialRect) -> bool {
        self.min_x() >= other.max_x()
    }

    pub fn is_to_left_of(&self, other: &SpatialRect) -> bool {
        self.max_x() <= other.min_x()
    }

    pub fn is_below(&self, other: &SpatialRect) -> bool {
        self.min_y() >= other.max_y()
    }

    pub fn is_above(&self, other: &SpatialRect) -> bool {
        self.max_y() <= other.min_y()
    }
}

/// Viewport transformation matrix for scrolling, scaling, device-pixel-ratio (DPR),
/// and window-to-screen conversions.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ViewportTransform {
    pub scroll_x: f64,
    pub scroll_y: f64,
    pub scale: f64,
    pub dpr: f64,
    pub viewport_width: f64,
    pub viewport_height: f64,
}

impl Default for ViewportTransform {
    fn default() -> Self {
        Self::new(1440.0, 900.0)
    }
}

impl ViewportTransform {
    pub const fn new(viewport_width: f64, viewport_height: f64) -> Self {
        Self {
            scroll_x: 0.0,
            scroll_y: 0.0,
            scale: 1.0,
            dpr: 1.0,
            viewport_width,
            viewport_height,
        }
    }

    pub fn with_scroll(mut self, scroll_x: f64, scroll_y: f64) -> Self {
        self.scroll_x = scroll_x;
        self.scroll_y = scroll_y;
        self
    }

    pub fn with_dpr(mut self, dpr: f64) -> Self {
        self.dpr = if dpr <= 0.0 { 1.0 } else { dpr };
        self
    }

    pub fn with_scale(mut self, scale: f64) -> Self {
        self.scale = if scale <= 0.0 { 1.0 } else { scale };
        self
    }

    pub fn viewport_rect(&self) -> SpatialRect {
        SpatialRect {
            x: 0.0,
            y: 0.0,
            width: self.viewport_width,
            height: self.viewport_height,
        }
    }

    /// Transforms an element rect in document space to viewport space (applying scroll and scale).
    pub fn element_to_viewport_rect(&self, doc_rect: &SpatialRect) -> SpatialRect {
        SpatialRect {
            x: (doc_rect.x - self.scroll_x) * self.scale,
            y: (doc_rect.y - self.scroll_y) * self.scale,
            width: doc_rect.width * self.scale,
            height: doc_rect.height * self.scale,
        }
    }

    /// Transforms a viewport-space rect to global screen space given the window offset.
    pub fn viewport_to_screen_rect(&self, vp_rect: &SpatialRect, win_x: f64, win_y: f64) -> SpatialRect {
        SpatialRect {
            x: (vp_rect.x + win_x) * self.dpr,
            y: (vp_rect.y + win_y) * self.dpr,
            width: vp_rect.width * self.dpr,
            height: vp_rect.height * self.dpr,
        }
    }

    /// Transforms screen coordinate point back to viewport coordinates.
    pub fn screen_to_viewport_point(&self, screen_pt: &SpatialPoint, win_x: f64, win_y: f64) -> SpatialPoint {
        let base_x = (screen_pt.x / self.dpr) - win_x;
        let base_y = (screen_pt.y / self.dpr) - win_y;
        SpatialPoint { x: base_x, y: base_y }
    }

    /// Checks if a document-space rect is visible in the current viewport.
    pub fn is_visible_in_viewport(&self, doc_rect: &SpatialRect) -> bool {
        let vp_rect = self.element_to_viewport_rect(doc_rect);
        self.viewport_rect().intersects(&vp_rect)
    }

    /// Calculates what fraction of the element rect is visible in the viewport [0.0, 1.0].
    pub fn visibility_ratio(&self, doc_rect: &SpatialRect) -> f64 {
        let vp_rect = self.element_to_viewport_rect(doc_rect);
        let vp = self.viewport_rect();
        if vp_rect.area() <= 0.0 {
            return 0.0;
        }
        vp.intersection(&vp_rect).map(|i| i.area() / vp_rect.area()).unwrap_or(0.0).clamp(0.0, 1.0)
    }

    /// Computes the scroll adjustment (delta_x, delta_y) needed to center the element in the viewport.
    pub fn compute_centering_scroll_delta(&self, doc_rect: &SpatialRect) -> (f64, f64) {
        let target_center = doc_rect.center();
        let desired_scroll_x = target_center.x - (self.viewport_width / 2.0);
        let desired_scroll_y = target_center.y - (self.viewport_height / 2.0);
        (desired_scroll_x - self.scroll_x, desired_scroll_y - self.scroll_y)
    }
}

/// Universal coordinate space normalizer supporting px, normalized float (0.0..1.0),
/// normalized 1000 ([0..1000]), and percentage ("50%").
pub fn normalize_coordinate_value(val: Option<&Value>, dimension: f64, default_val: f64, is_norm_1000: bool) -> f64 {
    let parsed: Option<f64> = match val {
        Some(Value::Number(n)) => n.as_f64(),
        Some(Value::String(s)) => {
            let trimmed = s.trim();
            if let Some(pct) = trimmed.strip_suffix('%') {
                pct.trim().parse::<f64>().ok().map(|p| (p / 100.0) * dimension)
            } else if let Some(px) = trimmed.strip_suffix("px") {
                px.trim().parse::<f64>().ok()
            } else {
                trimmed.parse::<f64>().ok()
            }
        }
        _ => None,
    };

    match parsed {
        Some(f) if f.is_finite() => {
            if f > 0.0 && f <= 1.0 {
                f * dimension
            } else if is_norm_1000 && f > 1.0 && f <= 1000.0 && dimension > 1000.0 {
                (f * dimension) / 1000.0
            } else {
                f
            }
        }
        _ => default_val,
    }
}

/// Validates that an action trajectory (e.g. for drag, swipe, or pointer motion)
/// remains strictly within allowable screen bounds and does not penetrate forbidden exclusion zones.
pub fn validate_trajectory(
    from: &SpatialPoint,
    to: &SpatialPoint,
    bounds: &SpatialRect,
    exclusion_zones: &[SpatialRect],
    sample_steps: usize,
) -> Result<(), String> {
    if !bounds.contains_point(from) {
        return Err(format!("Trajectory start point ({:.1}, {:.1}) is out of bounds", from.x, from.y));
    }
    if !bounds.contains_point(to) {
        return Err(format!("Trajectory end point ({:.1}, {:.1}) is out of bounds", to.x, to.y));
    }

    let steps = sample_steps.clamp(2, 64);
    for step in 0..=steps {
        let t = step as f64 / steps as f64;
        let pt = SpatialPoint {
            x: from.x + (to.x - from.x) * t,
            y: from.y + (to.y - from.y) * t,
        };
        for zone in exclusion_zones {
            if zone.contains_point(&pt) {
                return Err(format!(
                    "Trajectory penetrates forbidden exclusion zone at ({:.1}, {:.1})",
                    pt.x, pt.y
                ));
            }
        }
    }
    Ok(())
}

/// Computes the exact 2D union area of a collection of bounding rectangles
/// using a coordinate-compression 1D interval decomposition (scanline) algorithm.
pub fn calculate_rectangles_union_area(rects: &[SpatialRect]) -> f64 {
    let valid_rects: Vec<&SpatialRect> = rects.iter().filter(|r| r.is_valid() && r.area() > 0.0).collect();
    if valid_rects.is_empty() {
        return 0.0;
    }
    if valid_rects.len() == 1 {
        return valid_rects[0].area();
    }
    if valid_rects.len() == 2 {
        let inter_area = valid_rects[0].intersection(valid_rects[1]).map(|i| i.area()).unwrap_or(0.0);
        return (valid_rects[0].area() + valid_rects[1].area() - inter_area).max(0.0);
    }

    // Collect and sort all distinct x-coordinates
    let mut x_coords: Vec<f64> = Vec::with_capacity(valid_rects.len() * 2);
    for r in &valid_rects {
        x_coords.push(r.x);
        x_coords.push(r.x + r.width);
    }
    x_coords.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    x_coords.dedup_by(|a, b| (*a - *b).abs() < 1e-9);

    let mut total_area = 0.0;
    for window in x_coords.windows(2) {
        let x1 = window[0];
        let x2 = window[1];
        let dx = x2 - x1;
        if dx <= 1e-9 {
            continue;
        }

        // Find all y-intervals from rectangles that span this x interval
        let mut y_intervals: Vec<(f64, f64)> = Vec::new();
        for r in &valid_rects {
            if r.x <= x1 + 1e-9 && (r.x + r.width) >= x2 - 1e-9 {
                y_intervals.push((r.y, r.y + r.height));
            }
        }

        if y_intervals.is_empty() {
            continue;
        }

        // Merge overlapping y-intervals
        y_intervals.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
        let mut merged_y_length = 0.0;
        let mut cur_start = y_intervals[0].0;
        let mut cur_end = y_intervals[0].1;

        for &(y_start, y_end) in &y_intervals[1..] {
            if y_start <= cur_end + 1e-9 {
                if y_end > cur_end {
                    cur_end = y_end;
                }
            } else {
                merged_y_length += (cur_end - cur_start).max(0.0);
                cur_start = y_start;
                cur_end = y_end;
            }
        }
        merged_y_length += (cur_end - cur_start).max(0.0);

        total_area += dx * merged_y_length;
    }

    total_area.max(0.0)
}

/// Normalizes a coordinate scalar into pixel units depending on the target space.
pub fn normalize_coordinate_in_space(val: f64, dimension: f64, space: &str) -> f64 {
    match space {
        "unit" | "normalized" => val * dimension,
        "normalized_1000" => (val * dimension) / 1000.0,
        _ => val,
    }
}
