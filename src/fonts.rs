use eframe::egui;
use std::path::{Path, PathBuf};

fn load_font_from_candidates(
    fonts: &mut egui::FontDefinitions,
    family: egui::FontFamily,
    font_name: &str,
    candidates: &[PathBuf],
    insert_at_start: bool,
) -> Option<String> {
    for path in candidates {
        if let Ok(bytes) = std::fs::read(path) {
            let name = font_name.to_owned();
            fonts
                .font_data
                .insert(name.clone(), egui::FontData::from_owned(bytes).into());

            let family_entry = fonts.families.entry(family).or_default();
            if insert_at_start {
                family_entry.insert(0, name.clone());
            } else {
                family_entry.push(name.clone());
            }
            return Some(path.display().to_string());
        }
    }

    None
}

fn app_font_candidates(file_names: &[&str]) -> Vec<PathBuf> {
    let mut paths = Vec::new();

    // 开发环境运行：项目根目录/assets/fonts
    for file in file_names {
        paths.push(Path::new("assets").join("fonts").join(file));
    }

    // 打包后运行：exe 同目录/fonts
    if let Ok(exe) = std::env::current_exe() {
        if let Some(exe_dir) = exe.parent() {
            for file in file_names {
                paths.push(exe_dir.join("fonts").join(file));
            }
        }
    }

    paths
}

pub fn setup_custom_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();

    // 1) 主字体：优先加载随应用发布的 Nerd Font（保证终端图标一致）
    //    文件名按打包目录约定，可按需增减。
    let bundled_mono_candidates = app_font_candidates(&[
        "CaskaydiaCoveNerdFont-Regular.ttf",
        "CaskaydiaCoveNerdFontMono-Regular.ttf",
        "JetBrainsMonoNerdFont-Regular.ttf",
        "MesloLGS NF Regular.ttf",
    ]);

    // 2) 兜底等宽字体：系统字体
    let system_mono_candidates = vec![
        PathBuf::from("C:/Windows/Fonts/CascadiaMono.ttf"),
        PathBuf::from("C:/Windows/Fonts/CascadiaCode.ttf"),
        PathBuf::from("C:/Windows/Fonts/consola.ttf"),
        PathBuf::from("C:/Windows/Fonts/consolas.ttf"),
    ];

    // 3) Emoji fallback：系统 Segoe UI Emoji（不要随应用分发）
    let emoji_candidates = vec![
        PathBuf::from("C:/Windows/Fonts/seguiemj.ttf"),
        PathBuf::from("C:/Windows/Fonts/SegoeUIEmoji.ttf"),
    ];

    // 4) CJK fallback：系统中文字体
    let cjk_candidates = vec![
        PathBuf::from("C:/Windows/Fonts/msyh.ttc"), // Microsoft YaHei
        PathBuf::from("C:/Windows/Fonts/simhei.ttf"), // SimHei
        PathBuf::from("C:/Windows/Fonts/simsun.ttc"), // SimSun
    ];

    let mono_loaded = load_font_from_candidates(
        &mut fonts,
        egui::FontFamily::Monospace,
        "term_mono",
        &bundled_mono_candidates,
        true,
    )
    .or_else(|| {
        load_font_from_candidates(
            &mut fonts,
            egui::FontFamily::Monospace,
            "term_mono",
            &system_mono_candidates,
            true,
        )
    });

    let emoji_loaded = load_font_from_candidates(
        &mut fonts,
        egui::FontFamily::Monospace,
        "emoji_fallback",
        &emoji_candidates,
        false,
    );

    let cjk_loaded = load_font_from_candidates(
        &mut fonts,
        egui::FontFamily::Proportional,
        "cjk_fallback",
        &cjk_candidates,
        false,
    );

    // 终端里中文作为 fallback，不要抢占等宽字体首位。
    if fonts.font_data.contains_key("cjk_fallback") {
        fonts
            .families
            .entry(egui::FontFamily::Monospace)
            .or_default()
            .push("cjk_fallback".to_owned());
    }

    // UI 文本里 emoji 也作为 fallback，避免方块字。
    if fonts.font_data.contains_key("emoji_fallback") {
        fonts
            .families
            .entry(egui::FontFamily::Proportional)
            .or_default()
            .push("emoji_fallback".to_owned());
    }

    ctx.set_fonts(fonts);

    eprintln!(
        "Font setup => mono: {}, emoji: {}, cjk: {}",
        mono_loaded.unwrap_or_else(|| "<default>".to_string()),
        emoji_loaded.unwrap_or_else(|| "<none>".to_string()),
        cjk_loaded.unwrap_or_else(|| "<none>".to_string())
    );
}
