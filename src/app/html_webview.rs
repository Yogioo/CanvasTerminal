use std::collections::HashMap;
use std::sync::mpsc;

/// JavaScript injected into URL webviews (idempotent via window.__antiBlankInstalled flag):
/// 1. Strips target="_blank" from all links (navigate in-place)
/// 2. Intercepts Ctrl+Click / Cmd+Click → IPC for node creation
/// 3. Override window.open() → same-window navigation (triggers NavigationStarting)
/// 4. Intercept history.pushState/replaceState → IPC for URL tracking
const ANTI_BLANK_JS: &str = r#"
(function(){
    console.log('[ANTI_BLANK] script started, url='+location.href);
    if(window.__antiBlankInstalled){console.log('[ANTI_BLANK] already installed, skipping');return;}
    window.__antiBlankInstalled=true;
    console.log('[ANTI_BLANK] installing handlers...');

    // ★ All DOM-dependent code must go inside DOMContentLoaded ★
    document.addEventListener('DOMContentLoaded',function(){
        console.log('[ANTI_BLANK] DOM ready, attaching handlers');

        // 1. Strip target=_blank
        (function(){
            function fix(){
                var els=document.querySelectorAll('[target="_blank"]');
                for(var i=0;i<els.length;i++){els[i].removeAttribute('target');}
            }
            fix();
            try{
                new MutationObserver(function(muts){
                    for(var m=0;m<muts.length;m++){
                        for(var n=0;n<muts[m].addedNodes.length;n++){
                            var node=muts[m].addedNodes[n];
                            if(node.nodeType===1){
                                if(node.matches&&node.matches('[target="_blank"]')) node.removeAttribute('target');
                                if(node.querySelectorAll){
                                    var as=node.querySelectorAll('[target="_blank"]');
                                    for(var i=0;i<as.length;i++){as[i].removeAttribute('target');}
                                }
                            }
                        }
                    }
                }).observe(document.documentElement,{childList:true,subtree:true});
            }catch(e){console.log('[ANTI_BLANK] MO error:',e);}
        })();

        // Catch form submit & clear dynamic target
        document.addEventListener('submit',function(e){
            if(e.target&&e.target.target==='_blank')e.target.target='';
        },true);
        var origFormSubmit=HTMLFormElement.prototype.submit;
        HTMLFormElement.prototype.submit=function(){
            if(this.target==='_blank')this.target='';
            return origFormSubmit.apply(this,arguments);
        };

        // 2. Ctrl+Click
        document.addEventListener('click',function(e){
            var link=e.target.closest('a');
            if(link&&link.href&&(e.ctrlKey||e.metaKey)){
                e.preventDefault();
                try{window.ipc.postMessage(JSON.stringify({type:'ctrl-click',url:link.href}));}catch(_){}
            }
        },true);

        // 3. Override window.open
        window.open=function(url){if(url)window.location.href=url;return null;};

        // 4. pushState/replaceState
        (function(h){
            var ps=h.pushState,rs=h.replaceState;
            function sc(){try{window.ipc.postMessage(JSON.stringify({type:'pushstate',url:location.href}));}catch(_){}}
            h.pushState=function(){ps.apply(h,arguments);sc();};
            h.replaceState=function(){rs.apply(h,arguments);sc();};
            window.addEventListener('popstate',sc);
        })(window.history);

        // 5. Intercept search form submit
        document.addEventListener('submit',function(e){
            var f=e.target;
            if(f&&f.tagName==='FORM'&&f.action&&f.querySelector('[name="wd"]')){
                e.preventDefault();
                var wd=f.querySelector('[name="wd"]').value;
                if(wd)window.location.href=f.action+'?wd='+encodeURIComponent(wd);
            }
        },true);
        document.addEventListener('keydown',function(e){
            if(e.key==='Enter'){
                var input=e.target.closest('#kw,input[name="wd"]');
                if(input&&input.value){
                    e.preventDefault();
                    window.location.href='/s?wd='+encodeURIComponent(input.value);
                }
            }
        },true);

        // 6. Poll location.href
        var lastHref=location.href;
        setInterval(function(){
            var h=location.href;
            if(h!==lastHref){lastHref=h;try{window.ipc.postMessage(JSON.stringify({type:'pushstate',url:h}));}catch(_){}}
        },500);

        console.log('[ANTI_BLANK] handlers installed');
    });
})();
"#;

