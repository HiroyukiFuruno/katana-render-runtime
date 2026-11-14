// Copyright 2020 the Resvg Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use super::{ImageRef, ImageRefMut};
use crate::svg_engine::usvg::filter::{ColorChannel, DisplacementMap};
use rgb::RGBA8;

/// Applies a displacement map.
///
/// - `map` pixels should have a **unpremultiplied alpha**.
/// - `src` pixels can have any alpha method.
///
/// `scale_x` and `scale_y` include the filter scale and canvas transform.
///
/// `src`, `map` and `dest` dimensions must match.
pub fn apply(
    fe: &DisplacementMap,
    scale_x: f32,
    scale_y: f32,
    src: ImageRef,
    map: ImageRef,
    dest: ImageRefMut,
) {
    debug_assert!(src.width == map.width && src.width == dest.width);
    debug_assert!(src.height == map.height && src.height == dest.height);

    let w = src.width as i32;
    let h = src.height as i32;

    let mut x: u32 = 0;
    let mut y: u32 = 0;
    for pixel in map.data.iter() {
        let dx = channel_offset(pixel, fe.x_channel_selector());
        let dy = channel_offset(pixel, fe.y_channel_selector());
        let ox = (x as f32 + dx * scale_x).round() as i32;
        let oy = (y as f32 + dy * scale_y).round() as i32;

        // TODO: we should use some kind of anti-aliasing when offset is on a pixel border

        if x < w as u32 && y < h as u32 && ox >= 0 && ox < w && oy >= 0 && oy < h {
            let idx = (oy * w + ox) as usize;
            let idx1 = (y * w as u32 + x) as usize;
            dest.data[idx1] = src.data[idx];
        }

        x += 1;
        if x == src.width {
            x = 0;
            y += 1;
        }
    }
}

fn channel_offset(pixel: &RGBA8, channel: ColorChannel) -> f32 {
    let c = match channel {
        ColorChannel::B => pixel.b,
        ColorChannel::G => pixel.g,
        ColorChannel::R => pixel.r,
        ColorChannel::A => pixel.a,
    };
    c as f32 / 255.0 - 0.5
}
