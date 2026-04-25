use super::super::GraphApp;
use arboard::Clipboard;
use eframe::egui::{self, Pos2, Rect};
use std::path::Path;

impl GraphApp {
    fn is_dropped_image_file(file: &egui::DroppedFile) -> bool {
        if !file.mime.is_empty() && file.mime.starts_with("image/") {
            return true;
        }

        if let Some(path) = &file.path {
            return Self::is_supported_image_path(path);
        }

        Path::new(&file.name)
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| {
                matches!(
                    ext.to_ascii_lowercase().as_str(),
                    "png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp"
                )
            })
            .unwrap_or(false)
    }

    fn parse_pasted_paths(text: &str) -> Vec<String> {
        text.lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .map(|line| line.trim_matches('"'))
            .map(|line| line.strip_prefix("file://").unwrap_or(line))
            .map(ToOwned::to_owned)
            .collect()
    }

    pub(in crate::app::ui) fn handle_canvas_image_import(
        &mut self,
        ctx: &egui::Context,
        rect: Rect,
        pointer_pos: Option<Pos2>,
        pointer_in_canvas: bool,
    ) {
        let now = ctx.input(|i| i.time);
        let dropped_files = ctx.input(|i| i.raw.dropped_files.clone());
        if !dropped_files.is_empty() {
            self.pending_dropped_files = dropped_files;
            self.pending_drop_spawn_world_pos = None;
            self.pending_drop_requested_at = Some(now);
        }

        if !self.pending_dropped_files.is_empty() {
            ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
            ctx.request_repaint();

            let drop_pointer_world_pos = if pointer_in_canvas {
                pointer_pos.map(|pointer| self.screen_to_world_pos(rect, pointer))
            } else {
                None
            };

            if let Some(world_pos) = drop_pointer_world_pos {
                self.pending_drop_spawn_world_pos = Some(world_pos);
            }

            if self.pending_drop_spawn_world_pos.is_none()
                && self
                    .pending_drop_requested_at
                    .is_some_and(|start| now - start > 0.8)
            {
                self.pending_dropped_files.clear();
                self.pending_drop_requested_at = None;
            }
        }

        let mut spawn_local = if !self.pending_dropped_files.is_empty() {
            self.pending_drop_spawn_world_pos
        } else if pointer_in_canvas {
            pointer_pos
                .map(|pointer| self.screen_to_world_pos(rect, pointer))
                .or(self.last_canvas_pointer_world_pos)
        } else {
            self.last_canvas_pointer_world_pos
        };

        if let Some(mut drop_spawn_local) = self.pending_drop_spawn_world_pos {
            let pending_files = std::mem::take(&mut self.pending_dropped_files);
            for file in pending_files {
                if !Self::is_dropped_image_file(&file) {
                    continue;
                }

                let spawn_pos = drop_spawn_local;
                if let Some(path) = file.path {
                    self.create_image_node_from_path(spawn_pos, path.to_string_lossy().to_string());
                } else if let Some(bytes) = file.bytes {
                    let display_name = if file.name.trim().is_empty() {
                        "粘贴图片".to_owned()
                    } else {
                        file.name
                    };
                    self.create_image_node_from_bytes(spawn_pos, display_name, bytes.to_vec());
                }
                self.advance_spawn_pos_below_selected(&mut drop_spawn_local);
            }
            spawn_local = Some(drop_spawn_local);
            self.last_drag_hover_world_pos = None;
            self.pending_drop_spawn_world_pos = None;
            self.pending_drop_requested_at = None;
        }

        let Some(mut spawn_local) = spawn_local else {
            return;
        };

        let (key_v_pressed, ctrl_down, command_down, paste_event_count, raw_paste_event_count) =
            ctx.input(|i| {
                let paste_events = i
                    .events
                    .iter()
                    .filter(|event| matches!(event, egui::Event::Paste(_)))
                    .count();

                let raw_paste_events = i
                    .raw
                    .events
                    .iter()
                    .filter(|event| matches!(event, egui::Event::Paste(_)))
                    .count();

                (
                    i.key_pressed(egui::Key::V),
                    i.modifiers.ctrl,
                    i.modifiers.command,
                    paste_events,
                    raw_paste_events,
                )
            });

        let paste_shortcut = key_v_pressed && (command_down || ctrl_down);
        let paste_requested = paste_shortcut || paste_event_count > 0 || raw_paste_event_count > 0;
        let suppress_canvas_image_paste = self.editing_text_node.is_some()
            || self.editing_title_node.is_some()
            || self.editing_startup_node.is_some()
            || ctx.wants_keyboard_input();

        if paste_requested && pointer_in_canvas && !suppress_canvas_image_paste {
            if let Ok(mut clipboard) = Clipboard::new() {
                if let Ok(image) = clipboard.get_image() {
                    let spawn_pos = spawn_local;
                    let size = [image.width, image.height];
                    let color_image = egui::ColorImage::from_rgba_unmultiplied(size, &image.bytes);
                    self.create_image_node_from_color_image(
                        spawn_pos,
                        "粘贴图片".to_owned(),
                        color_image,
                        ctx,
                    );
                    return;
                }

                if let Ok(files) = clipboard.get().file_list() {
                    let mut created_from_files = 0usize;
                    for file in files {
                        if Self::is_supported_image_path(&file) {
                            let spawn_pos = spawn_local;
                            self.create_image_node_from_path(
                                spawn_pos,
                                file.to_string_lossy().to_string(),
                            );
                            self.advance_spawn_pos_below_selected(&mut spawn_local);
                            created_from_files += 1;
                        }
                    }

                    if created_from_files > 0 {
                        return;
                    }
                }

                if let Ok(text) = clipboard.get_text() {
                    for candidate in Self::parse_pasted_paths(&text) {
                        let path = Path::new(&candidate);
                        if path.exists() && Self::is_supported_image_path(path) {
                            let spawn_pos = spawn_local;
                            self.create_image_node_from_path(spawn_pos, candidate);
                            self.advance_spawn_pos_below_selected(&mut spawn_local);
                        }
                    }
                }
            }
        }

        if !pointer_in_canvas || suppress_canvas_image_paste {
            return;
        }

        let pasted_texts: Vec<String> = ctx.input(|i| {
            i.events
                .iter()
                .filter_map(|event| match event {
                    egui::Event::Paste(text) => Some(text.clone()),
                    _ => None,
                })
                .collect()
        });

        for pasted in pasted_texts {
            for candidate in Self::parse_pasted_paths(&pasted) {
                let path = Path::new(&candidate);
                if path.exists() && Self::is_supported_image_path(path) {
                    let spawn_pos = spawn_local;
                    self.create_image_node_from_path(spawn_pos, candidate);
                    self.advance_spawn_pos_below_selected(&mut spawn_local);
                }
            }
        }
    }

}
