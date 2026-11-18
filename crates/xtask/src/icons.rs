//! Deterministic app-icon generator: renders the `{ }` mark and encodes PNG + ICO with `std` only.

#![allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss
)]

use anyhow::{Context, Result};
use std::path::Path;

const ICO_SIZES: [u32; 7] = [16, 24, 32, 48, 64, 128, 256];
const PNG_SIGNATURE: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];

type Rgb = [f32; 3];
type Point = (f32, f32);

/// Encoded icon artifacts, ready to be written to `assets/icons/`.
pub struct Assets {
    pub ico: Vec<u8>,
    pub png_256: Vec<u8>,
    pub png_512: Vec<u8>,
}

fn rgb(hex: u32) -> Rgb {
    let [_, r, g, b] = hex.to_be_bytes();
    [
        f32::from(r) / 255.0,
        f32::from(g) / 255.0,
        f32::from(b) / 255.0,
    ]
}

fn mix(a: Rgb, b: Rgb, t: f32) -> Rgb {
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
    ]
}

/// Pixel coverage for a signed distance (negative inside) measured in pixels.
fn coverage(distance: f32) -> f32 {
    (0.5 - distance).clamp(0.0, 1.0)
}

fn over(dst: &mut [f32; 4], color: Rgb, alpha: f32) {
    let dst_alpha = dst[3];
    let out_alpha = alpha + dst_alpha * (1.0 - alpha);
    if out_alpha <= 0.0 {
        return;
    }
    for (channel, src) in dst.iter_mut().zip(color) {
        *channel = (src * alpha + *channel * dst_alpha * (1.0 - alpha)) / out_alpha;
    }
    dst[3] = out_alpha;
}

fn rounded_box_distance(p: Point, half: f32, radius: f32) -> f32 {
    let inner = half - radius;
    let q = ((p.0 - half).abs() - inner, (p.1 - half).abs() - inner);
    let outside = (q.0.max(0.0) * q.0.max(0.0) + q.1.max(0.0) * q.1.max(0.0)).sqrt();
    outside + q.0.max(q.1).min(0.0) - radius
}

fn segment_distance(p: Point, a: Point, b: Point) -> f32 {
    let ab = (b.0 - a.0, b.1 - a.1);
    let ap = (p.0 - a.0, p.1 - a.1);
    let t = ((ap.0 * ab.0 + ap.1 * ab.1) / (ab.0 * ab.0 + ab.1 * ab.1)).clamp(0.0, 1.0);
    let off = (ap.0 - ab.0 * t, ap.1 - ab.1 * t);
    (off.0 * off.0 + off.1 * off.1).sqrt()
}

/// Brace polylines plus the cursor bar, in pixel coordinates for a `size`-px canvas.
fn mark_segments(size: f32) -> Vec<(Point, Point)> {
    const LEFT: [Point; 7] = [
        (0.44, 0.26),
        (0.37, 0.31),
        (0.37, 0.45),
        (0.29, 0.50),
        (0.37, 0.55),
        (0.37, 0.69),
        (0.44, 0.74),
    ];
    let scale = |p: Point| (p.0 * size, p.1 * size);
    let mirror = |p: Point| (1.0 - p.0, p.1);
    let mut segments = Vec::with_capacity(13);
    for pair in LEFT.windows(2) {
        segments.push((scale(pair[0]), scale(pair[1])));
        segments.push((scale(mirror(pair[0])), scale(mirror(pair[1]))));
    }
    segments.push((scale((0.5, 0.40)), scale((0.5, 0.60))));
    segments
}

/// Renders the app icon as straight-alpha RGBA8, row-major, `size * size * 4` bytes.
#[must_use]
pub fn render(size: u32) -> Vec<u8> {
    let extent = size as f32;
    let (fill, edge, accent) = (rgb(0x16_171B), rgb(0x2C_2F3A), rgb(0x5B_9DFF));
    let highlight = [1.0, 1.0, 1.0];
    let radius = extent * 0.225;
    let half = extent / 2.0;
    let stroke = (extent * 0.035).max(0.7);
    let segments = mark_segments(extent);

    let mut out = Vec::with_capacity(size as usize * size as usize * 4);
    for y in 0..size {
        for x in 0..size {
            let p = (x as f32 + 0.5, y as f32 + 0.5);
            let mut px = [0.0_f32; 4];

            let box_distance = rounded_box_distance(p, half, radius);
            over(&mut px, edge, coverage(box_distance));
            over(&mut px, fill, coverage(box_distance + 1.0));

            let mark_distance = segments
                .iter()
                .map(|&(a, b)| segment_distance(p, a, b))
                .fold(f32::INFINITY, f32::min)
                - stroke;
            let color = mix(accent, highlight, 0.28 * (1.0 - p.1 / extent));
            over(&mut px, color, coverage(mark_distance));

            out.extend(px.map(|v| (v * 255.0 + 0.5) as u8));
        }
    }
    out
}

