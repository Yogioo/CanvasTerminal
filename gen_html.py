import os
lines = []
lines.append("use std::collections::HashMap;")
lines.append("use std::sync::mpsc;")
lines.append("")
lines.append("use eframe::{egui::Rect, CreationContext};")
lines.append("use raw_window_handle::{")
lines.append("    DisplayHandle, HandleError, HasDisplayHandle, HasWindowHandle, RawDisplayHandle,")
lines.append("    RawWindowHandle, WindowHandle,")
lines.append("};")

with open(os.path.join(os.getcwd(), "src", "app", "html_webview.rs"), "w") as f:
    f.write(chr(10).join(lines))
print("ok")
