use crate::effects::apply;
use image::{Rgba, RgbaImage};
use serde_json::json;
use std::cell::RefCell;

thread_local! {
    static CAPTURE: RefCell<Option<String>> = const { RefCell::new(None) };
}
fn capture(line: &str) {
    CAPTURE.with_borrow_mut(|text| {
        if let Some(text) = text {
            text.push_str(line);
        }
    });
}

#[test]
fn effect_diagnostics_cover_stages_failure_cancellation_and_unwinding_without_payloads() {
    achimeld_application::diagnostics::install_sink(capture);
    CAPTURE.with_borrow_mut(|text| *text = Some(String::new()));
    let image = RgbaImage::from_pixel(32, 32, Rgba([255; 4]));
    let layer = json!({"displayName":"PRIVATE_DOCUMENT_TEXT","effects":[
        {"kind":"Outline","width":24,"outlineShape":"Round"},
        {"kind":"OuterLight","width":16,"blur":32}
    ]});
    apply(image.clone(), &layer, 32, 32, &|| false, None).unwrap();
    let invalid = json!({"effects":[{"kind":"PRIVATE_KIND", "colorHex":"PRIVATE_COLOR"}]});
    assert!(apply(image.clone(), &invalid, 32, 32, &|| false, None).is_err());
    let polls = std::cell::Cell::new(0);
    let cancelled = || {
        polls.set(polls.get() + 1);
        polls.get() > 1
    };
    assert!(apply(image.clone(), &layer, 32, 32, &cancelled, None).is_err());
    let result = std::panic::catch_unwind(|| {
        apply(
            image,
            &layer,
            32,
            32,
            &|| false,
            Some(&|_| panic!("PRIVATE_PANIC")),
        )
    });
    assert!(result.is_err());
    let text = CAPTURE.with_borrow_mut(Option::take).unwrap();
    for expected in [
        "kind=Outline",
        "canvas=32x32",
        "outline expand mask",
        "outline erode mask",
        "outline composite",
        "kind=OuterLight",
        "outer glow padded mask",
        "outer glow expand alpha",
        "outer glow blur",
        "outer glow composite",
        "kind=Unknown",
        "render effect: completed",
        "render effect: failed",
        "render effect: cancelled",
        "vector outline rasterization",
        "render effect: interrupted",
    ] {
        assert!(text.contains(expected), "Missing {expected}: {text}");
    }
    assert!(!text.contains("PRIVATE_"));
}
