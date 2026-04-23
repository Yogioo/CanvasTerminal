use super::GraphApp;
use crate::model::NodeKind;
use eframe::egui::{self, vec2, ColorImage, TextureHandle, TextureOptions};
use image::ImageReader;
use std::path::Path;

impl GraphApp {
    pub(in crate::app) fn is_supported_image_path(path: &Path) -> bool {
        path.extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| {
                matches!(
                    ext.to_ascii_lowercase().as_str(),
                    "png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp"
                )
            })
            .unwrap_or(false)
    }

    pub(in crate::app) fn decode_image_bytes(bytes: &[u8]) -> Result<ColorImage, String> {
        let image = image::load_from_memory(bytes).map_err(|e| format!("图片解码失败: {e}"))?;
        let rgba = image.to_rgba8();
        let size = [rgba.width() as usize, rgba.height() as usize];
        let pixels = rgba.into_vec();
        Ok(ColorImage::from_rgba_unmultiplied(size, &pixels))
    }

    fn load_image_from_path(path: &str) -> Result<ColorImage, String> {
        let reader = ImageReader::open(path).map_err(|e| format!("无法读取图片: {e}"))?;
        let image = reader.decode().map_err(|e| format!("图片解码失败: {e}"))?;
        let rgba = image.to_rgba8();
        let size = [rgba.width() as usize, rgba.height() as usize];
        let pixels = rgba.into_vec();
        Ok(ColorImage::from_rgba_unmultiplied(size, &pixels))
    }

    pub(in crate::app) fn image_aspect(&self, node_id: usize) -> Option<f32> {
        self.image_aspects.get(&node_id).copied()
    }

    fn ensure_image_texture(&mut self, node_id: usize, ctx: &egui::Context) {
        if self.image_textures.contains_key(&node_id) || self.image_errors.contains_key(&node_id) {
            return;
        }

        let Some(node) = self
            .nodes
            .iter()
            .find(|n| n.id == node_id && n.kind == NodeKind::Image)
        else {
            return;
        };

        let image_path = node.image_path.clone();
        let image = if let Some(bytes) = self.image_bytes.get(&node_id) {
            Self::decode_image_bytes(bytes)
        } else if image_path.trim().is_empty() {
            return;
        } else {
            Self::load_image_from_path(&image_path)
        };

        match image {
            Ok(color_image) => {
                let [w, h] = color_image.size;
                let aspect = if h == 0 { 1.0 } else { w as f32 / h as f32 };
                let texture = ctx.load_texture(
                    format!("image-node-{node_id}"),
                    color_image,
                    TextureOptions::LINEAR,
                );
                self.image_textures.insert(node_id, texture);
                self.image_errors.remove(&node_id);
                self.image_aspects.insert(node_id, aspect);

                if let Some(node) = self.nodes.iter_mut().find(|n| n.id == node_id) {
                    node.size = vec2(w as f32, h as f32);
                }
            }
            Err(err) => {
                self.image_errors.insert(node_id, err);
            }
        }
    }

    pub(in crate::app) fn ensure_image_textures(&mut self, ctx: &egui::Context) {
        let image_ids: Vec<usize> = self
            .nodes
            .iter()
            .filter(|n| n.kind == NodeKind::Image)
            .map(|n| n.id)
            .collect();
        for node_id in image_ids {
            self.ensure_image_texture(node_id, ctx);
        }
    }

    pub(in crate::app) fn image_texture(&self, node_id: usize) -> Option<&TextureHandle> {
        self.image_textures.get(&node_id)
    }

    pub(in crate::app) fn image_error(&self, node_id: usize) -> Option<&str> {
        self.image_errors.get(&node_id).map(String::as_str)
    }
}
