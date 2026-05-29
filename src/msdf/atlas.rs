use serde::Deserialize;
use std::collections::HashMap;

/// Bounds for a glyph in em space (plane) or atlas pixel space.
#[derive(Clone, Copy, Debug, Default)]
pub struct Bounds {
    pub left: f32,
    pub bottom: f32,
    pub right: f32,
    pub top: f32,
}

impl Bounds {
    #[allow(dead_code)]
    pub fn width(&self) -> f32 {
        self.right - self.left
    }
    #[allow(dead_code)]
    pub fn height(&self) -> f32 {
        self.top - self.bottom
    }
}

/// A single glyph entry.
#[derive(Clone, Debug)]
pub struct MsdfGlyph {
    pub advance: f32,
    pub plane_bounds: Option<Bounds>,
    pub atlas_bounds: Option<Bounds>,
}

/// Parsed MSDF/MTSDF atlas.
pub struct MsdfAtlas {
    pub atlas_width: u32,
    pub atlas_height: u32,
    pub distance_range: f32,
    #[allow(dead_code)]
    pub font_size_em: f32,
    pub glyphs: HashMap<char, MsdfGlyph>,
    pub png_data: Vec<u8>,
}

// ── JSON schema (intermediate) ──

#[derive(Deserialize)]
struct JsonAtlasRoot {
    atlas: JsonAtlasInfo,
    glyphs: Vec<JsonGlyph>,
}

#[derive(Deserialize)]
struct JsonAtlasInfo {
    #[serde(rename = "type")]
    atlas_type: String,
    #[serde(rename = "distanceRange")]
    distance_range: f32,
    size: f32,
    width: u32,
    height: u32,
}

#[derive(Deserialize)]
struct JsonGlyph {
    unicode: u32,
    advance: f32,
    #[serde(rename = "planeBounds")]
    plane_bounds: Option<JsonBounds>,
    #[serde(rename = "atlasBounds")]
    atlas_bounds: Option<JsonBounds>,
}

#[derive(Clone, Deserialize)]
#[allow(dead_code)]
struct JsonBounds {
    left: f32,
    bottom: f32,
    right: f32,
    top: f32,
}

impl MsdfAtlas {
    /// Load atlas from a JSON string and raw PNG bytes.
    pub fn load(json_bytes: &[u8], png_bytes: &[u8]) -> Result<Self, String> {
        let root: JsonAtlasRoot =
            serde_json::from_slice(json_bytes).map_err(|e| format!("JSON parse error: {e}"))?;

        if root.atlas.atlas_type != "mtsdf" && root.atlas.atlas_type != "msdf" {
            return Err(format!(
                "Unsupported atlas type: {} (expected mtsdf or msdf)",
                root.atlas.atlas_type
            ));
        }

        let mut glyphs: HashMap<char, MsdfGlyph> = HashMap::new();
        for g in &root.glyphs {
            let ch = match char::from_u32(g.unicode) {
                Some(c) => c,
                None => continue,
            };
            let plane_bounds = g.plane_bounds.clone().map(|b| Bounds {
                left: b.left,
                bottom: b.bottom,
                right: b.right,
                top: b.top,
            });
            let atlas_bounds = g.atlas_bounds.clone().map(|b| Bounds {
                left: b.left,
                bottom: b.bottom,
                right: b.right,
                top: b.top,
            });
            glyphs.insert(
                ch,
                MsdfGlyph {
                    advance: g.advance,
                    plane_bounds,
                    atlas_bounds,
                },
            );
        }

        Ok(Self {
            atlas_width: root.atlas.width,
            atlas_height: root.atlas.height,
            distance_range: root.atlas.distance_range,
            font_size_em: root.atlas.size,
            glyphs,
            png_data: png_bytes.to_vec(),
        })
    }

    /// Look up a glyph. Returns `None` if the character is missing.
    pub fn glyph(&self, ch: char) -> Option<&MsdfGlyph> {
        self.glyphs.get(&ch)
    }
}
