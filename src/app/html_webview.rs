use std::collections::HashMap;

use eframe::{egui::Rect, CreationContext};
use raw_window_handle::{
    DisplayHandle, HandleError, HasDisplayHandle, HasWindowHandle, RawDisplayHandle,
    RawWindowHandle, WindowHandle,
};
#[cfg(target_os = "windows")]
use wry::{dpi::LogicalPosition, dpi::LogicalSize, Rect as WryRect, WebView, WebViewBuilder};
#[cfg(target_os = "windows")]
use wry::WebViewBuilderExtWindows;




#[derive(Clone, Copy)]
pub(in crate::app) struct HtmlHostHandles {
    raw_window_handle: RawWindowHandle,
    raw_display_handle: RawDisplayHandle,
}

impl HtmlHostHandles {
    pub(in crate::app) fn from_creation_context(cc: &CreationContext<'_>) -> Option<Self> {
        let raw_window_handle = cc.window_handle().ok()?.as_raw();
        let raw_display_handle = cc.display_handle().ok()?.as_raw();
        Some(Self {
            raw_window_handle,
            raw_display_handle,
        })
    }
}

impl HasWindowHandle for HtmlHostHandles {
    fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
        Ok(unsafe { WindowHandle::borrow_raw(self.raw_window_handle) })
    }
}

impl HasDisplayHandle for HtmlHostHandles {
    fn display_handle(&self) -> Result<DisplayHandle<'_>, HandleError> {
        Ok(unsafe { DisplayHandle::borrow_raw(self.raw_display_handle) })
    }
}

#[cfg(target_os = "windows")]
struct HtmlWebViewInstance {
    webview: WebView,
    applied_html: String,
}

#[derive(Default)]
pub(in crate::app) struct HtmlWebViewHost {
    #[cfg(target_os = "windows")]
    webviews: HashMap<usize, HtmlWebViewInstance>,
}

impl HtmlWebViewHost {

    #[cfg(target_os = "windows")]
    pub(in crate::app) fn sync_webview(
        &mut self,
        node_id: usize,
        html_source: &str,
        screen_rect: Rect,
        handles: &HtmlHostHandles,
    ) {
        let bounds = WryRect {
            position: LogicalPosition::new(
                screen_rect.min.x.round() as i32,
                screen_rect.min.y.round() as i32,
            )
            .into(),
            size: LogicalSize::new(
                screen_rect.width().max(1.0).round(),
                screen_rect.height().max(1.0).round(),
            )
            .into(),
        };

        if let Some(instance) = self.webviews.get_mut(&node_id) {
            let _ = instance.webview.set_bounds(bounds);
            let _ = instance.webview.set_visible(true);
            if instance.applied_html != html_source {
                let _ = instance.webview.load_html(html_source);
                instance.applied_html = html_source.to_owned();
            }
        } else {
            let Ok(wv) = WebViewBuilder::new()
                .with_html(html_source.to_owned())
                .with_bounds(bounds)
                .with_browser_accelerator_keys(false)
                .build_as_child(handles)
            else {
                return;
            };
            self.webviews.insert(
                node_id,
                HtmlWebViewInstance {
                    webview: wv,
                    applied_html: html_source.to_owned(),
                },
            );
        }
    }

    #[cfg(not(target_os = "windows"))]
    pub(in crate::app) fn sync_webview(
        &mut self,
        _node_id: usize,
        _html_source: &str,
        _screen_rect: Rect,
        _handles: &HtmlHostHandles,
    ) {}

    #[cfg(target_os = "windows")]
    pub(in crate::app) fn remove_webview(&mut self, node_id: usize) {
        self.webviews.remove(&node_id);
    }

    #[cfg(not(target_os = "windows"))]
    pub(in crate::app) fn remove_webview(&mut self, _node_id: usize) {}

    pub(in crate::app) fn clear_all(&mut self) {
        #[cfg(target_os = "windows")]
        { self.webviews.clear(); }
    }

    #[cfg(target_os = "windows")]
    pub(in crate::app) fn remove_orphans(&mut self, active_ids: &std::collections::HashSet<usize>) {
        self.webviews.retain(|id, _| active_ids.contains(id));
    }

    #[cfg(not(target_os = "windows"))]
    pub(in crate::app) fn remove_orphans(&mut self, _active_ids: &std::collections::HashSet<usize>) {}

    #[cfg(target_os = "windows")]
    pub(in crate::app) fn set_visible(&mut self, node_id: usize, visible: bool) {
        if let Some(instance) = self.webviews.get(&node_id) {
            let _ = instance.webview.set_visible(visible);
        }
    }

    #[cfg(not(target_os = "windows"))]
    pub(in crate::app) fn set_visible(&mut self, _node_id: usize, _visible: bool) {}

    /// When the user interacts with the canvas (clicking on nodes, edges, or empty space),
    /// call this method to return keyboard focus from the webview back to the parent window.
    /// This ensures that keyboard shortcuts (Ctrl+C, Delete, etc.) are handled by the canvas
    /// rather than being intercepted by the webview.
    pub(in crate::app) fn return_focus_to_parent(&self, handles: &HtmlHostHandles) {
        #[cfg(target_os = "windows")]
        {
            use windows::Win32::UI::Input::KeyboardAndMouse::SetFocus;
            use windows::Win32::Foundation::HWND;
            use raw_window_handle::RawWindowHandle;

            if let RawWindowHandle::Win32(handle) = handles.raw_window_handle {
                let hwnd = HWND(handle.hwnd.get() as _);
                unsafe { let _ = SetFocus(Some(hwnd)); }
            }
        }
        #[cfg(not(target_os = "windows"))]
        let _ = handles;
    }
}
