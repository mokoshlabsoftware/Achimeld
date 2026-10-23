use egui::{Color32, Pos2, Rect, Sense, Stroke, Vec2};

#[derive(Clone, Copy)]
pub(crate) enum Icon {
    Layers,
    Monitor,
    Eye(bool),
    Folder,
    Delete,
    Copy,
    Paste,
    Close,
    Effects,
    Plus,
    Star(bool),
    Chevron(bool),
    Opacity,
    Atlas,
}

pub(crate) fn button(ui: &mut egui::Ui, icon: Icon, label: &str, enabled: bool) -> egui::Response {
    sized_button(ui, icon, label, enabled, 28.0)
}

pub(crate) const CLOSE_BUTTON_SIZE: f32 = 32.0;

pub(crate) fn close_button(ui: &mut egui::Ui, id: egui::Id, label: &str) -> egui::Response {
    header_button(ui, id, Icon::Close, label, true)
}

pub(crate) fn header_button(
    ui: &mut egui::Ui,
    id: egui::Id,
    icon: Icon,
    label: &str,
    enabled: bool,
) -> egui::Response {
    let response = ui
        .scope(|ui| {
            // Empty content keeps font metrics out of the square's dimensions.
            ui.spacing_mut().button_padding = Vec2::ZERO;
            ui.spacing_mut().interact_size = Vec2::splat(CLOSE_BUTTON_SIZE);
            ui.add_enabled(
                enabled,
                crate::buttons::Button::new(egui::Atoms::default())
                    .id(id)
                    .selected(matches!(icon, Icon::Star(true)))
                    .min_size(Vec2::splat(CLOSE_BUTTON_SIZE))
                    .bar(),
            )
        })
        .inner;
    let style = if matches!(icon, Icon::Star(true)) {
        crate::buttons::ButtonStyle::Accent
    } else {
        crate::buttons::ButtonStyle::Quiet
    };
    let (_, ink) = crate::buttons::colors(ui, &response, style);
    let glyph_size = if matches!(icon, Icon::Close) {
        13.5
    } else {
        16.0
    };
    paint(
        ui.painter(),
        Rect::from_center_size(response.rect.center(), Vec2::splat(glyph_size)),
        icon,
        ink,
    );
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, response.enabled(), label)
    });
    response.on_hover_text(crate::localization::text(ui.ctx(), label))
}

pub(crate) fn sized_button(
    ui: &mut egui::Ui,
    icon: Icon,
    label: &str,
    enabled: bool,
    size: f32,
) -> egui::Response {
    let response = ui.add_enabled(
        enabled,
        crate::buttons::Button::new("")
            .selected(matches!(icon, Icon::Star(true)))
            .min_size(Vec2::splat(size)),
    );
    let (_, color) = crate::buttons::colors(
        ui,
        &response,
        if matches!(icon, Icon::Star(true)) {
            crate::buttons::ButtonStyle::Accent
        } else {
            crate::buttons::ButtonStyle::Standard
        },
    );
    // Layout/font metrics can make the actual button larger than its minimum.
    // Center a square glyph in that actual body instead of stretching its inset.
    let glyph_size: f32 = if matches!(icon, Icon::Eye(_)) {
        22.0
    } else {
        16.0
    };
    paint(
        ui.painter(),
        Rect::from_center_size(response.rect.center(), Vec2::splat(glyph_size.min(size))),
        icon,
        color,
    );
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, label));
    response.on_hover_text(crate::localization::text(ui.ctx(), label))
}

pub(crate) fn layer_button(
    ui: &mut egui::Ui,
    id: egui::Id,
    icon: Icon,
    label: &str,
    icon_opacity: f32,
    body_opacity: f32,
) -> egui::Response {
    let size = layer_control_size(icon);
    let response = ui
        .scope(|ui| {
            ui.spacing_mut().button_padding = Vec2::ZERO;
            ui.spacing_mut().interact_size.y = size;
            let button = crate::buttons::Button::new(egui::Atoms::default())
                .id(id)
                .subtle_body(body_opacity)
                .sense(Sense::click_and_drag())
                .min_size(Vec2::splat(size));
            ui.add_sized(
                Vec2::splat(size),
                if matches!(icon, Icon::Effects) {
                    button.square_corners()
                } else {
                    button
                },
            )
        })
        .inner;
    let (_, ink) = crate::buttons::subtle_colors(
        ui,
        &response,
        crate::buttons::ButtonStyle::Standard,
        body_opacity,
    );
    let glyph_size = if matches!(icon, Icon::Effects) {
        16.0 * 0.85
    } else {
        size
    };
    paint(
        ui.painter(),
        Rect::from_center_size(response.rect.center(), Vec2::splat(glyph_size)),
        icon,
        ink.gamma_multiply(icon_opacity),
    );
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, response.enabled(), label)
    });
    response.on_hover_text(crate::localization::text(ui.ctx(), label))
}

pub(crate) fn layer_control_size(icon: Icon) -> f32 {
    22.0 * if matches!(icon, Icon::Effects) {
        0.85
    } else {
        1.0
    }
}

