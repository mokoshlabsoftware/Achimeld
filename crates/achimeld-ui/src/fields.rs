//! Shared left-hand label column, with complete wrapping labels in fixed forms.
use achimeld_application::Preferences;

#[derive(Clone, Copy, Default)]
struct LabelRules {
    min_width: f32,
    width_percent: f32,
    abbreviate: bool,
}

const RULES: &str = "achimeld-field-label-rules";
const DRAG: &str = "achimeld-field-label-drag";
const COMMIT: &str = "achimeld-field-label-commit";
const FIXED_WIDTH: &str = "achimeld-fixed-width-fields";

/// Fixed windows and fixed-width sections must retain complete field labels.
/// Mark the containing form, so all nested property helpers use the same rule.
pub(crate) fn fixed_width<R>(ui: &mut egui::Ui, content: impl FnOnce(&mut egui::Ui) -> R) -> R {
    ui.scope_builder(
        egui::UiBuilder::new().ui_stack_info(egui::UiStackInfo::default().with_tag(FIXED_WIDTH)),
        content,
    )
    .inner
}

fn has_fixed_width(ui: &egui::Ui) -> bool {
    ui.stack().iter().any(|entry| {
        entry.tags().contains(FIXED_WIDTH)
            || matches!(
                entry.kind(),
                Some(
                    egui::UiKind::Modal
                        | egui::UiKind::Menu
                        | egui::UiKind::Popup
                        | egui::UiKind::Tooltip
                        | egui::UiKind::Picker
                )
            )
    })
}

pub(crate) fn configure(ctx: &egui::Context, preferences: &Preferences) {
    ctx.data_mut(|data| {
        data.insert_temp(
            egui::Id::new(RULES),
            data.get_temp::<LabelRules>(egui::Id::new(DRAG))
                .unwrap_or(LabelRules {
                    min_width: preferences.field_label_min_width,
                    width_percent: preferences.field_label_width_percent,
                    abbreviate: preferences.abbreviate_field_labels,
                }),
        );
    });
}

pub(crate) fn take_change(ctx: &egui::Context) -> Option<(f32, f32)> {
    if !ctx.input(|input| input.focused && input.pointer.primary_down()) {
        ctx.data_mut(|data| {
            if let Some(next) = data.remove_temp::<LabelRules>(egui::Id::new(DRAG)) {
                data.insert_temp(egui::Id::new(COMMIT), next);
            }
        });
    }
    ctx.data_mut(|data| data.remove_temp::<LabelRules>(egui::Id::new(COMMIT)))
        .map(|rules| (rules.min_width, rules.width_percent))
}

fn divider(ui: &mut egui::Ui, id: egui::Id, row: egui::Rect, label_width: f32, rules: LabelRules) {
    let x = row.left() + label_width + ui.spacing().item_spacing.x * 0.5;
    let rect =
        egui::Rect::from_center_size(egui::pos2(x, row.center().y), egui::vec2(6.0, row.height()))
            .intersect(ui.clip_rect());
    let response = ui
        .interact(rect, id.with("label-divider"), egui::Sense::drag())
        .on_hover_cursor(egui::CursorIcon::ResizeHorizontal);
    if response.hovered() || response.dragged() {
        let panel = ui.clip_rect();
        let painter = ui.painter();
        painter.line_segment(
            [egui::pos2(x, panel.top()), egui::pos2(x, panel.bottom())],
            egui::Stroke::new(3.0, ui.visuals().selection.bg_fill),
        );
        for offset in [-8.0, 0.0, 8.0] {
            painter.circle_filled(
                egui::pos2(x, panel.center().y + offset),
                2.5,
                ui.visuals().text_color(),
            );
        }
    }
    if response.dragged()
        && let Some(pointer) = response.interact_pointer_pos()
    {
        let next = LabelRules {
            min_width: 40.0,
            width_percent: ((pointer.x - row.left() - ui.spacing().item_spacing.x * 0.5)
                / row.width()
                * 100.0)
                .clamp(5.0, 50.0),
            ..rules
        };
        ui.data_mut(|data| {
            data.insert_temp(egui::Id::new(DRAG), next);
            data.insert_temp(egui::Id::new(RULES), next);
        });
        ui.ctx().request_repaint();
    }
    if response.drag_stopped() {
        ui.data_mut(|data| {
            if let Some(next) = data.remove_temp::<LabelRules>(egui::Id::new(DRAG)) {
                data.insert_temp(egui::Id::new(COMMIT), next);
            }
        });
    }
    response.on_hover_text("Drag to resize labels and fields throughout the application");
}

