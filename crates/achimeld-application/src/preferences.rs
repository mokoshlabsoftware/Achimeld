use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Preferences {
    pub language: String,
    pub control_radius: u8,
    pub header_size: f32,
    pub normal_size: f32,
    pub small_size: f32,
    #[serde(skip_serializing_if = "is_default_knob_size")]
    pub knob_label_size: f32,
    #[serde(skip_serializing_if = "is_default_knob_size")]
    pub knob_value_size: f32,
    pub inspector_spacing: f32,
    #[serde(skip_serializing_if = "is_default_label_min_width")]
    pub field_label_min_width: f32,
    #[serde(skip_serializing_if = "is_default_label_width_percent")]
    pub field_label_width_percent: f32,
    #[serde(skip_serializing_if = "is_true")]
    pub abbreviate_field_labels: bool,
    #[serde(skip_serializing_if = "is_true")]
    pub layer_properties_in_sidebar: bool,
    #[serde(skip_serializing_if = "is_true")]
    pub show_effect_mini_values: bool,
    pub naming_convention: String,
    pub achievement_nodes_per_row: u8,
    pub stat_nodes_per_row: u8,
    pub sort_nodes_by_icon_style: bool,
    pub sort_nodes_by_stat_activation: bool,
    pub new_line_per_icon_style: bool,
    pub fill_nodes_by_columns: bool,
    pub order_selected_nodes_only: bool,
    pub include_helpers_in_sorting: bool,
    pub palette: BTreeMap<String, [u8; 4]>,
}
fn is_true(value: &bool) -> bool {
    *value
}
fn is_default_knob_size(value: &f32) -> bool {
    *value == 10.0
}
fn is_default_label_min_width(value: &f32) -> bool {
    *value == 90.0
}
fn is_default_label_width_percent(value: &f32) -> bool {
    *value == 15.0
}
impl Default for Preferences {
    fn default() -> Self {
        let colors = [
            ("Window", [23, 25, 29, 255]),
            ("Panel", [32, 35, 41, 255]),
            ("Control", [42, 46, 53, 255]),
            ("Border", [65, 71, 80, 255]),
            ("Primary text", [241, 243, 245, 255]),
            ("Muted text", [167, 173, 183, 255]),
            ("Accent", [124, 156, 255, 255]),
            ("Success", [100, 199, 141, 255]),
            ("Warning", [244, 190, 91, 255]),
            ("Error", [255, 114, 114, 255]),
            ("Background top", [21, 21, 21, 255]),
            ("Background bottom", [48, 48, 48, 255]),
            ("Grid", [56, 56, 56, 255]),
        ];
        Self {
            language: "English".into(),
            control_radius: 6,
            header_size: 18.0,
            normal_size: 13.0,
            small_size: 11.0,
            knob_label_size: 10.0,
            knob_value_size: 10.0,
            inspector_spacing: 7.0,
            field_label_min_width: 90.0,
            field_label_width_percent: 15.0,
            abbreviate_field_labels: true,
            layer_properties_in_sidebar: true,
            show_effect_mini_values: true,
            naming_convention: "ACHIEVEMENT_{0:000}".into(),
            achievement_nodes_per_row: 5,
            stat_nodes_per_row: 5,
            sort_nodes_by_icon_style: false,
            sort_nodes_by_stat_activation: false,
            new_line_per_icon_style: false,
            fill_nodes_by_columns: false,
            order_selected_nodes_only: false,
            include_helpers_in_sorting: false,
            palette: colors.into_iter().map(|(n, c)| (n.into(), c)).collect(),
        }
    }
}
impl Preferences {
    pub fn validate(&self) -> Result<(), String> {
        if !achimeld_core::INTERFACE_LANGUAGES.contains(&self.language.as_str())
            || self.control_radius > 24
            || !(10.0..=48.0).contains(&self.header_size)
            || !(9.0..=32.0).contains(&self.normal_size)
            || !(8.0..=24.0).contains(&self.small_size)
            || !(8.0..=24.0).contains(&self.knob_label_size)
            || !(8.0..=24.0).contains(&self.knob_value_size)
            || !(0.0..=32.0).contains(&self.inspector_spacing)
            || !(40.0..=240.0).contains(&self.field_label_min_width)
            || !(5.0..=50.0).contains(&self.field_label_width_percent)
            || !(1..=10).contains(&self.achievement_nodes_per_row)
            || !(1..=10).contains(&self.stat_nodes_per_row)
        {
            return Err("Invalid appearance preferences".into());
        }
        if Preferences::default()
            .palette
            .keys()
            .any(|key| !self.palette.contains_key(key))
        {
            return Err("Missing palette color".into());
        }
        self.achievement_api_name(1)?;
        Ok(())
    }
    pub fn achievement_api_name(&self, index: usize) -> Result<String, String> {
        let pattern = &self.naming_convention;
        let start = pattern
            .find('{')
            .ok_or("Naming convention needs {0} or {0:000}")?;
        let end = pattern[start..]
            .find('}')
            .map(|i| i + start)
            .ok_or("Naming convention has an unclosed placeholder")?;
        let token = &pattern[start + 1..end];
        let digits = if token == "0" {
            index.to_string()
        } else {
            let format = token
                .strip_prefix("0:")
                .ok_or("Only the numbered {0} placeholder is supported")?;
            if format.is_empty() || format.len() > 9 || !format.bytes().all(|b| b == b'0') {
                return Err("Number padding requires between one and nine zeroes".into());
            }
            format!("{index:0width$}", width = format.len())
        };
        let result = format!("{}{}{}", &pattern[..start], digits, &pattern[end + 1..]);
        if result.contains(['{', '}']) {
            return Err("Naming convention must contain exactly one numbered placeholder".into());
        }
        achimeld_core::SteamApiName::new(&result).map_err(|e| e.to_string())?;
        Ok(result)
    }
}
