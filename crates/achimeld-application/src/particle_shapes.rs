use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
pub struct ParticleShape {
    pub kind: String,
    pub operation: String,
    pub scale: f64,
    pub offset_x: f64,
    pub offset_y: f64,
    pub rotation: f64,
    pub vertex_count: u32,
    pub inner_radius: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inner_radius_min: Option<f64>,
}
impl Default for ParticleShape {
    fn default() -> Self {
        Self {
            kind: "Sphere".into(),
            operation: "Union".into(),
            scale: 1.0,
            offset_x: 0.0,
            offset_y: 0.0,
            rotation: 0.0,
            vertex_count: 6,
            inner_radius: 0.45,
            inner_radius_min: None,
        }
    }
}
impl ParticleShape {
    pub fn validate(&self) -> Result<(), String> {
        if !matches!(self.kind.as_str(), "Sphere" | "Polygon" | "Star")
            || !matches!(self.operation.as_str(), "Union" | "Subtract" | "Intersect")
            || !(3..=16).contains(&self.vertex_count)
        {
            return Err("Invalid particle component shape or operation".into());
        }
        for (value, min, max) in [
            (self.scale, 0.05, 2.0),
            (self.offset_x, -1.0, 1.0),
            (self.offset_y, -1.0, 1.0),
            (self.rotation, -360.0, 360.0),
            (self.inner_radius, 0.05, 1.0),
            (
                self.inner_radius_min.unwrap_or(self.inner_radius),
                0.05,
                self.inner_radius,
            ),
        ] {
            if !value.is_finite() || !(min..=max).contains(&value) {
                return Err("Invalid particle shape transform".into());
            }
        }
        Ok(())
    }
}
