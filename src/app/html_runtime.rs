use super::html_webview::NavEvent;
use super::GraphApp;
use crate::constants::{HTML_HEADER_HEIGHT, WEBPAGE_HEADER_HEIGHT};
use crate::model::{NodeData, NodeKind};
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

    pub(in crate::app) fn is_webpage_node(&self, node_id: usize) -> bool {
        self.nodes
            .iter()
            .find(|node| node.id == node_id)
            .is_some_and(|node| node.kind == NodeKind::WebPage)
    }

    /// Drain webview navigation events and update node URLs
    pub(in crate::app) fn poll_webview_nav_events(&mut self) {
        let events = self.html_webview_host.drain_nav_events();
        for event in events {
            let NavEvent::Navigating { node_id, url } = &event;
            eprintln!("[POLL_NAV] Navigating: node={node_id} url={url}");
            // The webview is ALREADY navigating. Just sync applied_source + node url.
            self.html_webview_host.on_navigated(*node_id, url);
            Self::update_webpage_node_url(self, *node_id, url);
            // Inject anti-blank JS after every navigation (page just loaded → JS context ready)
            self.html_webview_host.inject_anti_blank(*node_id);
            self.webviews_dirty = true;
        }
    }

    /// Helper: update a WebPage node's URL in the data model
    fn update_webpage_node_url(this: &mut Self, node_id: usize, new_url: &str) {
        if let Some(node) = this.nodes.iter_mut().find(|n| n.id == node_id) {
            if node.kind == NodeKind::WebPage {
                if let NodeData::WebPage { url } = &mut node.data {
                    if *url != new_url {
                        eprintln!("[POLL_NAV] URL changed: '{}' -> '{}'", url, new_url);
                        *url = new_url.to_owned();
                        this.mark_workspace_dirty();
                    }
                }
            }
        }
    }

    /// Poll IPC events (Ctrl+Click from JavaScript) and create new WebPage nodes
    pub(in crate::app) fn poll_ipc_events(&mut self) {
        let events = self.html_webview_host.drain_ipc_events();
        for event in events {
            eprintln!("[IPC_POLL] Ctrl+Click: from node={} url={}", event.node_id, event.url);

            // Calculate position and size: match the source node, place to the right
            let (spawn_pos, spawn_size) = if let Some(src_node) = self.nodes.iter().find(|n| n.id == event.node_id) {
                (
                    egui::Pos2::new(
                        src_node.pos.x + src_node.size.x + 24.0,
                        src_node.pos.y,
                    ),
                    src_node.size,
                )
            } else {
                (egui::Pos2::new(100.0, 100.0), egui::vec2(420.0, 260.0))
            };

            // Create the new WebPage node with the same size
            let new_id = self.create_webpage_node(spawn_pos, false);
            if let Some(new_node) = self.nodes.iter_mut().find(|n| n.id == new_id) {
                new_node.size = spawn_size;
            }
            // Set its URL and navigate
            self.navigate_webview_to(new_id, &event.url);
        }
    }

    /// Programmatically navigate a webpage node to a new URL
    pub(in crate::app) fn navigate_webview_to(&mut self, node_id: usize, url: &str) {
        // Normalize URL: prepend https:// if no scheme is present
        let normalized = if !url.contains("://") && !url.is_empty() {
            format!("https://{url}")
        } else {
            url.to_owned()
        };

        // Update node data and navigate only if URL changed
        let mut changed = false;
        if let Some(node) = self.nodes.iter_mut().find(|n| n.id == node_id) {
            if node.kind == NodeKind::WebPage {
                if let NodeData::WebPage { url } = &mut node.data {
                    if *url != normalized {
                        *url = normalized.clone();
                        self.mark_workspace_dirty();
                        changed = true;
                    }
                }
            }
        }

        if changed {
            self.html_webview_host.navigate_to(node_id, &normalized);
        }
    }

    /// Called each frame after node rendering.
    /// Syncs all HTML and WebPage nodes' webview positions and content.
    /// Creates new webviews for new nodes, updates bounds for existing ones.
    pub(in crate::app) fn sync_all_html_webviews(&mut self, canvas_rect: egui::Rect) {
        // Dirty-guard: skip the entire webview sync loop when nothing has changed
        // (zoom, node positions, content, window size). This prevents ANY touch
        // on the WebView2 child windows during simple mouse movement on the canvas.
        if !self.webviews_dirty {
            return;
        }
        self.webviews_dirty = false;

        let Some(handles) = self.html_host_handles else {
            return;
        };

        // Collect both Html and WebPage nodes: (id, source, is_url)
        let web_node_ids: Vec<(usize, String, bool)> = self
            .nodes
            .iter()
            .filter(|n| n.kind == NodeKind::Html || n.kind == NodeKind::WebPage)
            .map(|n| match &n.data {
                crate::model::NodeData::Html { html_source } => (n.id, html_source.clone(), false),
                crate::model::NodeData::WebPage { url } => (n.id, url.clone(), true),
                _ => unreachable!(),
            })
            .collect();

        let live_ids: std::collections::HashSet<usize> =
            web_node_ids.iter().map(|(id, _, _)| *id).collect();
        let editing_id = self.editing_text_node;

        for (node_id, source, is_url) in &web_node_ids {
            // Skip if this node is currently being edited (text editor is open instead)
            if Some(*node_id) == editing_id {
                continue;
            }

            let Some(node) = self.nodes.iter().find(|n| n.id == *node_id) else {
                continue;
            };

            let header_height = match node.kind {
                NodeKind::Html => HTML_HEADER_HEIGHT,
                NodeKind::WebPage => WEBPAGE_HEADER_HEIGHT,
                _ => unreachable!(),
            };
            let content_world_y = node.pos.y + header_height;
            let content_world_height = (node.size.y - header_height).max(1.0);
            let content_world_rect = egui::Rect::from_min_size(
                egui::Pos2::new(node.pos.x, content_world_y),
                egui::Vec2::new(node.size.x, content_world_height),
            );
            let screen_rect = self.world_to_screen_rect(canvas_rect, content_world_rect);

            // Normalize URL: prepend https:// if no scheme is present
            let normalized_source = if *is_url && !source.is_empty() && !source.contains("://") {
                format!("https://{source}")
            } else {
                source.clone()
            };

            self.html_webview_host.sync_webview(
                *node_id,
                &normalized_source,
                screen_rect,
                &handles,
                *is_url,
                self.zoom,
            );
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
