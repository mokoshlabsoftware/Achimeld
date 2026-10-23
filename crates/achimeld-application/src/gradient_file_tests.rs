use super::*;

// Independent descriptor fixture: explicit-length IDs, unterminated Unicode names,
// reverse key order, a separate opacity knot and a non-central color midpoint.
fn fixture() -> Vec<u8> {
    fn identifier(s: &str) -> Vec<u8> {
        [(s.len() as u32).to_be_bytes().as_slice(), s.as_bytes()].concat()
    }
    fn obj(class: &str, fields: Vec<(&str, Vec<u8>)>) -> Vec<u8> {
        let mut b = [
            b"Objc".as_slice(),
            &0_u32.to_be_bytes(),
            &identifier(class),
            &(fields.len() as u32).to_be_bytes(),
        ]
        .concat();
        for (name, value) in fields {
            b.extend(identifier(name));
            b.extend(value);
        }
        b
    }
    fn list(items: Vec<Vec<u8>>) -> Vec<u8> {
        [
            b"VlLs".as_slice(),
            &(items.len() as u32).to_be_bytes(),
            &items.concat(),
        ]
        .concat()
    }
    fn int(n: u32) -> Vec<u8> {
        [b"long".as_slice(), &n.to_be_bytes()].concat()
    }
    fn double(n: f64) -> Vec<u8> {
        [b"doub".as_slice(), &n.to_be_bytes()].concat()
    }
    fn en(kind: &str, value: &str) -> Vec<u8> {
        [b"enum".as_slice(), &identifier(kind), &identifier(value)].concat()
    }
    let colors = [0.0, 255.0]
        .into_iter()
        .enumerate()
        .map(|(i, c)| {
            obj(
                "Clrt",
                vec![
                    ("Mdpn", int(if i == 0 { 25 } else { 50 })),
                    ("Lctn", int(i as u32 * 4096)),
                    ("Type", en("Clry", "UsrS")),
                    (
                        "Clr ",
                        obj(
                            "RGBC",
                            vec![
                                ("Bl  ", double(c)),
                                ("Grn ", double(c)),
                                ("Rd  ", double(c)),
                            ],
                        ),
                    ),
                ],
            )
        })
        .collect();
    let transparency = [(0, 0.0_f64), (2048, 100.0), (4096, 100.0)]
        .map(|(position, percent)| {
            obj(
                "TrnS",
                vec![
                    ("Mdpn", int(50)),
                    ("Lctn", int(position)),
                    (
                        "Opct",
                        [b"UntF#Prc".as_slice(), &percent.to_be_bytes()].concat(),
                    ),
                ],
            )
        })
        .to_vec();
    let name = "Świt 🌅"
        .encode_utf16()
        .flat_map(u16::to_be_bytes)
        .collect::<Vec<_>>();
    let gradient = obj(
        "Grdn",
        vec![
            ("Trns", list(transparency)),
            ("Clrs", list(colors)),
            ("Intr", double(4096.0)),
            ("GrdF", en("GrdF", "CstS")),
            (
                "Nm  ",
                [
                    b"TEXT".as_slice(),
                    &((name.len() / 2) as u32).to_be_bytes(),
                    &name,
                ]
                .concat(),
            ),
        ],
    );
    let root = obj(
        "null",
        vec![("GrdL", list(vec![obj("Grdn", vec![("Grad", gradient)])]))],
    );
    [b"8BGR\0\x05\0\0\0\x10".as_slice(), &root[4..]].concat()
}

#[test]
fn photoshop_gradient_fixture_merges_opacity_midpoint_and_unicode() {
    let gradients = decode(&fixture()).unwrap();
    assert_eq!(gradients.len(), 1);
    assert_eq!(gradients[0].name, "Świt 🌅");
    assert_eq!(
        gradients[0]
            .stops
            .iter()
            .map(|s| s.offset)
            .collect::<Vec<_>>(),
        [0.0, 0.25, 0.5, 1.0]
    );
    assert_eq!(
        decode_color(&gradients[0].stops[1].color_hex),
        Some([128, 128, 128, 128])
    );
    assert_eq!(
        decode_color(&gradients[0].stops[2].color_hex),
        Some([170, 170, 170, 255])
    );
}

#[test]
fn photoshop_gradient_export_round_trip_retains_names_alpha_and_hard_edges() {
    let preset = GradientPreset {
        id: new_id(),
        name: "Sunset 🌄".into(),
        stops: [
            (0.0, [1, 2, 3, 0]),
            (0.5, [40, 50, 60, 128]),
            (0.5, [200, 150, 70, 255]),
            (1.0, [255; 4]),
        ]
        .into_iter()
        .map(|(offset, color)| GradientStop {
            offset,
            color_hex: encode_color(color),
        })
        .collect(),
    };
    let bytes = encode(&[preset.clone(), preset.clone()]).unwrap();
    assert_eq!(&bytes[..10], b"8BGR\0\x05\0\0\0\x10");
    let decoded = decode(&bytes).unwrap();
    assert_eq!(decoded.len(), 2);
    for gradient in decoded {
        assert_eq!(gradient.name, preset.name);
        assert_eq!(gradient.stops, preset.stops);
    }
}

#[test]
fn photoshop_gradient_rejects_truncation_unsupported_features_and_resource_bombs() {
    let valid = fixture();
    for n in 0..valid.len() {
        assert!(
            decode(&valid[..n]).is_err(),
            "Accepted truncated length {n}"
        );
    }
    for (from, to) in [(b"CstS", b"ClNs"), (b"RGBC", b"CMYC"), (b"UsrS", b"FrgC")] {
        let mut bad = valid.clone();
        let index = bad.windows(4).position(|w| w == from).unwrap();
        bad[index..index + 4].copy_from_slice(to);
        assert!(decode(&bad).is_err());
    }
    assert!(decode(&vec![0; MAX_BYTES + 1]).is_err());
    let mut bad = valid.clone();
    bad[10..14].copy_from_slice(&u32::MAX.to_be_bytes());
    assert!(decode(&bad).is_err());
    let mut bad = valid;
    bad.extend_from_slice(b"garbage");
    assert!(decode(&bad).is_err());
    assert!(encode(&[]).is_err());
}