use eframe::{egui::Rect, CreationContext};
use raw_window_handle::{
    DisplayHandle, HandleError, HasDisplayHandle, HasWindowHandle, RawDisplayHandle,
    RawWindowHandle, WindowHandle,
};
#[cfg(target_os = "windows")]
use wry::{PageLoadEvent, WebView, WebViewBuilder};
use wry::dpi::{LogicalPosition, LogicalSize};
use wry::Rect as WryRect;
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
    applied_source: String,
    is_url: bool,
    last_zoom: f32,
}

/// Event type for webview navigation tracking.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::app) enum NavEvent {
    /// A navigation STARTED (from NavigationStarting). The webview is already navigating.
    Navigating { node_id: usize, url: String },
}

/// IPC event from JavaScript (via window.ipc.postMessage)
#[derive(Debug, Clone)]
pub(in crate::app) struct IpcEvent {
    pub node_id: usize,
    pub url: String,
}

pub(in crate::app) struct HtmlWebViewHost {
    #[cfg(target_os = "windows")]
    webviews: HashMap<usize, HtmlWebViewInstance>,
    /// Receiver for navigation events
    #[cfg(target_os = "windows")]
    nav_rx: mpsc::Receiver<NavEvent>,
    #[cfg(target_os = "windows")]
    nav_tx: mpsc::Sender<NavEvent>,
    #[cfg(target_os = "windows")]
    ipc_rx: Option<mpsc::Receiver<IpcEvent>>,
    #[cfg(target_os = "windows")]
    ipc_tx: mpsc::Sender<IpcEvent>,
}

impl HtmlWebViewHost {
    pub(in crate::app) fn new() -> Self {
        let (nav_tx, nav_rx) = mpsc::channel();
        let (ipc_tx, ipc_rx) = mpsc::channel::<IpcEvent>();
        Self {
            webviews: HashMap::new(),
            nav_rx,
            nav_tx,
            ipc_rx: Some(ipc_rx),
            ipc_tx,
        }
    }

    #[cfg(target_os = "windows")]
    pub(in crate::app) fn drain_nav_events(&mut self) -> Vec<NavEvent> {
        let mut events = Vec::new();
        while let Ok(event) = self.nav_rx.try_recv() {
            events.push(event);
        }
        events
    }

    #[cfg(not(target_os = "windows"))]
    pub(in crate::app) fn drain_nav_events(&mut self) -> Vec<NavEvent> {
        Vec::new()
    }

    /// Navigate a specific node's webview to a new URL
    #[cfg(target_os = "windows")]
    pub(in crate::app) fn navigate_to(&mut self, node_id: usize, url: &str) {
        if let Some(instance) = self.webviews.get_mut(&node_id) {
            let _ = instance.webview.load_url(url);
            instance.applied_source = url.to_owned();
            instance.is_url = true;
        }
    }

    #[cfg(not(target_os = "windows"))]
    pub(in crate::app) fn navigate_to(&mut self, _node_id: usize, _url: &str) {}

    /// Called when the webview itself navigated (e.g. via hyperlink click).
    /// Updates applied_source so the next sync_webview doesn't redundantly call load_url.
    #[cfg(target_os = "windows")]
    pub(in crate::app) fn on_navigated(&mut self, node_id: usize, new_url: &str) {
        if let Some(instance) = self.webviews.get_mut(&node_id) {
            instance.applied_source = new_url.to_owned();
            instance.is_url = true;
        }
    }

    #[cfg(not(target_os = "windows"))]
    pub(in crate::app) fn on_navigated(&mut self, _node_id: usize, _new_url: &str) {}

