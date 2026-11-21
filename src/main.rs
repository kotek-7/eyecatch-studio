mod render;

use anyhow::{Context, Result};
use eframe::egui;
use render::Preset;
use std::{
    path::{Path, PathBuf},
    sync::mpsc,
};

struct Studio {
    root: PathBuf,
    presets: Vec<Preset>,
    selected: usize,
    query: String,
    output: String,
    import_path: String,
    mark: Option<image::RgbaImage>,
    texture: Option<egui::TextureHandle>,
    dirty: bool,
    unsaved: bool,
    status: String,
    worker: Option<mpsc::Receiver<Result<usize, String>>>,
}

impl Studio {
    fn new(root: PathBuf, presets: Vec<Preset>) -> Self {
        let output = root.join("output").display().to_string();
        Self {
            root,
            presets,
            selected: 0,
            query: String::new(),
            output,
            import_path: String::new(),
            mark: None,
            texture: None,
            dirty: true,
            unsaved: false,
            status: String::new(),
            worker: None,
        }
    }

    fn save(&mut self) -> Result<()> {
        for preset in &self.presets {
            render::color(&preset.background)?;
            if let Some(color) = &preset.foreground {
                render::color(color)?;
            }
        }
        let temporary = self.root.join("catalog.json.tmp");
        std::fs::write(
            &temporary,
            serde_json::to_string_pretty(&self.presets)? + "\n",
        )?;
        std::fs::rename(temporary, self.root.join("catalog.json"))?;
        self.unsaved = false;
        Ok(())
    }

    fn import(&mut self, path: &Path) -> Result<()> {
        // Validate before copying or adding a catalog entry.
        let mark = render::load_mark(path)?;
        let ext = path
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("png")
            .to_ascii_lowercase();
        anyhow::ensure!(
            ["svg", "png", "jpg", "jpeg", "webp"].contains(&ext.as_str()),
            "SVG / PNG / JPEG / WebP を選んでください"
        );
        let name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("Imported mark")
            .to_string();
        let base: String = name
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() {
                    c.to_ascii_lowercase()
                } else {
                    '-'
                }
            })
            .collect();
        let base = if base.trim_matches('-').is_empty() {
            "imported"
        } else {
            base.trim_matches('-')
        };
        let mut id = base.to_string();
        let mut number = 2;
        while self.presets.iter().any(|p| p.id == id)
            || self.root.join(format!("assets/{id}.{ext}")).exists()
        {
            id = format!("{base}-{number}");
            number += 1;
        }
        let asset = format!("assets/{id}.{ext}");
        std::fs::copy(path, self.root.join(&asset))?;
        self.presets.push(Preset {
            id,
            name,
            group: "Imported".into(),
            asset,
            background: "#E5EAF1".into(),
            foreground: None,
            scale: 1.0,
            source: "Local import".into(),
        });
        self.selected = self.presets.len() - 1;
        self.mark = Some(mark);
        self.texture = None;
        self.dirty = true;
        self.unsaved = true;
        self.query.clear();
        Ok(())
    }

    fn refresh(&mut self, ctx: &egui::Context) -> Result<()> {
        let preset = &self.presets[self.selected];
        if self.mark.is_none() {
            self.mark = Some(render::load_mark(&self.root.join(&preset.asset))?);
        }
        let canvas = render::compose(self.mark.as_ref().unwrap(), preset)?;
        let preview =
            image::imageops::resize(&canvas, 960, 540, image::imageops::FilterType::Triangle);
        self.texture = Some(ctx.load_texture(
            "preview",
            egui::ColorImage::from_rgba_unmultiplied([960, 540], preview.as_raw()),
            egui::TextureOptions::LINEAR,
        ));
        Ok(())
    }

    fn export_current(&self) -> Result<()> {
        render::export(
            &self.root,
            Path::new(&self.output),
            &self.presets[self.selected],
        )
    }

    fn export_all(&mut self) {
        let (tx, rx) = mpsc::channel();
        let root = self.root.clone();
        let output = PathBuf::from(&self.output);
        let presets = self.presets.clone();
        std::thread::spawn(move || {
            let result = render::export_all(&root, &output, &presets)
                .map(|()| presets.len())
                .map_err(|e| format!("{e:#}"));
            let _ = tx.send(result);
        });
        self.worker = Some(rx);
        self.status = "画像を書き出しています…".into();
    }

    fn report(&mut self, result: Result<()>, success: &str) {
        self.status = match result {
            Ok(()) => success.into(),
            Err(e) => format!("{e:#}"),
        };
    }
}

fn edit_color(ui: &mut egui::Ui, label: &str, value: &mut String) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.label(label);
        // Keep widget positions and IDs stable while a hex value is incomplete.
        let mut rgb = render::color(value).unwrap_or([128; 3]);
        if ui.color_edit_button_srgb(&mut rgb).changed() {
            *value = format!("#{:02X}{:02X}{:02X}", rgb[0], rgb[1], rgb[2]);
            changed = true;
        }
        changed |= ui
            .add(egui::TextEdit::singleline(value).desired_width(85.0))
            .changed();
    });
    changed
}

