//! haneng-settings — 설정 창 (eframe/egui).
//!
//! `config.txt`와 `exceptions.txt`를 편집한다. 데몬은 시작 시에만 설정을
//! 읽으므로, 저장 후 데몬을 재시작해야 적용된다 (창에도 안내 표시).
//! 실시간 토글(활성화/자동)은 데몬 트레이 메뉴가 담당한다.
//!
//! egui 기본 폰트에는 한글이 없어 OS 시스템 폰트를 찾아 싣는다.

#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

use haneng_core::{config, Config, Layout, Sensitivity};

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([460.0, 560.0])
            .with_title("haneng 설정"),
        ..Default::default()
    };
    eframe::run_native(
        "haneng-settings",
        options,
        Box::new(|cc| {
            install_korean_font(&cc.egui_ctx);
            Ok(Box::new(SettingsApp::load()))
        }),
    )
}

/// OS 시스템 한글 폰트를 찾아 egui에 등록한다. 못 찾으면 기본 폰트
/// (한글은 □로 표시)로 동작은 유지된다.
fn install_korean_font(ctx: &eframe::egui::Context) {
    use eframe::egui::{FontData, FontDefinitions, FontFamily};
    let candidates = [
        // macOS
        "/System/Library/Fonts/Supplemental/AppleGothic.ttf",
        "/System/Library/Fonts/AppleSDGothicNeo.ttc",
        // Windows
        "C:\\Windows\\Fonts\\malgun.ttf",
        // Linux
        "/usr/share/fonts/truetype/nanum/NanumGothic.ttf",
        "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
        "/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc",
    ];
    let Some(bytes) = candidates.iter().find_map(|path| std::fs::read(path).ok()) else {
        eprintln!("한글 폰트를 찾지 못했습니다 — 한글이 □로 보일 수 있습니다.");
        return;
    };
    let mut fonts = FontDefinitions::default();
    fonts
        .font_data
        .insert("korean".into(), FontData::from_owned(bytes).into());
    for family in [FontFamily::Proportional, FontFamily::Monospace] {
        fonts
            .families
            .entry(family)
            .or_default()
            .push("korean".into());
    }
    ctx.set_fonts(fonts);
}

struct SettingsApp {
    config: Config,
    disabled_apps_text: String,
    exceptions: Vec<String>,
    new_exception: String,
    status: String,
}

impl SettingsApp {
    fn load() -> Self {
        let config = config::load_config();
        let disabled_apps_text = config.disabled_apps.join(", ");
        Self {
            config,
            disabled_apps_text,
            exceptions: config::load_exceptions(),
            new_exception: String::new(),
            status: String::new(),
        }
    }

    fn save(&mut self) {
        self.config.disabled_apps = self
            .disabled_apps_text
            .split(',')
            .map(|s| s.trim().to_lowercase())
            .filter(|s| !s.is_empty())
            .collect();
        let result = config::save_config(&self.config)
            .and_then(|()| config::save_exceptions(&self.exceptions));
        self.status = match result {
            Ok(()) => format!(
                "저장됨: {} — 데몬을 재시작하면 적용됩니다.",
                config::config_dir()
                    .map(|d| d.display().to_string())
                    .unwrap_or_default()
            ),
            Err(e) => format!("저장 실패: {e}"),
        };
    }
}

impl eframe::App for SettingsApp {
    fn ui(&mut self, ui: &mut eframe::egui::Ui, _frame: &mut eframe::Frame) {
        use eframe::egui;
        {
            ui.heading("haneng — 한/영 오타 교정 설정");
            ui.add_space(8.0);

            ui.checkbox(&mut self.config.auto, "단어 경계 자동 교정 (시작 기본값)");
            ui.add_space(8.0);

            ui.label("자동 교정 민감도");
            ui.horizontal(|ui| {
                for (value, label) in [
                    (Sensitivity::Conservative, "보수적"),
                    (Sensitivity::Balanced, "균형 (권장)"),
                    (Sensitivity::Aggressive, "적극적"),
                ] {
                    ui.radio_value(&mut self.config.sensitivity, value, label);
                }
            });
            ui.add_space(8.0);

            ui.label("한글 자판 배열");
            ui.horizontal(|ui| {
                for (value, label) in [
                    (Layout::Dubeolsik, "두벌식"),
                    (Layout::Sebeolsik390, "세벌식 390"),
                    (Layout::SebeolsikFinal, "세벌식 최종"),
                ] {
                    ui.radio_value(&mut self.config.layout, value, label);
                }
            });
            ui.add_space(8.0);

            ui.label("교정을 끌 앱 (쉼표로 구분, 이름 일부만 써도 됨)");
            ui.text_edit_singleline(&mut self.disabled_apps_text);
            ui.add_space(12.0);

            ui.separator();
            ui.label(format!(
                "예외 사전 — 자동 변환하지 않는 단어 ({}개)",
                self.exceptions.len()
            ));
            ui.small("자동 교정을 백스페이스로 되돌리면 자동으로 학습됩니다.");
            egui::ScrollArea::vertical()
                .max_height(160.0)
                .show(ui, |ui| {
                    let mut remove: Option<usize> = None;
                    for (i, word) in self.exceptions.iter().enumerate() {
                        ui.horizontal(|ui| {
                            if ui.small_button("삭제").clicked() {
                                remove = Some(i);
                            }
                            ui.label(word);
                        });
                    }
                    if let Some(i) = remove {
                        self.exceptions.remove(i);
                    }
                });
            ui.horizontal(|ui| {
                ui.text_edit_singleline(&mut self.new_exception);
                if ui.button("추가").clicked() {
                    let word = self.new_exception.trim().to_string();
                    if !word.is_empty() && !self.exceptions.contains(&word) {
                        self.exceptions.push(word);
                    }
                    self.new_exception.clear();
                }
            });
            ui.add_space(12.0);

            ui.separator();
            ui.horizontal(|ui| {
                if ui.button("저장").clicked() {
                    self.save();
                }
                if ui.button("다시 불러오기").clicked() {
                    *self = Self::load();
                }
            });
            if !self.status.is_empty() {
                ui.add_space(4.0);
                ui.small(&self.status);
            }
        }
    }
}