pub(crate) fn paint(painter: &egui::Painter, rect: Rect, icon: Icon, color: Color32) {
    let p =
        |x: f32, y: f32| rect.min + Vec2::new(x * rect.width() / 18.0, y * rect.height() / 18.0);
    let stroke = Stroke::new(1.4, color);
    let line = |a: (f32, f32), b: (f32, f32)| {
        painter.line_segment([p(a.0, a.1), p(b.0, b.1)], stroke);
    };
    let path = |points: &[(f32, f32)]| {
        painter.add(egui::Shape::line(
            points.iter().map(|&(x, y)| p(x, y)).collect(),
            stroke,
        ));
    };
    match icon {
        Icon::Monitor => {
            path(&[
                (2.0, 2.0),
                (16.0, 2.0),
                (16.0, 12.0),
                (2.0, 12.0),
                (2.0, 2.0),
            ]);
            line((9.0, 12.0), (9.0, 16.0));
            line((5.0, 16.0), (13.0, 16.0));
        }
        Icon::Opacity => {
            painter.circle_stroke(rect.center(), rect.width() / 3.0, stroke);
            let points = std::iter::once(rect.center())
                .chain((0..=16).map(|i| {
                    let a = -std::f32::consts::FRAC_PI_2 + i as f32 * std::f32::consts::PI / 16.0;
                    rect.center() + Vec2::angled(a) * rect.width() / 3.0
                }))
                .collect();
            painter.add(egui::Shape::convex_polygon(points, color, Stroke::NONE));
        }
        Icon::Atlas => {
            path(&[
                (1.0, 1.0),
                (17.0, 1.0),
                (17.0, 17.0),
                (1.0, 17.0),
                (1.0, 1.0),
            ]);
            for offset in [6.0, 12.0] {
                line((offset, 1.0), (offset, 17.0));
                line((1.0, offset), (17.0, offset));
            }
        }
        Icon::Layers => {
            path(&[(1.0, 5.0), (9.0, 1.0), (17.0, 5.0), (9.0, 9.0), (1.0, 5.0)]);
            path(&[(1.0, 9.0), (9.0, 13.0), (17.0, 9.0)]);
            path(&[(1.0, 13.0), (9.0, 17.0), (17.0, 13.0)]);
        }
        Icon::Eye(visible) => {
            let ctx = painter.ctx();
            let id = egui::Id::new("layer-eye-glyph");
            let texture = ctx
                .data(|data| data.get_temp::<egui::TextureHandle>(id))
                .unwrap_or_else(|| {
                    let texture = ctx.load_texture(
                        "layer-eye",
                        egui::ColorImage::from_rgba_unmultiplied(
                            [64, 64],
                            include_bytes!("../assets/toolbar/layer-eye.rgba"),
                        ),
                        egui::TextureOptions::LINEAR
                            .with_mipmap_mode(Some(egui::TextureFilter::Linear)),
                    );
                    ctx.data_mut(|data| data.insert_temp(id, texture.clone()));
                    texture
                });
            painter.image(
                texture.id(),
                rect,
                Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                color,
            );
            if !visible {
                line((3.0, 3.0), (15.0, 15.0));
            }
        }
        Icon::Folder => path(&[
            (1.0, 15.0),
            (1.0, 3.0),
            (7.0, 3.0),
            (9.0, 6.0),
            (17.0, 6.0),
            (17.0, 15.0),
            (1.0, 15.0),
        ]),
        Icon::Chevron(collapsed) => {
            if collapsed {
                path(&[(6.0, 3.0), (12.0, 9.0), (6.0, 15.0)]);
            } else {
                path(&[(3.0, 6.0), (9.0, 12.0), (15.0, 6.0)]);
            }
        }
        Icon::Delete => {
            path(&[(5.0, 5.0), (5.0, 16.0), (13.0, 16.0), (13.0, 5.0)]);
            line((2.0, 4.0), (16.0, 4.0));
            path(&[(6.0, 4.0), (6.0, 1.0), (12.0, 1.0), (12.0, 4.0)]);
        }
        Icon::Copy => {
            path(&[
                (5.0, 5.0),
                (16.0, 5.0),
                (16.0, 16.0),
                (5.0, 16.0),
                (5.0, 5.0),
            ]);
            path(&[(2.0, 12.0), (2.0, 2.0), (12.0, 2.0)]);
        }
        Icon::Paste => {
            path(&[
                (5.0, 4.0),
                (2.0, 4.0),
                (2.0, 17.0),
                (16.0, 17.0),
                (16.0, 4.0),
                (13.0, 4.0),
            ]);
            path(&[(5.0, 1.0), (13.0, 1.0), (13.0, 6.0), (5.0, 6.0), (5.0, 1.0)]);
        }
        Icon::Close => {
            line((3.0, 3.0), (15.0, 15.0));
            line((15.0, 3.0), (3.0, 15.0));
        }
        Icon::Effects => {
            let galley = painter.layout_no_wrap(
                "fx".into(),
                egui::FontId::proportional(rect.height()),
                color,
            );
            // Center the visible glyphs rather than the font's line-height box.
            let position = rect.center() - galley.mesh_bounds.center().to_vec2();
            painter.galley(position, galley, color);
        }
        Icon::Plus => {
            line((3.0, 9.0), (15.0, 9.0));
            line((9.0, 3.0), (9.0, 15.0));
        }
        Icon::Star(active) => {
            let points: Vec<_> = (0..10)
                .map(|i| {
                    let a = (i as f32 * 36.0 - 90.0).to_radians();
                    let r = if i % 2 == 0 { 8.0 } else { 3.6 };
                    p(9.0 + r * a.cos(), 9.0 + r * a.sin())
                })
                .collect();
            if active {
                let mut mesh = egui::Mesh::default();
                mesh.colored_vertex(rect.center(), color);
                for point in &points {
                    mesh.colored_vertex(*point, color);
                }
                for i in 0..10 {
                    mesh.add_triangle(0, i + 1, (i + 1) % 10 + 1);
                }
                painter.add(egui::Shape::mesh(mesh));
            }
            painter.add(egui::Shape::closed_line(points, stroke));
        }
    }
}

