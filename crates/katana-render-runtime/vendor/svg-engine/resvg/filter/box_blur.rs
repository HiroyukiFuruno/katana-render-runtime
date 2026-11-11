// Copyright 2020 the Resvg Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

// Based on https://github.com/fschutt/fastblur

#![allow(clippy::needless_range_loop)]

use super::ImageRefMut;
use rgb::RGBA8;
use std::cmp;

const STEPS: usize = 5;

/// Applies a box blur.
///
/// Input image pixels should have a **premultiplied alpha**.
///
/// A negative or zero `sigma_x`/`sigma_y` will disable the blur along that axis.
///
/// # Allocations
///
/// This method will allocate a copy of the `src` image as a back buffer.
pub fn apply(sigma_x: f64, sigma_y: f64, mut src: ImageRefMut) {
    let boxes_horz = create_box_gauss(sigma_x as f32);
    let boxes_vert = create_box_gauss(sigma_y as f32);
    let mut backbuf = src.data.to_vec();
    let mut backbuf = ImageRefMut::new(src.width, src.height, &mut backbuf);

    for (box_size_horz, box_size_vert) in boxes_horz.iter().zip(boxes_vert.iter()) {
        let radius_horz = ((box_size_horz - 1) / 2) as usize;
        let radius_vert = ((box_size_vert - 1) / 2) as usize;
        box_blur_impl(radius_horz, radius_vert, &mut backbuf, &mut src);
    }
}

#[inline(never)]
fn create_box_gauss(sigma: f32) -> [i32; STEPS] {
    if sigma > 0.0 {
        let n_float = STEPS as f32;

        // Ideal averaging filter width
        let w_ideal = (12.0 * sigma * sigma / n_float).sqrt() + 1.0;
        let mut wl = w_ideal.floor() as i32;
        if wl % 2 == 0 {
            wl -= 1;
        }

        let wu = wl + 2;

        let wl_float = wl as f32;
        let m_ideal = (12.0 * sigma * sigma
            - n_float * wl_float * wl_float
            - 4.0 * n_float * wl_float
            - 3.0 * n_float)
            / (-4.0 * wl_float - 4.0);
        let m = m_ideal.round() as usize;

        box_sizes(wl, wu, m)
    } else {
        [1; STEPS]
    }
}

fn box_sizes(wl: i32, wu: i32, m: usize) -> [i32; STEPS] {
    let mut sizes = [0; STEPS];
    for i in 0..STEPS {
        if i < m {
            sizes[i] = wl;
        } else {
            sizes[i] = wu;
        }
    }
    sizes
}

#[inline]
fn box_blur_impl(
    blur_radius_horz: usize,
    blur_radius_vert: usize,
    backbuf: &mut ImageRefMut,
    frontbuf: &mut ImageRefMut,
) {
    box_blur_vert(blur_radius_vert, frontbuf, backbuf);
    box_blur_horz(blur_radius_horz, backbuf, frontbuf);
}

#[inline]
fn box_blur_vert(blur_radius: usize, backbuf: &ImageRefMut, frontbuf: &mut ImageRefMut) {
    if blur_radius == 0 {
        frontbuf.data.copy_from_slice(backbuf.data);
        return;
    }
    let width = backbuf.width as usize;
    let height = backbuf.height as usize;
    let spec = BlurLineSpec::new(height, width, blur_radius);
    for i in 0..width {
        let start = i;
        let end = i + width * (height - 1);
        blur_line(&spec, start, end, backbuf, frontbuf);
    }
}

#[inline]
fn box_blur_horz(blur_radius: usize, backbuf: &ImageRefMut, frontbuf: &mut ImageRefMut) {
    if blur_radius == 0 {
        frontbuf.data.copy_from_slice(backbuf.data);
        return;
    }
    let width = backbuf.width as usize;
    let height = backbuf.height as usize;
    let spec = BlurLineSpec::new(width, 1, blur_radius);
    for i in 0..height {
        let start = i * width;
        let end = (i + 1) * width - 1;
        blur_line(&spec, start, end, backbuf, frontbuf);
    }
}

struct BlurLineSpec {
    length: usize,
    step: usize,
    radius: usize,
    inverse: f32,
    radius_prev: isize,
    radius_next: isize,
}

impl BlurLineSpec {
    fn new(length: usize, step: usize, radius: usize) -> Self {
        let inverse = 1.0 / (radius + radius + 1) as f32;
        let radius_prev = radius as isize - length as isize;
        let radius_next = radius as isize + 1;
        Self { length, step, radius, inverse, radius_prev, radius_next }
    }
}