fn crc32(parts: &[&[u8]]) -> u32 {
    let mut crc = u32::MAX;
    for byte in parts.iter().flat_map(|part| part.iter()) {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            crc = if crc & 1 == 1 {
                (crc >> 1) ^ 0xEDB8_8320
            } else {
                crc >> 1
            };
        }
    }
    !crc
}

fn adler32(data: &[u8]) -> u32 {
    let (mut a, mut b) = (1_u32, 0_u32);
    for byte in data {
        a = (a + u32::from(*byte)) % 65_521;
        b = (b + a) % 65_521;
    }
    (b << 16) | a
}

fn push_chunk(png: &mut Vec<u8>, kind: [u8; 4], data: &[u8]) {
    png.extend_from_slice(&(data.len() as u32).to_be_bytes());
    png.extend_from_slice(&kind);
    png.extend_from_slice(data);
    png.extend_from_slice(&crc32(&[&kind, data]).to_be_bytes());
}

/// Encodes RGBA8 pixels as a PNG using stored (uncompressed) zlib blocks.
#[must_use]
pub fn encode_png(width: u32, height: u32, rgba: &[u8]) -> Vec<u8> {
    let stride = width as usize * 4;
    let mut raw = Vec::with_capacity((stride + 1) * height as usize);
    for row in rgba.chunks_exact(stride) {
        raw.push(0);
        raw.extend_from_slice(row);
    }

    let mut zlib = vec![0x78, 0x01];
    let mut blocks = raw.chunks(65_535).peekable();
    while let Some(block) = blocks.next() {
        let len = block.len() as u16;
        zlib.push(u8::from(blocks.peek().is_none()));
        zlib.extend_from_slice(&len.to_le_bytes());
        zlib.extend_from_slice(&(!len).to_le_bytes());
        zlib.extend_from_slice(block);
    }
    zlib.extend_from_slice(&adler32(&raw).to_be_bytes());

    let mut ihdr = Vec::with_capacity(13);
    ihdr.extend_from_slice(&width.to_be_bytes());
    ihdr.extend_from_slice(&height.to_be_bytes());
    ihdr.extend_from_slice(&[8, 6, 0, 0, 0]);

    let mut png = PNG_SIGNATURE.to_vec();
    push_chunk(&mut png, *b"IHDR", &ihdr);
    push_chunk(&mut png, *b"IDAT", &zlib);
    push_chunk(&mut png, *b"IEND", &[]);
    png
}

fn render_png(size: u32) -> Vec<u8> {
    encode_png(size, size, &render(size))
}

/// Builds a PNG-in-ICO container (Vista+) from `(size, png_bytes)` frames.
fn encode_ico(frames: &[(u32, Vec<u8>)]) -> Vec<u8> {
    let mut out = vec![0, 0, 1, 0];
    out.extend_from_slice(&(frames.len() as u16).to_le_bytes());
    let mut offset = 6 + 16 * frames.len();
    for (size, png) in frames {
        let dimension = if *size >= 256 { 0 } else { *size as u8 };
        out.extend_from_slice(&[dimension, dimension, 0, 0]);
        out.extend_from_slice(&1_u16.to_le_bytes());
        out.extend_from_slice(&32_u16.to_le_bytes());
        out.extend_from_slice(&(png.len() as u32).to_le_bytes());
        out.extend_from_slice(&(offset as u32).to_le_bytes());
        offset += png.len();
    }
    for (_, png) in frames {
        out.extend_from_slice(png);
    }
    out
}

#[must_use]
pub fn build() -> Assets {
    let frames: Vec<(u32, Vec<u8>)> = ICO_SIZES.iter().map(|&s| (s, render_png(s))).collect();
    let png_256 = frames
        .iter()
        .find(|(size, _)| *size == 256)
        .map_or_else(|| render_png(256), |(_, png)| png.clone());
    Assets {
        ico: encode_ico(&frames),
        png_256,
        png_512: render_png(512),
    }
}

/// Writes `jide.ico`, `jide-256.png`, and `jide-512.png` under `<root>/assets/icons/`.
pub fn run(root: &Path) -> Result<()> {
    let dir = root.join("assets").join("icons");
    std::fs::create_dir_all(&dir).with_context(|| format!("creating {}", dir.display()))?;
    let assets = build();
    for (name, bytes) in [
        ("jide.ico", &assets.ico),
        ("jide-256.png", &assets.png_256),
        ("jide-512.png", &assets.png_512),
    ] {
        let path = dir.join(name);
        std::fs::write(&path, bytes).with_context(|| format!("writing {}", path.display()))?;
        println!("Generated {}: {} bytes", path.display(), bytes.len());
    }
    Ok(())
}