pub(crate) fn thumbnail(
    ui: &mut egui::Ui,
    texture: Option<egui::TextureId>,
    kind: &str,
    size: f32,
) -> egui::Response {
    thumbnail_with_sense(ui, texture, kind, size, Sense::click(), None)
}

pub(crate) fn draggable_thumbnail(
    ui: &mut egui::Ui,
    texture: Option<egui::TextureId>,
    kind: &str,
    size: f32,
) -> egui::Response {
    thumbnail_with_sense(ui, texture, kind, size, Sense::click_and_drag(), None)
}

fn thumbnail_with_sense(
    ui: &mut egui::Ui,
    texture: Option<egui::TextureId>,
    kind: &str,
    size: f32,
    sense: Sense,
    id: Option<egui::Id>,
) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(
        Vec2::splat(size),
        if id.is_some() { Sense::hover() } else { sense },
    );
    let response = id.map_or(response, |id| ui.interact(rect, id, sense));
    paint_thumbnail(ui, rect, texture, kind, ui.visuals().text_color());
    if crate::layer_palette::is_adjustment(kind) {
        return response.on_hover_text(crate::localization::text(
            ui.ctx(),
            crate::layer_palette::label(kind),
        ));
    }
    ui.painter().rect_stroke(
        rect,
        0.0,
        Stroke::new(1.0, ui.visuals().widgets.noninteractive.bg_stroke.color),
        egui::StrokeKind::Inside,
    );
    response
}

pub(crate) fn thumbnail_button(
    ui: &mut egui::Ui,
    id: egui::Id,
    texture: Option<egui::TextureId>,
    label: &str,
) -> egui::Response {
    let response = ui
        .scope(|ui| {
            ui.spacing_mut().button_padding = Vec2::ZERO;
            ui.spacing_mut().interact_size = Vec2::splat(CLOSE_BUTTON_SIZE);
            ui.add(
                crate::buttons::Button::new(egui::Atoms::default())
                    .id(id)
                    .bar()
                    .min_size(Vec2::splat(CLOSE_BUTTON_SIZE)),
            )
        })
        .inner;
    let size = response.rect.size().min_elem() - 8.0;
    let (_, ink) = crate::buttons::colors(ui, &response, crate::buttons::ButtonStyle::Quiet);
    paint_thumbnail(
        ui,
        Rect::from_center_size(response.rect.center(), Vec2::splat(size)),
        texture,
        "Icon",
        ink,
    );
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, response.enabled(), label)
    });
    response.on_hover_text(crate::localization::text(ui.ctx(), label))
}

fn paint_thumbnail(
    ui: &egui::Ui,
    rect: Rect,
    texture: Option<egui::TextureId>,
    kind: &str,
    ink: Color32,
) {
    let painter = ui.painter();
    if crate::layer_palette::is_adjustment(kind) {
        if let Some(symbol) = crate::layer_icons::texture(ui.ctx(), kind) {
            painter.image(
                symbol,
                rect.shrink(5.0),
                Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                ink,
            );
        }
        return;
    }
    for y in 0..6 {
        for x in 0..6 {
            let tile = rect.width() / 6.0;
            painter.rect_filled(
                Rect::from_min_size(
                    rect.min + Vec2::new(x as f32 * tile, y as f32 * tile),
                    Vec2::splat(tile),
                ),
                0.0,
                if (x + y) % 2 == 0 {
                    Color32::from_gray(45)
                } else {
                    Color32::from_gray(65)
                },
            );
        }
    }
    if let Some(texture) = texture {
        painter.image(
            texture,
            rect,
            Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
            Color32::WHITE,
        );
    } else {
        paint(
            painter,
            rect.shrink(8.0),
            if kind == "Folder" {
                Icon::Folder
            } else {
                Icon::Layers
            },
            ink,
        );
    }
}
