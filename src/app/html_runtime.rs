use super::GraphApp;
use crate::constants::HTML_HEADER_HEIGHT;
use crate::model::NodeKind;
use eframe::egui;

impl GraphApp {
    pub(in crate::app) fn html_host_available(&self) -> bool {
        self.html_host_handles.is_some()
    }

    pub(in crate::app) fn is_html_node(&self, node_id: usize) -> bool {
        self.nodes
            .iter()
            .find(|node| node.id == node_id)
            .is_some_and(|node| node.kind == NodeKind::Html)
    }

    /// Called each frame after node rendering.
    /// Syncs all HTML nodes' webview positions and HTML source.
    /// Creates new webviews for new nodes, updates bounds for existing ones.
    pub(in crate::app) fn sync_all_html_webviews(&mut self, canvas_rect: egui::Rect) {
        let Some(handles) = self.html_host_handles else {
            return;
        };

        let html_node_ids: Vec<(usize, String)> = self
            .nodes
            .iter()
            .filter(|n| n.kind == NodeKind::Html)
            .map(|n| {
                let source = match &n.data {
                    crate::model::NodeData::Html { html_source } => html_source.clone(),
                    _ => String::new(),
                };
                (n.id, source)
            })
            .collect();

        let live_ids: std::collections::HashSet<usize> =
            html_node_ids.iter().map(|(id, _)| *id).collect();
        let editing_id = self.editing_text_node;

        for (node_id, html_source) in &html_node_ids {
            // Skip if this node is currently being edited (text editor is open instead)
            if Some(*node_id) == editing_id {
                continue;
            }

            let Some(node) = self.nodes.iter().find(|n| n.id == *node_id) else {
                continue;
            };

            let content_world_y = node.pos.y + HTML_HEADER_HEIGHT;
            let content_world_height = (node.size.y - HTML_HEADER_HEIGHT).max(1.0);
            let content_world_rect = egui::Rect::from_min_size(
                egui::Pos2::new(node.pos.x, content_world_y),
                egui::Vec2::new(node.size.x, content_world_height),
            );
            let screen_rect = self.world_to_screen_rect(canvas_rect, content_world_rect);

            self.html_webview_host
                .sync_webview(*node_id, html_source, screen_rect, &handles);
        }

        // Remove orphaned webviews (nodes that no longer exist)
        self.html_webview_host.remove_orphans(&live_ids);

        // Hide webview for the node currently being edited (so the text editor is not covered)
        if let Some(edit_id) = editing_id {
            self.html_webview_host.set_visible(edit_id, false);
    }
}

    /// When the user interacts with the canvas (clicking on nodes, edges, or empty space),
    /// call this method to return keyboard focus from the webview back to the parent window.
    /// This ensures that keyboard shortcuts (Ctrl+C, Delete, etc.) are handled by the canvas
    /// rather than being intercepted by the webview.
    pub(in crate::app) fn ensure_canvas_focus(&self) {
        if let Some(ref handles) = self.html_host_handles {
            self.html_webview_host.return_focus_to_parent(handles);
        }
    }
}
