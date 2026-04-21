use eframe::egui;

pub fn setup_custom_fonts(ctx: &egui::Context) {
    // 关键点：终端必须优先用等宽字体；中文字体只做 fallback。
    let mono_candidates = [
        "C:/Windows/Fonts/CascadiaMono.ttf",
        "C:/Windows/Fonts/CascadiaCode.ttf",
        "C:/Windows/Fonts/consola.ttf",
        "C:/Windows/Fonts/consolas.ttf",
    ];

    let cjk_candidates = [
        "C:/Windows/Fonts/msyh.ttc",   // Microsoft YaHei
        "C:/Windows/Fonts/simhei.ttf", // SimHei
        "C:/Windows/Fonts/simsun.ttc", // SimSun
    ];

    let mut fonts = egui::FontDefinitions::default();

    let mut mono_loaded = None::<String>;
    for path in mono_candidates {
        if let Ok(bytes) = std::fs::read(path) {
            let name = "term_mono".to_owned();
            fonts
                .font_data
                .insert(name.clone(), egui::FontData::from_owned(bytes).into());
            fonts
                .families
                .entry(egui::FontFamily::Monospace)
                .or_default()
                .insert(0, name.clone());
            mono_loaded = Some(path.to_owned());
            break;
        }
    }

    let mut cjk_loaded = None::<String>;
    for path in cjk_candidates {
        if let Ok(bytes) = std::fs::read(path) {
            let name = "cjk".to_owned();
            fonts
                .font_data
                .insert(name.clone(), egui::FontData::from_owned(bytes).into());

            // UI 文本优先中文字体，避免方块字。
            fonts
                .families
                .entry(egui::FontFamily::Proportional)
                .or_default()
                .insert(0, name.clone());

            // 终端里中文作为 fallback，不要抢占等宽字体首位。
            fonts
                .families
                .entry(egui::FontFamily::Monospace)
                .or_default()
                .push(name.clone());

            cjk_loaded = Some(path.to_owned());
            break;
        }
    }

    ctx.set_fonts(fonts);

    eprintln!(
        "Font setup => mono: {}, cjk: {}",
        mono_loaded.unwrap_or_else(|| "<default>".to_string()),
        cjk_loaded.unwrap_or_else(|| "<none>".to_string())
    );
}