fn rules(ctx: &egui::Context) -> LabelRules {
    ctx.data(|data| data.get_temp(egui::Id::new(RULES)))
        .unwrap_or(LabelRules {
            min_width: 90.0,
            width_percent: 15.0,
            abbreviate: true,
        })
}

fn label_width(rules: LabelRules, available: f32, gap: f32) -> f32 {
    // The minimum takes precedence over the percentage. At physically smaller
    // widths preserve room for the control instead of overflowing the panel.
    (available * rules.width_percent / 100.0)
        .max(rules.min_width)
        .min((available - gap - 24.0).max(0.0))
}

pub(crate) fn minimum_row_width(ui: &egui::Ui, control_width: f32) -> f32 {
    let rules = rules(ui.ctx());
    let control = control_width + ui.spacing().item_spacing.x;
    (rules.min_width + control).max(control / (1.0 - rules.width_percent / 100.0))
}

fn abbreviated(full: &str) -> String {
    let words: Vec<_> = full.split_whitespace().collect();
    if words.len() < 2 || words.last().is_some_and(|word| word.chars().count() == 1) {
        return full.to_owned();
    }
    let mut result = String::new();
    for word in &words[..words.len() - 1] {
        if word.chars().all(|c| !c.is_lowercase()) || word.ends_with('.') {
            result.push_str(word);
        } else if let Some(first) = word.chars().next() {
            result.extend(first.to_uppercase());
            result.push('.');
        }
        result.push(' ');
    }
    let last = words[words.len() - 1];
    let mut chars = last.chars();
    if let Some(first) = chars.next() {
        result.extend(first.to_uppercase());
        result.extend(chars);
    }
    result
}

#[cfg(test)]
pub(crate) fn label_matches(actual: &str, expected: &str) -> bool {
    actual == expected || abbreviated(actual) == expected
}

pub(crate) fn row<R>(
    ui: &mut egui::Ui,
    label: &str,
    content: impl FnOnce(&mut egui::Ui) -> R,
) -> R {
    row_with_icon(ui, label, None, content)
}