struct BlurLine {
    last_index: usize,
    target: usize,
    left: usize,
    right: usize,
    first: RGBA8,
    last: RGBA8,
    red: isize,
    green: isize,
    blue: isize,
    alpha: isize,
}

impl BlurLine {
    fn new(spec: &BlurLineSpec, first_index: usize, last_index: usize) -> Self {
        let target = first_index;
        let left = target;
        let right = target + spec.radius * spec.step;
        let first = RGBA8::default();
        let last = RGBA8::default();
        let red = spec.radius_next * (first.r as isize);
        let green = spec.radius_next * (first.g as isize);
        let blue = spec.radius_next * (first.b as isize);
        let alpha = spec.radius_next * (first.a as isize);
        Self { last_index, target, left, right, first, last, red, green, blue, alpha }
    }

    fn write(&mut self, added: RGBA8, removed: RGBA8, spec: &BlurLineSpec, frontbuf: &mut ImageRefMut) {
        self.red += sub(added.r, removed.r);
        self.green += sub(added.g, removed.g);
        self.blue += sub(added.b, removed.b);
        self.alpha += sub(added.a, removed.a);
        frontbuf.data[self.target] = RGBA8 {
            r: round(self.red as f32 * spec.inverse) as u8,
            g: round(self.green as f32 * spec.inverse) as u8,
            b: round(self.blue as f32 * spec.inverse) as u8,
            a: round(self.alpha as f32 * spec.inverse) as u8,
        };
        self.target += spec.step;
    }
}

fn blur_line(spec: &BlurLineSpec, start: usize, end: usize, backbuf: &ImageRefMut, frontbuf: &mut ImageRefMut) {
    let mut line = BlurLine::new(spec, start, end);
    seed_blur_line(&mut line, spec, backbuf);
    blur_leading_edge(&mut line, spec, backbuf, frontbuf);
    if spec.length <= spec.radius {
        return;
    }
    blur_middle(&mut line, spec, backbuf, frontbuf);
    blur_trailing_edge(&mut line, spec, backbuf, frontbuf);
}

fn seed_blur_line(line: &mut BlurLine, spec: &BlurLineSpec, backbuf: &ImageRefMut) {
    for j in 0..cmp::min(spec.radius, spec.length) {
        let bb = backbuf.data[line.target + j * spec.step];
        line.red += bb.r as isize;
        line.green += bb.g as isize;
        line.blue += bb.b as isize;
        line.alpha += bb.a as isize;
    }
    if spec.radius > spec.length {
        line.red += spec.radius_prev * (line.last.r as isize);
        line.green += spec.radius_prev * (line.last.g as isize);
        line.blue += spec.radius_prev * (line.last.b as isize);
        line.alpha += spec.radius_prev * (line.last.a as isize);
    }
}

fn blur_leading_edge(line: &mut BlurLine, spec: &BlurLineSpec, backbuf: &ImageRefMut, frontbuf: &mut ImageRefMut) {
    for _ in 0..cmp::min(spec.length, spec.radius + 1) {
        let bb = if line.right > line.last_index { line.last } else { backbuf.data[line.right] };
        line.right += spec.step;
        line.write(bb, line.first, spec, frontbuf);
    }
}

fn blur_middle(line: &mut BlurLine, spec: &BlurLineSpec, backbuf: &ImageRefMut, frontbuf: &mut ImageRefMut) {
    for _ in (spec.radius + 1)..(spec.length - spec.radius) {
        let bb1 = backbuf.data[line.right];
        line.right += spec.step;
        let bb2 = backbuf.data[line.left];
        line.left += spec.step;
        line.write(bb1, bb2, spec, frontbuf);
    }
}

fn blur_trailing_edge(line: &mut BlurLine, spec: &BlurLineSpec, backbuf: &ImageRefMut, frontbuf: &mut ImageRefMut) {
    for _ in 0..cmp::min(spec.length - spec.radius - 1, spec.radius) {
        let bb = backbuf.data[line.left];
        line.left += spec.step;
        line.write(line.last, bb, spec, frontbuf);
    }
}

/// Fast rounding for x <= 2^23.
/// This is orders of magnitude faster than built-in rounding intrinsic.
///
/// Source: https://stackoverflow.com/a/42386149/585725
#[inline]
fn round(mut x: f32) -> f32 {
    x += 12582912.0;
    x -= 12582912.0;
    x
}

#[inline]
fn sub(c1: u8, c2: u8) -> isize {
    c1 as isize - c2 as isize
}

#[cfg(test)]
crate::svg_engine::owned_svg_model_tests!("../../../../src/svg_engine_box_blur_axis_regression_tests.rs");
