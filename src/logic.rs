use base64::prelude::*;
use image::Luma;
use image::{GrayImage, ImageReader};

use std::f32::consts::PI;
use std::fs;
use std::fs::File;
use std::io::Write;

pub const IMG_DIR: &str = "src/imgs";
pub const IMG_MASK_DIR: &str = "src/imgs/masks/";

// cutoffs, each halfway between the measured groups
// fill ratio: circle ~0.92+, square/cross ~0.64, triangle ~0.42
pub const CIRCLE_MIN_FILL: f32 = 0.8;
pub const TRIANGLE_MAX_FILL: f32 = 0.53;
// edge ratio: square ~0.71, cross ~0.45
pub const SQUARE_MIN_EDGE: f32 = 0.58;

// how far the opening reaches; lines thinner than 2*r+1 px get removed
pub const OPEN_RADIUS: i32 = 1;

pub fn img_clean(gi: &GrayImage) -> (GrayImage, Vec<(u32, u32)>) {
    let h = gi.height();
    let w = gi.width();

    let mut visited = vec![false; (w * h) as usize];
    let mut best: Vec<(u32, u32)> = Vec::new();

    let mut cleaned = GrayImage::new(w, h);

    for y in 0..h {
        for x in 0..w {
            if visited[(y * w + x) as usize] {
                continue;
            }
            if gi.get_pixel(x, y).0[0] != 255 {
                continue;
            }

            let mut stack: Vec<(u32, u32)> = Vec::new();
            let mut blob: Vec<(u32, u32)> = Vec::new();

            stack.push((x, y));
            visited[(y * w + x) as usize] = true;

            while let Some((cx, cy)) = stack.pop() {
                blob.push((cx, cy));

                // left
                if cx > 0 {
                    let (nx, ny) = (cx - 1, cy);
                    if gi.get_pixel(nx, ny).0[0] == 255 && !visited[(ny * w + nx) as usize] {
                        visited[(ny * w + nx) as usize] = true;
                        stack.push((nx, ny));
                    }
                }

                // right
                if cx + 1 < w {
                    let (nx, ny) = (cx + 1, cy);
                    if gi.get_pixel(nx, ny).0[0] == 255 && !visited[(ny * w + nx) as usize] {
                        visited[(ny * w + nx) as usize] = true;
                        stack.push((nx, ny));
                    }
                }

                // up
                if cy > 0 {
                    let (nx, ny) = (cx, cy - 1);
                    if gi.get_pixel(nx, ny).0[0] == 255 && !visited[(ny * w + nx) as usize] {
                        visited[(ny * w + nx) as usize] = true;
                        stack.push((nx, ny));
                    }
                }

                // down
                if cy + 1 < h {
                    let (nx, ny) = (cx, cy + 1);
                    if gi.get_pixel(nx, ny).0[0] == 255 && !visited[(ny * w + nx) as usize] {
                        visited[(ny * w + nx) as usize] = true;
                        stack.push((nx, ny));
                    }
                }
            }

            if blob.len() > best.len() {
                best = blob
            }
        }
    }

    for &(b1, b2) in &best {
        cleaned.put_pixel(b1, b2, Luma([255]));
    }

    (cleaned, best)
}

// one pass of erode (keep_if_all = true) or dilate (keep_if_all = false):
// a pixel becomes white if all / any of the pixels within `r` of it are white
fn morph(gi: &GrayImage, r: i32, keep_if_all: bool) -> GrayImage {
    let (w, h) = gi.dimensions();
    let mut out = GrayImage::new(w, h);

    for y in 0..h {
        for x in 0..w {
            let mut all_white = true;
            let mut any_white = false;

            for dy in -r..=r {
                for dx in -r..=r {
                    let nx = x as i32 + dx;
                    let ny = y as i32 + dy;

                    // outside the image counts as black
                    let white = nx >= 0
                        && ny >= 0
                        && nx < w as i32
                        && ny < h as i32
                        && gi.get_pixel(nx as u32, ny as u32).0[0] == 255;

                    all_white &= white;
                    any_white |= white;
                }
            }

            if (keep_if_all && all_white) || (!keep_if_all && any_white) {
                out.put_pixel(x, y, Luma([255]));
            }
        }
    }

    out
}

// erode then dilate: thin noise lines vanish, solid shapes come back the same size
pub fn open_mask(gi: &GrayImage, r: i32) -> GrayImage {
    morph(&morph(gi, r, true), r, false)
}