impl eframe::App for Studio {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        if let Some(worker) = &self.worker {
            if let Ok(result) = worker.try_recv() {
                self.status = match result {
                    Ok(n) => format!("{n}枚を {} に保存しました", self.output),
                    Err(e) => e,
                };
                self.worker = None;
            } else {
                ctx.request_repaint_after(std::time::Duration::from_millis(100));
            }
        }
        for path in ctx.input(|i| {
            i.raw
                .dropped_files
                .iter()
                .map(|f| f.path().to_path_buf())
                .collect::<Vec<_>>()
        }) {
            let result = self.import(&path);
            self.report(
                result,
                "素材を追加しました。設定を保存すると一覧に残ります。",
            );
        }
        egui::Panel::top("header").show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.heading("Eyecatch Studio");
                ui.label("1600 × 900 / PNG");
                if ui.button("設定を保存").clicked() {
                    let result = self.save();
                    self.report(result, "設定を保存しました");
                }
                if self.unsaved {
                    ui.label("● 未保存");
                }
            });
        });
        egui::Panel::bottom("footer").show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label("保存先");
                ui.add(egui::TextEdit::singleline(&mut self.output).desired_width(430.0));
                ui.add_enabled_ui(self.worker.is_none(), |ui| {
                    if ui.button("この画像を出力").clicked() {
                        let result = self.export_current();
                        self.report(result, "PNGを保存しました");
                    }
                    if ui.button("すべて出力").clicked() {
                        self.export_all();
                    }
                });
            });
            if self.worker.is_some() {
                ui.spinner();
            }
            ui.label(&self.status);
        });
        egui::Panel::left("catalog")
            .exact_size(225.0)
            .show(ui, |ui| {
                ui.label(format!("素材 / {} プリセット", self.presets.len()));
                ui.add(egui::TextEdit::singleline(&mut self.query).hint_text("名前・分野で検索"));
                ui.separator();
                let query = self.query.to_lowercase();
                egui::ScrollArea::vertical().show(ui, |ui| {
                    for (index, p) in self.presets.iter().enumerate() {
                        if !format!("{} {} {}", p.name, p.group, p.id)
                            .to_lowercase()
                            .contains(&query)
                        {
                            continue;
                        }
                        ui.horizontal(|ui| {
                            let rgb = render::color(&p.background).unwrap_or([128; 3]);
                            ui.colored_label(egui::Color32::from_rgb(rgb[0], rgb[1], rgb[2]), "●");
                            if ui
                                .selectable_label(self.selected == index, &p.name)
                                .on_hover_text(&p.id)
                                .clicked()
                            {
                                self.selected = index;
                                self.mark = None;
                                self.texture = None;
                                self.dirty = true;
                            }
                        });
                    }
                });
            });
        egui::CentralPanel::default().show(ui, |ui| {
            let p = &mut self.presets[self.selected];
            ui.heading(&p.name);
            ui.horizontal(|ui| {
                let mut changed = edit_color(ui, "背景", &mut p.background);
                if let Some(fg) = &mut p.foreground {
                    changed |= edit_color(ui, "アイコン", fg);
                }
                changed |= ui
                    .add(egui::Slider::new(&mut p.scale, 0.4..=1.6).text("大きさ"))
                    .changed();
                if changed {
                    self.dirty = true;
                    self.unsaved = true;
                }
            });
            if self.dirty {
                self.dirty = false;
                if let Err(error) = self.refresh(&ctx) {
                    self.status = format!("{error:#}");
                } else {
                    self.status.clear();
                }
            }
            if let Some(texture) = &self.texture {
                let width = ui
                    .available_width()
                    .min((ui.available_height() - 105.0).max(180.0) * 16.0 / 9.0);
                ui.add(
                    egui::Image::new(texture)
                        .fit_to_exact_size(egui::vec2(width, width * 9.0 / 16.0)),
                );
            }
            ui.separator();
            ui.label("SVG / PNG をここにドロップ、またはファイルのパスを入力");
            ui.horizontal(|ui| {
                ui.add(egui::TextEdit::singleline(&mut self.import_path).desired_width(420.0));
                if ui.button("素材を追加").clicked() {
                    let path = PathBuf::from(self.import_path.trim());
                    let result = self.import(&path);
                    self.report(result, "素材を追加しました");
                }
            });
            let p = &self.presets[self.selected];
            ui.small(format!("{} / {}", p.group, p.asset));
        });
    }
}

fn main() -> Result<()> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let presets: Vec<Preset> = serde_json::from_slice(&std::fs::read(root.join("catalog.json"))?)?;
    anyhow::ensure!(!presets.is_empty(), "The catalog is empty");
    match std::env::args().nth(1).as_deref() {
        Some("--export-all") => {
            let output = std::env::args()
                .nth(2)
                .map(PathBuf::from)
                .unwrap_or(root.join("output"));
            render::export_all(&root, &output, &presets)?;
            println!("Exported {} images to {}", presets.len(), output.display());
            return Ok(());
        }
        Some("--help") => {
            println!(
                "eyecatch-studio              Open editor\neyecatch-studio --export-all [directory]  Export saved presets"
            );
            return Ok(());
        }
        Some(arg) => anyhow::bail!("Unknown argument: {arg}"),
        None => {}
    }
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1180.0, 790.0])
            .with_min_inner_size([940.0, 650.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Eyecatch Studio",
        options,
        Box::new(move |cc| {
            cc.egui_ctx.set_visuals(egui::Visuals::dark());
            Ok(Box::new(Studio::new(root, presets)))
        }),
    )
    .map_err(|e| anyhow::anyhow!(e.to_string()))
    .context("Cannot start editor")
}
