//! P5-B1 Runtime MSDF Generation Spike
//!
//! Standalone binary that generates MTSDF glyph bitmaps from a TTF/OTF font
//! using the pure-Rust `fdsm` crate, and saves them as PNG files.
//!
//! Usage: cargo run --bin msdf_spike
//!
//! Output: tmp/msdf_spike_<char>.png

use fdsm::bezier::scanline::FillRule;
use fdsm::generate::generate_mtsdf;
use fdsm::render::correct_sign_mtsdf;
use fdsm::shape::Shape;
use fdsm::transform::Transform;
use image::RgbaImage;
use nalgebra::{Affine2, Similarity2, Vector2};
use std::fs;
use std::path::PathBuf;

/// Target font size in pixels (em size), matching the current atlas (font_size=48).
const TARGET_FONT_SIZE: f64 = 48.0;

/// px_range / distanceRange — must match existing atlas (4.0) and shader uniform.
const PX_RANGE: f64 = 4.0;

/// Test characters for spike validation.
/// 盎 (U+76CE) — GB2312 L1 common char, should be in existing atlas (baseline).
/// 龘 (U+9F98) — GB2312 L2 rare char, likely NOT in existing atlas.
const TEST_CHARS: &[char] = &['盎', '龘'];

fn main() -> Result<(), String> {
    let font_path = r"C:\Windows\Fonts\NotoSansSC-VF.ttf";
    let output_dir = PathBuf::from("tmp");

    fs::create_dir_all(&output_dir)
        .map_err(|e| format!("Failed to create output dir {}: {e}", output_dir.display()))?;

    println!("=== P5-B1 Runtime MSDF Spike ===");
    println!("Font: {font_path}");
    println!("Output dir: {}", output_dir.display());
    println!();

    // 1. Read font file
    let font_data =
        fs::read(font_path).map_err(|e| format!("Cannot read font file: {e}"))?;

    // 2. Parse font face
    let face = ttf_parser::Face::parse(&font_data, 0)
        .map_err(|e| format!("Failed to parse font face: {e:?}"))?;

    let units_per_em = face.units_per_em() as f64;
    println!("Font units_per_em: {units_per_em}");

    // Use actual units_per_em for shrinkage calculation
    let shrinkage = units_per_em / TARGET_FONT_SIZE;
    println!("Target font size: {TARGET_FONT_SIZE} em-px");
    println!("Shrinkage (font_units/texel): {shrinkage:.2}");
    println!("PX range: {PX_RANGE}");
    println!();

    for &ch in TEST_CHARS {
        println!("--- Processing U+{:04X} '{}' ---", ch as u32, ch);

        // 3. Get glyph index
        let glyph_id = match face.glyph_index(ch) {
            Some(id) => id,
            None => {
                println!("  SKIP: char '{}' not found in font", ch);
                println!();
                continue;
            }
        };
        println!("  Glyph ID: {}", glyph_id.0);

        // 4. Get glyph bounding box (font design units)
        let bbox = match face.glyph_bounding_box(glyph_id) {
            Some(bb) => bb,
            None => {
                println!("  SKIP: no bounding box for glyph");
                println!();
                continue;
            }
        };
        println!(
            "  BBox (font units): x=[{:.0}, {:.0}] y=[{:.0}, {:.0}]",
            bbox.x_min as f64, bbox.x_max as f64, bbox.y_min as f64, bbox.y_max as f64
        );

        // 5. Load shape from glyph
        let mut shape = match fdsm_ttf_parser::load_shape_from_face(&face, glyph_id) {
            Some(s) => s,
            None => {
                println!("  SKIP: could not load glyph shape (likely no outlines in font)");
                println!();
                continue;
            }
        };
        println!("  Contours: {}", shape.contours.len());

        // 6. Compute transformation: font units → pixel space with RANGE margin.
        //    The shape must be scaled so that em-size glyphs span TARGET_FONT_SIZE pixels,
        //    and we add PX_RANGE pixels of margin on each side for the distance field.
        //
        //    With msdf-atlas-gen alignment (origin at bottom-left):
        //    - Shift so bbox.min maps to RANGE (margin)
        //    - Scale by 1/shrinkage (font units → pixels)
        let transformation: Affine2<f64> = nalgebra::convert(Similarity2::new(
            Vector2::new(
                PX_RANGE - (bbox.x_min as f64) / shrinkage,
                PX_RANGE - (bbox.y_min as f64) / shrinkage,
            ),
            0.0,
            1.0 / shrinkage,
        ));

        // Compute output image dimensions
        let width =
            ((bbox.x_max as f64 - bbox.x_min as f64) / shrinkage + 2.0 * PX_RANGE).ceil() as u32;
        let height =
            ((bbox.y_max as f64 - bbox.y_min as f64) / shrinkage + 2.0 * PX_RANGE).ceil() as u32;
        println!("  Output image: {width}×{height}");

        // 7. Apply transformation to shape
        shape.transform(&transformation);

        // 8. Edge coloring (required for MSDF multi-channel)
        //    sin_alpha=0.03 is the default from msdfgen; seed is arbitrary
        let colored_shape = Shape::edge_coloring_simple(shape, 0.03, 69441337420);
        let prepared = colored_shape.prepare();

        // 9. Generate MTSDF (multi-channel MSDF + true SDF in alpha)
        //    MTSDF format: R=MSDF channel 0, G=MSDF channel 1, B=MSDF channel 2, A=true SDF
        //    This matches the current atlas format and shader expectation.
        let mut mtsdf = RgbaImage::new(width, height);
        generate_mtsdf(&prepared, PX_RANGE, &mut mtsdf);

        // 10. Sign correction: ensure inside=negative (msdfgen convention)
        //     The shader expects median(RGB) - 0.5 normalized, so sign matters.
        correct_sign_mtsdf(&mut mtsdf, &prepared, FillRule::Nonzero);

        // 11. Save as PNG
        let png_path = output_dir.join(format!("msdf_spike_{ch}.png"));
        mtsdf
            .save(&png_path)
            .map_err(|e| format!("Failed to save PNG {}: {e}", png_path.display()))?;

        let file_size = fs::metadata(&png_path)
            .map(|m| m.len())
            .unwrap_or(0);
        println!("  Saved: {} ({file_size} bytes)", png_path.display());

        // 12. Print metrics for atlas metadata compatibility analysis
        //     planeBounds: relative to baseline origin, in em-normalized coordinates.
        //     atlasBounds: pixel coordinates in atlas texture.
        //     advance: glyph advance width in em units.
        let advance = face.glyph_hor_advance(glyph_id).unwrap_or(0) as f64;
        println!("  Metrics:");
        println!("    advance (font units): {advance}");
        println!("    advance (pixels): {:.2}", advance / shrinkage);
        println!(
            "    planeBounds (em): left={:.5} bottom={:.5} right={:.5} top={:.5}",
            bbox.x_min as f64 / units_per_em,
            bbox.y_min as f64 / units_per_em,
            bbox.x_max as f64 / units_per_em,
            bbox.y_max as f64 / units_per_em,
        );
        println!(
            "    atlasBounds (px): left={:.1} bottom={:.1} right={:.1} top={:.1}",
            PX_RANGE,
            PX_RANGE,
            PX_RANGE + ((bbox.x_max - bbox.x_min) as f64) / shrinkage,
            PX_RANGE + ((bbox.y_max - bbox.y_min) as f64) / shrinkage,
        );
        println!(
            "    Note: atlas y-origin = bottom in msdf-atlas-gen convention."
        );
        println!(
            "          fdsm generates top-down image → flip vertically when uploading to GPU."
        );
        println!();
    }

    println!("=== Done ===");
    println!("Open output images with any image viewer to inspect quality.");
    println!("The R/G/B channels carry the 3-channel signed distance field.");
    println!("The A channel carries the true signed distance field (SDF).");

    Ok(())
}