pub(crate) fn row_with_icon<R>(
    ui: &mut egui::Ui,
    label: &str,
    icon: Option<(egui::Image<'_>, String)>,
    content: impl FnOnce(&mut egui::Ui) -> R,
) -> R {
    row_impl(ui, label, icon, false, 24.0, |ui, _| content(ui))
}

pub(crate) fn row_with_min_control<R>(
    ui: &mut egui::Ui,
    label: &str,
    min_control_width: f32,
    content: impl FnOnce(&mut egui::Ui) -> R,
) -> R {
    row_impl(ui, label, None, false, min_control_width, |ui, _| {
        content(ui)
    })
}

pub(crate) fn checkbox(ui: &mut egui::Ui, label: &str, value: &mut bool) -> egui::Response {
    row_impl(ui, label, None, true, 24.0, |ui, label| {
        let mut response = ui.checkbox(value, "").labelled_by(label.id);
        if label.clicked() {
            *value = !*value;
            response.mark_changed();
        }
        response
    })
}

fn row_impl<R>(
    ui: &mut egui::Ui,
    label: &str,
    icon: Option<(egui::Image<'_>, String)>,
    clickable_label: bool,
    min_control_width: f32,
    content: impl FnOnce(&mut egui::Ui, &egui::Response) -> R,
) -> R {
    let rules = rules(ui.ctx());
    let fixed = has_fixed_width(ui);
    let full = crate::localization::text(ui.ctx(), label);
    // Measure and paint the same compact, regular-weight property label font.
    let mut font = egui::TextStyle::Body.resolve(ui.style());
    font.size = (font.size - 1.0).max(1.0);
    let text_width = ui
        .painter()
        .layout_no_wrap(full.clone(), font.clone(), ui.visuals().text_color())
        .size()
        .x;
    let icon_width = if icon.is_some() {
        20.0 + ui.spacing().item_spacing.x
    } else {
        0.0
    };
    let width = ui.available_width();
    let label_width = if fixed {
        // Leave room for the field. Long localized labels wrap on the left;
        // global abbreviation and column-budget preferences cannot hide them.
        (text_width + icon_width).ceil().min(width * 0.6)
    } else {
        label_width(rules, width, ui.spacing().item_spacing.x)
    }
    .min((width - ui.spacing().item_spacing.x - min_control_width).max(0.0));
    let short = if rules.abbreviate && !fixed && text_width > (label_width - icon_width).max(0.0) {
        abbreviated(&full)
    } else {
        full.clone()
    };
    let response = ui.horizontal(|ui| {
        ui.set_width(width);
        let height = ui.spacing().interact_size.y;
        let label = ui.allocate_ui_with_layout(
            egui::vec2(label_width, height),
            egui::Layout::left_to_right(egui::Align::Center),
            |ui| {
                ui.set_min_size(egui::vec2(label_width, height));
                let mut clip = ui.clip_rect().intersect(ui.max_rect());
                if fixed {
                    // A wrapped label can make the row taller than interact_size.
                    // Keep its horizontal column bounded without clipping later lines.
                    clip.min.y = ui.clip_rect().min.y;
                    clip.max.y = ui.clip_rect().max.y;
                }
                ui.set_clip_rect(clip);
                if let Some((icon, hint)) = icon {
                    ui.add_sized([20.0, 20.0], icon).on_hover_text(hint);
                }
                let text = egui::Label::new(egui::RichText::new(short).font(font))
                    .wrap_mode(if fixed {
                        egui::TextWrapMode::Wrap
                    } else {
                        egui::TextWrapMode::Truncate
                    })
                    .show_tooltip_when_elided(false)
                    .sense(if clickable_label {
                        egui::Sense::click()
                    } else {
                        egui::Sense::hover()
                    })
                    .halign(egui::Align::LEFT);
                let response = ui.add(text);
                response.widget_info(|| {
                    egui::WidgetInfo::labeled(egui::WidgetType::Label, ui.is_enabled(), &full)
                });
                response.on_hover_text(&full).on_disabled_hover_text(full)
            },
        );
        content(ui, &label.inner)
    });
    if !fixed {
        divider(
            ui,
            response.response.id,
            response.response.rect,
            label_width,
            rules,
        );
    }
    response.inner
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hovered_divider_spans_panel_with_three_centered_grip_dots() {
        let ctx = egui::Context::default();
        let mut panel = egui::Rect::NOTHING;
        let mut point = egui::Pos2::ZERO;
        for hover in [false, true] {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(500.0, 600.0),
                    )),
                    events: if hover {
                        vec![egui::Event::PointerMoved(point)]
                    } else {
                        vec![]
                    },
                    ..Default::default()
                },
                |ui| {
                    panel = ui.clip_rect();
                    let control = row(ui, "Name", |ui| ui.label("Example"));
                    point = egui::pos2(control.rect.left() - 4.0, control.rect.center().y);
                },
            );
            if hover {
                assert!(output.shapes.iter().any(|paint| matches!(&paint.shape,
                    egui::Shape::LineSegment { points, stroke }
                    if stroke.width == 3.0 && points[0].y == panel.top() && points[1].y == panel.bottom())));
                let dots: Vec<_> = output
                    .shapes
                    .iter()
                    .filter_map(|paint| match &paint.shape {
                        egui::Shape::Circle(circle) if circle.radius == 2.5 => Some(circle.center),
                        _ => None,
                    })
                    .collect();
                assert_eq!(dots.len(), 3);
                assert_eq!(dots[1].y, panel.center().y);
            }
            output.textures_delta.clear();
        }
    }

    #[test]
    fn resizing_restores_full_labels_and_respects_abbreviation_preference() {
        for abbreviate in [true, false] {
            let ctx = egui::Context::default();
            let preferences = Preferences {
                field_label_min_width: 40.0,
                field_label_width_percent: 15.0,
                abbreviate_field_labels: abbreviate,
                ..Default::default()
            };
            crate::theme::apply(&ctx, &preferences);
            for width in [1400.0, 400.0, 1400.0] {
                let mut output = ctx.run_ui(Default::default(), |ui| {
                    ui.set_width(width);
                    row(ui, "Bottom height", |ui| {
                        ui.label("45%");
                    });
                });
                let expected = if abbreviate && width == 400.0 {
                    "B. Height"
                } else {
                    "Bottom height"
                };
                assert!(output.shapes.iter().any(|shape| matches!(&shape.shape,
                    egui::Shape::Text(text) if text.galley.text() == expected)));
                output.textures_delta.clear();
            }
        }
    }

    #[test]
    fn fixed_forms_keep_complete_localized_labels_without_clipping_or_overlap() {
        for width in [120.0, 220.0, 420.0] {
            for font in [13.0, 24.0] {
                for scale in [1.0, 1.5, 2.0] {
                    for (minimum, percent) in [(40.0, 5.0), (240.0, 50.0)] {
                        for language in ["English", "Polish"] {
                            let ctx = egui::Context::default();
                            ctx.set_pixels_per_point(scale);
                            let preferences = Preferences {
                                normal_size: font,
                                field_label_min_width: minimum,
                                field_label_width_percent: percent,
                                abbreviate_field_labels: true,
                                ..Default::default()
                            };
                            crate::theme::apply(&ctx, &preferences);
                            crate::localization::language(&ctx, language);
                            let mut control = egui::Rect::NOTHING;
                            let mut output = ctx.run_ui(
                                egui::RawInput {
                                    screen_rect: Some(egui::Rect::from_min_size(
                                        egui::Pos2::ZERO,
                                        egui::vec2(width + 40.0, 600.0),
                                    )),
                                    ..Default::default()
                                },
                                |ui| {
                                    ui.set_width(width);
                                    fixed_width(ui, |ui| {
                                        // A nested layout must retain the fixed-form rule.
                                        ui.scope(|ui| {
                                            row(ui, "WkAcCl", |ui| {
                                                control = ui
                                                    .allocate_exact_size(
                                                        egui::vec2(ui.available_width(), 24.0),
                                                        egui::Sense::hover(),
                                                    )
                                                    .0;
                                            });
                                        });
                                    });
                                    assert!(ui.min_rect().width() <= width + 1.0);
                                },
                            );
                            output.textures_delta.clear();
                            let full = crate::localization::text(&ctx, "WkAcCl");
                            let (shape, text) = output
                                .shapes
                                .iter()
                                .find_map(|shape| match &shape.shape {
                                    egui::Shape::Text(text) if text.galley.text() == full => {
                                        Some((shape, text))
                                    }
                                    _ => None,
                                })
                                .expect("Complete fixed-form label");
                            let rect = text.galley.rect.translate(text.pos.to_vec2());
                            assert!(!text.galley.elided);
                            assert!(
                                shape.clip_rect.expand(1.0).contains_rect(rect),
                                "Clipped {full}: {rect:?} in {:?}",
                                shape.clip_rect
                            );
                            assert!(rect.right() <= control.left(), "Overlapping {full}");
                            assert!(
                                rect.top() <= control.bottom() && control.top() <= rect.bottom()
                            );
                            if width == 120.0 {
                                assert!(text.galley.rows.len() > 1, "Narrow labels must wrap");
                            }
                            assert!(take_change(&ctx).is_none());
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn fixed_container_ancestry_overrides_abbreviations_without_affecting_other_rows() {
        for kind in [
            egui::UiKind::Modal,
            egui::UiKind::Popup,
            egui::UiKind::Menu,
            egui::UiKind::Tooltip,
            egui::UiKind::Picker,
        ] {
            let ctx = egui::Context::default();
            let mut output = ctx.run_ui(Default::default(), |ui| {
                ui.scope_builder(
                    egui::UiBuilder::new().ui_stack_info(egui::UiStackInfo::new(kind)),
                    |ui| {
                        ui.scope(|ui| {
                            row(ui, "Steam App ID", |ui| {
                                ui.label("480");
                            });
                        });
                    },
                );
                row(ui, "Bottom height", |ui| {
                    ui.label("45%");
                });
            });
            output.textures_delta.clear();
            let labels: Vec<_> = output
                .shapes
                .iter()
                .filter_map(|shape| match &shape.shape {
                    egui::Shape::Text(text) => Some(text.galley.text()),
                    _ => None,
                })
                .collect();
            assert!(labels.contains(&"Steam App ID"), "{kind:?}");
            assert!(!labels.contains(&"S. A. ID"), "{kind:?}");
            assert!(
                labels.contains(&"Bottom height"),
                "Relaxed rows must retain full labels"
            );
        }
    }
    #[test]
    fn dragging_a_label_divider_resizes_all_rows_and_commits_once_on_release() {
        let ctx = egui::Context::default();
        let mut rectangles = [egui::Rect::NOTHING; 2];
        let mut frame = |events| {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(420.0, 200.0),
                    )),
                    events,
                    ..Default::default()
                },
                |ui| {
                    configure(ui.ctx(), &Preferences::default());
                    ui.set_width(400.0);
                    for (index, label) in ["Top height", "Bottom height"].into_iter().enumerate() {
                        row(ui, label, |ui| {
                            rectangles[index] = ui
                                .allocate_exact_size(
                                    egui::vec2(ui.available_width(), 24.0),
                                    egui::Sense::hover(),
                                )
                                .0;
                        });
                    }
                },
            );
            output.textures_delta.clear();
            rectangles
        };
        let original = frame(vec![]);
        let press = egui::pos2(original[0].left() - 4.0, original[0].center().y);
        frame(vec![egui::Event::PointerMoved(press)]);
        frame(vec![egui::Event::PointerButton {
            pos: press,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: egui::Modifiers::NONE,
        }]);
        let moved = press + egui::vec2(60.0, 0.0);
        frame(vec![egui::Event::PointerMoved(moved)]);
        let updated = frame(vec![]);
        assert!(updated[0].left() > original[0].left() + 50.0);
        assert!((updated[0].left() - updated[1].left()).abs() < 1.0);
        assert!(take_change(&ctx).is_none());
        frame(vec![egui::Event::PointerButton {
            pos: moved,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        }]);
        let (minimum, ratio) = take_change(&ctx).expect("Commit on release");
        assert_eq!(minimum, 40.0);
        assert!(ratio > 35.0 && ratio < 45.0);
        assert!(take_change(&ctx).is_none());
    }

    #[test]
    fn label_budget_uses_minimum_then_percentage_and_caps_impossible_widths() {
        let rules = LabelRules {
            min_width: 90.0,
            width_percent: 15.0,
            abbreviate: true,
        };
        assert_eq!(label_width(rules, 320.0, 8.0), 90.0);
        assert_eq!(label_width(rules, 1000.0, 8.0), 150.0);
        assert_eq!(label_width(rules, 80.0, 8.0), 48.0);
    }

    #[test]
    fn short_labels_keep_their_meaning_and_preserve_acronyms_and_axes() {
        assert_eq!(abbreviated("Bottom height"), "B. Height");
        assert_eq!(abbreviated("Random scale"), "R. Scale");
        assert_eq!(abbreviated("Steam App ID"), "S. A. ID");
        assert_eq!(abbreviated("Offset X"), "Offset X");
        assert_eq!(abbreviated("Width"), "Width");
        assert_eq!(abbreviated("下端の高さ"), "下端の高さ");
    }

    #[test]
    fn labels_remain_left_aligned_at_narrow_widths_and_larger_fonts() {
        for width in [160.0, 240.0, 360.0, 1000.0] {
            for font_size in [13.0, 24.0] {
                let ctx = egui::Context::default();
                let preferences = Preferences {
                    normal_size: font_size,
                    ..Default::default()
                };
                crate::theme::apply(&ctx, &preferences);
                let mut label = egui::Rect::NOTHING;
                let mut control = egui::Rect::NOTHING;
                let mut available = 0.0;
                let mut left = 0.0;
                let mut output = ctx.run_ui(
                    egui::RawInput {
                        screen_rect: Some(egui::Rect::from_min_size(
                            egui::Pos2::ZERO,
                            egui::vec2(width + 40.0, 200.0),
                        )),
                        ..Default::default()
                    },
                    |ui| {
                        ui.set_width(width);
                        available = ui.available_width();
                        left = ui.cursor().left();
                        control = row(ui, "Width", |ui| {
                            ui.allocate_exact_size(
                                egui::vec2(ui.available_width(), 32.0),
                                egui::Sense::hover(),
                            )
                            .0
                        });
                    },
                );
                output.textures_delta.clear();
                for shape in output.shapes {
                    if let egui::Shape::Text(text) = shape.shape
                        && text.galley.text() == "Width"
                    {
                        label = text.galley.rect.translate(text.pos.to_vec2());
                    }
                }
                assert!(label.is_positive(), "Missing label at {width}/{font_size}");
                let expected = left + label_width(rules(&ctx), available, 8.0) + 8.0;
                assert!(
                    (control.left() - expected).abs() < 1.0,
                    "{width}/{font_size}: {control:?} vs {expected}"
                );
                assert!(label.right() <= control.left());
                assert!((label.left() - left).abs() < 1.0);
                assert!((label.center().y - control.center().y).abs() < 1.0);
                assert!(control.right() <= left + available + 1.0);
            }
        }
    }

    #[test]
    fn abbreviated_labels_explain_the_full_name_on_hover() {
        let ctx = egui::Context::default();
        crate::theme::apply(
            &ctx,
            &Preferences {
                field_label_min_width: 60.0,
                ..Default::default()
            },
        );
        let mut style = (*ctx.global_style()).clone();
        style.interaction.tooltip_delay = 0.0;
        ctx.set_global_style(style);
        let frame = |time, events| {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    time: Some(time),
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(400.0, 200.0),
                    )),
                    events,
                    ..Default::default()
                },
                |ui| {
                    ui.set_width(220.0);
                    row(ui, "Bottom height", |ui| {
                        ui.label("45–100%");
                    });
                },
            );
            output.textures_delta.clear();
            output
        };
        let first = frame(0.0, vec![]);
        let label = first
            .shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                egui::Shape::Text(text) if text.galley.text() == "B. Height" => {
                    Some(text.galley.rect.translate(text.pos.to_vec2()))
                }
                _ => None,
            })
            .expect("Visible short label");
        let events = vec![egui::Event::PointerMoved(label.center())];
        frame(1.0, events.clone());
        frame(2.0, events.clone());
        let hovered = frame(3.0, events);
        assert!(
            hovered.shapes.iter().any(|shape| matches!(&shape.shape,
            egui::Shape::Text(text) if text.galley.text() == "Bottom height")),
            "Missing full-name tooltip"
        );
    }

    #[test]
    fn left_checkbox_labels_toggle_the_field_and_respect_disabled_state() {
        for enabled in [true, false] {
            let ctx = egui::Context::default();
            let mut value = false;
            let mut frame = |events| {
                let mut output = ctx.run_ui(
                    egui::RawInput {
                        screen_rect: Some(egui::Rect::from_min_size(
                            egui::Pos2::ZERO,
                            egui::vec2(320.0, 120.0),
                        )),
                        events,
                        ..Default::default()
                    },
                    |ui| {
                        ui.add_enabled_ui(enabled, |ui| {
                            checkbox(ui, "Visible", &mut value);
                        });
                    },
                );
                output.textures_delta.clear();
                output
            };
            let output = frame(vec![]);
            let label = output
                .shapes
                .iter()
                .find_map(|shape| match &shape.shape {
                    egui::Shape::Text(text) if text.galley.text() == "Visible" => {
                        Some(text.galley.rect.translate(text.pos.to_vec2()).center())
                    }
                    _ => None,
                })
                .unwrap();
            for pressed in [true, false] {
                frame(vec![
                    egui::Event::PointerMoved(label),
                    egui::Event::PointerButton {
                        pos: label,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: egui::Modifiers::NONE,
                    },
                ]);
            }
            assert_eq!(value, enabled);
        }
    }
}
