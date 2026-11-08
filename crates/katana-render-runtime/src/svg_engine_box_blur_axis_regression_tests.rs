use super::{ImageRefMut, box_blur_horz, box_blur_vert};
use rgb::RGBA8;
use rstest::rstest;

const CHANNELS: usize = 4;
type Pixel = [u8; CHANNELS];

const ONE: &[Pixel] = &[[210, 120, 60, 240]];
const ONE_RADIUS_1: &[Pixel] = &[[70, 40, 20, 80]];
const ONE_RADIUS_2: &[Pixel] = &[[42, 24, 12, 48]];
const LINE: &[Pixel] = &[[90, 45, 0, 180], [180, 90, 30, 210], [0, 45, 90, 120]];
const LINE_RADIUS_1: &[Pixel] = &[[90, 45, 10, 130], [90, 60, 40, 170], [60, 45, 40, 110]];
const SINGLE_RADIUS_1: &[Pixel] = &[[30, 15, 0, 60], [60, 30, 10, 70], [0, 15, 30, 40]];
const LINE_RADIUS_2: &[Pixel] = &[[54, 36, 24, 102]; 3];
const SINGLE_RADIUS_2: &[Pixel] = &[[18, 9, 0, 36], [36, 18, 6, 42], [0, 9, 18, 24]];
const GRID: &[Pixel] = &[
    [90, 45, 0, 180], [180, 90, 30, 210], [0, 45, 90, 120],
    [30, 60, 120, 150], [60, 120, 30, 180], [120, 0, 60, 150],
];
const GRID_HORZ: &[Pixel] = &[
    [90, 45, 10, 130], [90, 60, 40, 170], [60, 45, 40, 110],
    [30, 60, 50, 110], [70, 60, 70, 160], [60, 40, 30, 110],
];
const GRID_VERT: &[Pixel] = &[
    [40, 35, 40, 110], [80, 70, 20, 130], [40, 15, 50, 90],
    [40, 35, 40, 110], [80, 70, 20, 130], [40, 15, 50, 90],
];

fn pixels(values: &[Pixel]) -> Vec<RGBA8> {
    values.iter().map(|&[r, g, b, a]| {
        assert!(r <= a && g <= a && b <= a);
        RGBA8 { r, g, b, a }
    }).collect()
}

fn run_axis(width: u32, height: u32, radius: usize, input: &[Pixel], vertical: bool) -> Vec<RGBA8> {
    let mut source = pixels(input);
    let mut output = vec![RGBA8::default(); source.len()];
    let source = ImageRefMut::new(width, height, &mut source);
    let mut output_view = ImageRefMut::new(width, height, &mut output);
    if vertical {
        box_blur_vert(radius, &source, &mut output_view);
    } else {
        box_blur_horz(radius, &source, &mut output_view);
    }
    output
}

#[rstest]
#[case(1, 1, 0, ONE, ONE, ONE)]
#[case(1, 1, 1, ONE, ONE_RADIUS_1, ONE_RADIUS_1)]
#[case(1, 1, 2, ONE, ONE_RADIUS_2, ONE_RADIUS_2)]
#[case(3, 1, 1, LINE, LINE_RADIUS_1, SINGLE_RADIUS_1)]
#[case(1, 3, 1, LINE, SINGLE_RADIUS_1, LINE_RADIUS_1)]
#[case(3, 1, 2, LINE, LINE_RADIUS_2, SINGLE_RADIUS_2)]
#[case(1, 3, 2, LINE, SINGLE_RADIUS_2, LINE_RADIUS_2)]
#[case(3, 2, 0, GRID, GRID, GRID)]
#[case(3, 2, 1, GRID, GRID_HORZ, GRID_VERT)]
fn zero_extended_blur_by_axis(
    #[case] width: u32,
    #[case] height: u32,
    #[case] radius: usize,
    #[case] input: &[Pixel],
    #[case] horizontal: &[Pixel],
    #[case] vertical: &[Pixel],
) {
    assert_eq!(run_axis(width, height, radius, input, false), pixels(horizontal));
    assert_eq!(run_axis(width, height, radius, input, true), pixels(vertical));
}
