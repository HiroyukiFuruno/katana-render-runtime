use super::super::{BlendMode, Color, Group, NonZeroF32, Opacity, PositiveF32, filter as f};

#[test]
fn single_input_kinds_match_only_their_input() -> Result<(), String> {
    let wanted = f::Input::Reference("wanted".to_owned());
    let absent = f::Input::Reference("absent".to_owned());
    let mut single = color_input_kinds(&wanted);
    single.extend(convolution_and_shadow_kinds(&wanted)?);
    single.extend(lighting_kinds(&wanted));
    single.extend(geometry_kinds(&wanted)?);
    for kind in single {
        assert_inputs(&kind, &[(&wanted, true), (&absent, false)]);
    }
    Ok(())
}

#[test]
fn two_input_kinds_match_either_input() {
    let wanted = f::Input::Reference("wanted".to_owned());
    let other = f::Input::Reference("other".to_owned());
    let absent = f::Input::Reference("absent".to_owned());
    for kind in two_input_kinds(&wanted, &other) {
        assert_inputs(&kind, &[(&wanted, true), (&other, true), (&absent, false)]);
    }
}

#[test]
fn merge_matches_nonfirst_input_and_rejects_absent_or_empty() {
    let wanted = f::Input::Reference("wanted".to_owned());
    let other = f::Input::Reference("other".to_owned());
    let absent = f::Input::Reference("absent".to_owned());
    let merge = f::Kind::Merge(f::Merge {
        inputs: vec![other, wanted.clone()],
    });
    assert_inputs(&merge, &[(&wanted, true), (&absent, false)]);
    assert_inputs(
        &f::Kind::Merge(f::Merge { inputs: Vec::new() }),
        &[(&wanted, false)],
    );
}

#[test]
fn inputless_kinds_reject_inputs() -> Result<(), String> {
    let wanted = f::Input::Reference("wanted".to_owned());
    for kind in inputless_kinds()? {
        assert_inputs(
            &kind,
            &[(&wanted, false), (&f::Input::SourceGraphic, false)],
        );
    }
    Ok(())
}

fn assert_inputs(kind: &f::Kind, cases: &[(&f::Input, bool)]) {
    for (input, expected) in cases {
        assert_eq!(kind.has_input(input), *expected, "{kind:?}, {input:?}");
    }
}

fn positive() -> Result<PositiveF32, String> {
    PositiveF32::new(1.0).ok_or_else(|| "positive value unavailable".to_owned())
}

fn color_input_kinds(input: &f::Input) -> Vec<f::Kind> {
    vec![
        f::Kind::ColorMatrix(f::ColorMatrix {
            input: input.clone(),
            kind: f::ColorMatrixKind::default(),
        }),
        f::Kind::ComponentTransfer(f::ComponentTransfer {
            input: input.clone(),
            func_r: f::TransferFunction::Identity,
            func_g: f::TransferFunction::Identity,
            func_b: f::TransferFunction::Identity,
            func_a: f::TransferFunction::Identity,
        }),
    ]
}

fn convolution_and_shadow_kinds(input: &f::Input) -> Result<Vec<f::Kind>, String> {
    let p = positive()?;
    let matrix = f::ConvolveMatrixData::new(0, 0, 1, 1, vec![1.0]).ok_or("matrix invalid")?;
    let divisor = NonZeroF32::new(1.0).ok_or("divisor invalid")?;
    Ok(vec![
        f::Kind::ConvolveMatrix(f::ConvolveMatrix {
            input: input.clone(),
            matrix,
            divisor,
            bias: 0.0,
            edge_mode: f::EdgeMode::None,
            preserve_alpha: false,
        }),
        f::Kind::DropShadow(f::DropShadow {
            input: input.clone(),
            dx: 0.0,
            dy: 0.0,
            std_dev_x: p,
            std_dev_y: p,
            color: Color::black(),
            opacity: Opacity::ONE,
        }),
        f::Kind::GaussianBlur(f::GaussianBlur {
            input: input.clone(),
            std_dev_x: p,
            std_dev_y: p,
        }),
    ])
}

fn lighting_kinds(input: &f::Input) -> Vec<f::Kind> {
    let light = f::LightSource::DistantLight(f::DistantLight {
        azimuth: 0.0,
        elevation: 0.0,
    });
    vec![
        f::Kind::DiffuseLighting(f::DiffuseLighting {
            input: input.clone(),
            surface_scale: 1.0,
            diffuse_constant: 1.0,
            lighting_color: Color::black(),
            light_source: light,
        }),
        f::Kind::SpecularLighting(f::SpecularLighting {
            input: input.clone(),
            surface_scale: 1.0,
            specular_constant: 1.0,
            specular_exponent: 1.0,
            lighting_color: Color::black(),
            light_source: light,
        }),
    ]
}

fn geometry_kinds(input: &f::Input) -> Result<Vec<f::Kind>, String> {
    let p = positive()?;
    Ok(vec![
        f::Kind::Morphology(f::Morphology {
            input: input.clone(),
            operator: f::MorphologyOperator::Erode,
            radius_x: p,
            radius_y: p,
        }),
        f::Kind::Offset(f::Offset {
            input: input.clone(),
            dx: 0.0,
            dy: 0.0,
        }),
        f::Kind::Tile(f::Tile {
            input: input.clone(),
        }),
    ])
}

fn two_input_kinds(first: &f::Input, second: &f::Input) -> Vec<f::Kind> {
    vec![
        f::Kind::Blend(f::Blend {
            input1: first.clone(),
            input2: second.clone(),
            mode: BlendMode::Normal,
        }),
        f::Kind::Composite(f::Composite {
            input1: first.clone(),
            input2: second.clone(),
            operator: f::CompositeOperator::Over,
        }),
        f::Kind::DisplacementMap(f::DisplacementMap {
            input1: first.clone(),
            input2: second.clone(),
            scale: 1.0,
            x_channel_selector: f::ColorChannel::R,
            y_channel_selector: f::ColorChannel::G,
        }),
    ]
}

fn inputless_kinds() -> Result<Vec<f::Kind>, String> {
    let p = positive()?;
    Ok(vec![
        f::Kind::Flood(f::Flood {
            color: Color::black(),
            opacity: Opacity::ONE,
        }),
        f::Kind::Image(f::Image {
            root: Group::empty(),
        }),
        f::Kind::Turbulence(f::Turbulence {
            base_frequency_x: p,
            base_frequency_y: p,
            num_octaves: 1,
            seed: 0,
            stitch_tiles: false,
            kind: f::TurbulenceKind::Turbulence,
        }),
    ])
}

#[test]
fn convolve_matrix_data_preserves_order_and_rejects_invalid_dimensions() -> Result<(), String> {
    let matrix = f::ConvolveMatrixData::new(1, 0, 2, 2, vec![1.0, 2.0, 3.0, 4.0])
        .ok_or("valid matrix rejected")?;
    assert_eq!(matrix.data(), &[1.0, 2.0, 3.0, 4.0]);
    assert_eq!(matrix.get(1, 1), 4.0);
    assert!(f::ConvolveMatrixData::new(0, 0, 2, 2, vec![1.0]).is_none());
    assert!(f::ConvolveMatrixData::new(2, 0, 2, 2, vec![1.0; 4]).is_none());
    assert!(f::ConvolveMatrixData::new(0, 2, 2, 2, vec![1.0; 4]).is_none());
    Ok(())
}