    /// Inject the anti-blank JS into a specific webview.
    /// Safe to call any time — the JS guard (__antiBlankInstalled) prevents duplicate setup.
    #[cfg(target_os = "windows")]
    pub(in crate::app) fn inject_anti_blank(&mut self, node_id: usize) {
        if let Some(instance) = self.webviews.get_mut(&node_id) {
            if instance.is_url {
                if let Err(e) = instance.webview.evaluate_script(ANTI_BLANK_JS) {
                    eprintln!("[JS_INJECT] node={node_id} failed: {e:?}");
                }
            }
        }
    }

    #[cfg(not(target_os = "windows"))]
    pub(in crate::app) fn inject_anti_blank(&mut self, _node_id: usize) {}
}

impl HtmlWebViewHost {

    #[cfg(target_os = "windows")]
    pub(in crate::app) fn sync_webview(
        &mut self,
        node_id: usize,
        source: &str,
        screen_rect: Rect,
        handles: &HtmlHostHandles,
        is_url: bool,
        zoom_scale: f32,
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
            // Only sync zoom when it changes, to avoid unnecessary WebView2 re-renders
            let rounded_zoom = (zoom_scale * 10.0).round() / 10.0; // round to 0.1
            let clamped_zoom = rounded_zoom.clamp(0.1, 5.0);
            if (instance.last_zoom - clamped_zoom).abs() > 0.01 {
                let _ = instance.webview.zoom(clamped_zoom as f64);
                instance.last_zoom = clamped_zoom;
            }
            // Inject anti-blank script every frame. It's async and harmless if already injected.
            // This ensures it runs on every page, even after user-initiated navigation.
            // Fallback: inject anti-blank JS every frame (idempotent via __antiBlankInstalled).
            if is_url {
                if let Err(e) = instance.webview.evaluate_script(ANTI_BLANK_JS) {
                    eprintln!("[JS_INJECT] frame evaluate_script failed: {e:?}");
                }
            }

            if instance.applied_source != source || instance.is_url != is_url {
                if is_url && !source.is_empty() {
                    let _ = instance.webview.load_url(source);
                } else {
                    let _ = instance.webview.load_html(source);
                }
                instance.applied_source = source.to_owned();
                instance.is_url = is_url;
            }
        } else {
            let rounded_zoom = (zoom_scale * 10.0).round() / 10.0;
            let clamped_zoom = rounded_zoom.clamp(0.1, 5.0);

            // Set up navigation handler to detect URL changes.
            // We use with_navigation_handler instead of with_on_page_load_handler because
            // the latter's ContentLoading handler calls Self::url_from_webview()? which
            // can throw COM errors during navigation, potentially interfering with WebView2.
            let nav_tx_nav = self.nav_tx.clone();
            let nav_tx_fin = self.nav_tx.clone();
            let nav_tx_ipc = self.nav_tx.clone();
            let ipc_tx = self.ipc_tx.clone();

            // Load initial content via the builder (during init_webview / build_as_child)
            let mut builder = WebViewBuilder::new();
            if is_url && !source.is_empty() {
                builder = builder.with_url(source);
            } else if !source.is_empty() {
                builder = builder.with_html(source);
            }

            // Inject anti-blank JS into every new document automatically.
            // This uses WebView2's AddScriptToExecuteOnDocumentCreated under the hood,
            // so it runs BEFORE page content — no timing issues with evaluate_script.
            builder = builder.with_initialization_script(ANTI_BLANK_JS);

            let Ok(wv) = builder
                .with_bounds(bounds)
                .with_browser_accelerator_keys(true)
                .with_devtools(true)
                // Track URL changes when user clicks links:
                // NavigationStarting fires when ANY navigation starts (link click, load_url, etc.)
                // Returns true to allow navigation, false to block
                .with_navigation_handler(move |url| {
                    eprintln!("[NAV] node={node_id} url={url}");
                    let _ = nav_tx_nav.send(NavEvent::Navigating {
                        node_id,
                        url: url.clone(),
                    });
                    true // allow all navigation
                })
                // Fallback: detect navigation completion (catches JS-initiated navigations)
                .with_on_page_load_handler(move |event, url| {
                    if matches!(event, PageLoadEvent::Finished) {
                        eprintln!("[FIN] node={node_id} url={url}");
                        let _ = nav_tx_fin.send(NavEvent::Navigating {
                            node_id,
                            url,
                        });
                    }
                })
                // Listen for Ctrl+Click / Cmd+Click from JavaScript (via IPC)
.with_ipc_handler(move |req| {
                    let body = req.body();
                    if let Ok(val) = serde_json::from_str::<serde_json::Value>(body) {
                        let msg_type = val.get("type").and_then(|v| v.as_str()).unwrap_or("");
                        match msg_type {
                            "ctrl-click" => {
                                if let Some(url) = val.get("url").and_then(|v| v.as_str()) {
                                    eprintln!("[IPC] Ctrl+Click node={node_id} url={url}");
                                    let _ = ipc_tx.send(IpcEvent {
                                        node_id,
                                        url: url.to_owned(),
                                    });
                                }
                            }
                            "pushstate" => {
                                if let Some(url) = val.get("url").and_then(|v| v.as_str()) {
                                    eprintln!("[IPC] pushState node={node_id} url={url}");
                                    // Treat like a navigation event to update the address bar
                                    let _ = nav_tx_ipc.send(NavEvent::Navigating {
                                        node_id,
                                        url: url.to_owned(),
                                    });
                                }
                            }
                            "debug" => {
                                if let Some(msg) = val.get("msg").and_then(|v| v.as_str()) {
                                    eprintln!("[IPC-debug] node={node_id} msg={msg}");
                                    if let Some(url) = val.get("url").and_then(|v| v.as_str()) {
                                        eprintln!("[IPC-debug]   url={url}");
                                    }
                                    if let Some(target) = val.get("target").and_then(|v| v.as_str()) {
                                        eprintln!("[IPC-debug]   target={target}");
                                    }
                                    if let Some(wd) = val.get("wd").and_then(|v| v.as_str()) {
                                        eprintln!("[IPC-debug]   wd={wd}");
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                })
                .build_as_child(handles)
            else {
                return;
            };
            // Set initial zoom
            let _ = wv.zoom(clamped_zoom as f64);
            // Also run evaluate_script immediately as a fallback
            if let Err(e) = wv.evaluate_script(ANTI_BLANK_JS) {
                eprintln!("[JS_INJECT] initial evaluate_script failed: {e:?}");
            }
            self.webviews.insert(
                node_id,
                HtmlWebViewInstance {
                    webview: wv,
                    applied_source: source.to_owned(),
                    is_url,
                    last_zoom: clamped_zoom,
                },
            );
        }
    }

    #[cfg(not(target_os = "windows"))]
    pub(in crate::app) fn sync_webview(
        &mut self,
        _node_id: usize,
        _source: &str,
        _screen_rect: Rect,
        _handles: &HtmlHostHandles,
        _is_url: bool,
        _zoom_scale: f32,
    ) {}

    #[cfg(target_os = "windows")]
    pub(in crate::app) fn remove_webview(&mut self, node_id: usize) {
        self.webviews.remove(&node_id);
    }

    #[cfg(not(target_os = "windows"))]
    pub(in crate::app) fn remove_webview(&mut self, _node_id: usize) {}

    /// Drain IPC events (Ctrl+Click from JavaScript)
    pub(in crate::app) fn drain_ipc_events(&mut self) -> Vec<IpcEvent> {
        #[cfg(target_os = "windows")]
        {
            let mut events = Vec::new();
            if let Some(ref rx) = self.ipc_rx {
                while let Ok(event) = rx.try_recv() {
                    events.push(event);
                }
            }
            return events;
        }
        #[cfg(not(target_os = "windows"))]
        {
            Vec::new()
        }
    }

    pub(in crate::app) fn clear_all(&mut self) {
        #[cfg(target_os = "windows")]
        {
            self.webviews.clear();
        }
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