// average position of the shape's pixels
fn centroid(best: &[(u32, u32)]) -> (f32, f32) {
    let mut sum_x: f32 = 0.0;
    let mut sum_y: f32 = 0.0;

    for &(x, y) in best {
        sum_x += x as f32;
        sum_y += y as f32;
    }

    (sum_x / best.len() as f32, sum_y / best.len() as f32)
}

// distance from the centre to the farthest shape pixel
fn farthest(best: &[(u32, u32)], cx: f32, cy: f32) -> f32 {
    let mut far: f32 = 0.0;

    for &(x, y) in best {
        let dx = x as f32 - cx;
        let dy = y as f32 - cy;

        let dist = (dx * dx + dy * dy).sqrt();

        if dist > far {
            far = dist;
        }
    }

    far
}

pub fn fill_ratio(best: &[(u32, u32)]) -> f32 {
    // no shape found, avoid dividing by zero
    if best.is_empty() {
        return 0.0;
    }

    let (cx, cy) = centroid(best);
    let far = farthest(best, cx, cy);

    best.len() as f32 / (PI * far * far)
}

// nearest background pixel divided by farthest shape pixel, both from the centre.
// a cross has deep notches near its centre so this comes out low; a square doesn't.
pub fn edge_ratio(cleaned: &GrayImage, best: &[(u32, u32)]) -> f32 {
    if best.is_empty() {
        return 0.0;
    }

    let (cx, cy) = centroid(best);
    let far = farthest(best, cx, cy);

    let mut near = f32::MAX;

    for (x, y, pixel) in cleaned.enumerate_pixels() {
        if pixel.0[0] == 255 {
            continue;
        }

        let dx = x as f32 - cx;
        let dy = y as f32 - cy;

        let dist = (dx * dx + dy * dy).sqrt();

        if dist < near {
            near = dist;
        }
    }

    near / far
}

pub fn classify(fill: f32, edge: f32) -> &'static str {
    if fill >= CIRCLE_MIN_FILL {
        "circle"
    } else if fill <= TRIANGLE_MAX_FILL {
        "triangle"
    } else if edge >= SQUARE_MIN_EDGE {
        "square"
    } else {
        "cross"
    }
}

// returns (image index, shape name) for every image, sorted by index
pub fn pick_img() -> Result<Vec<(usize, &'static str)>, Box<dyn std::error::Error>> {
    let mut results: Vec<(usize, &'static str)> = Vec::new();

    for entry in fs::read_dir(IMG_DIR)? {
        let entry = entry?;
        let path = entry.path();

        if !path.is_file() {
            continue;
        }

        // "3.png" -> 3, skip anything that isn't named with a number
        let Some(index) = path
            .file_stem()
            .and_then(|s| s.to_str())
            .and_then(|s| s.parse::<usize>().ok())
        else {
            continue;
        };

        let img = ImageReader::open(&path)?.decode()?;

        let grip_pix = img.to_rgb8();

        let width = img.width();
        let height = img.height();

        let mut mask_hold = GrayImage::new(width, height);

        for y in 0..height {
            for x in 0..width {
                let pix = grip_pix.get_pixel(x, y).0;

                if pix.iter().all(|c| *c <= 130) {
                    mask_hold.put_pixel(x, y, Luma([255]));
                }
            }
        }

        let opened = open_mask(&mask_hold, OPEN_RADIUS);
        let (cleaned, best) = img_clean(&opened);
        cleaned.save(format!(
            "{IMG_MASK_DIR}mask_{}",
            entry.file_name().to_string_lossy()
        ))?;

        let fill = fill_ratio(&best);
        let edge = edge_ratio(&cleaned, &best);
        let shape = classify(fill, edge);

        println!("  {index}: fill {fill:.2}, edge {edge:.2} -> {shape}");
        results.push((index, shape));
    }

    // read_dir order is random, so sort by index to get 0..8
    results.sort_by_key(|&(index, _)| index);

    Ok(results)
}

pub fn dcode(b64: &str, i: usize) {
    let bytes = BASE64_STANDARD.decode(b64).expect("valid base64");

    let mut file = File::create(format!("{IMG_DIR}/{i}.png")).expect("failed to create file");
    file.write_all(&bytes)
        .expect("failed to write bytes to file");
}
