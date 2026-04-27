use egui::{
    Color32, ColorImage, CornerRadius, Id, Pos2, Rect, RichText, ScrollArea, Sense, Stroke,
    StrokeKind, TextureHandle, TextureOptions, Ui, Vec2,
};
use std::path::{Path, PathBuf};
use walbridge_extract::{
    color::Srgb,
    config::Config,
    extract::{self, Assignment, Extraction, Mutation, RankedCluster},
};

pub struct VisualizerApp {
    config: Option<Config>,
    config_error: Option<String>,
    state: State,
}

enum State {
    Idle,
    Loaded {
        image_path: PathBuf,
        extraction: Extraction,
        /// Decoded image + egui texture; populated lazily on first paint
        /// so we have access to the egui context.
        preview: Option<TextureHandle>,
    },
    Error(String),
}

impl VisualizerApp {
    pub fn new(initial_image: Option<PathBuf>, config_path: Option<PathBuf>) -> Self {
        let effective_config_path = config_path.or_else(Config::default_config_path);
        let (config, config_error) = match Config::load(effective_config_path.as_deref()) {
            Ok(c) => (Some(c), None),
            Err(e) => (None, Some(format!("{e:#}"))),
        };

        let mut app = Self {
            config,
            config_error,
            state: State::Idle,
        };
        if let Some(path) = initial_image {
            app.load_image(&path);
        }
        app
    }

    fn load_image(&mut self, path: &Path) {
        let cfg = match &self.config {
            Some(c) => c.clone(),
            None => {
                self.state = State::Error(
                    self.config_error
                        .clone()
                        .unwrap_or_else(|| "config not loaded".into()),
                );
                return;
            }
        };
        match extract::extract(path, &cfg) {
            Ok(extraction) => {
                self.state = State::Loaded {
                    image_path: path.to_path_buf(),
                    extraction,
                    preview: None,
                };
            }
            Err(e) => {
                self.state = State::Error(format!("extract failed: {e:#}"));
            }
        }
    }

    fn ensure_preview(&mut self, ctx: &egui::Context) {
        let State::Loaded {
            image_path,
            preview,
            ..
        } = &mut self.state
        else {
            return;
        };
        if preview.is_some() {
            return;
        }
        let handle = match load_image_as_texture(ctx, image_path) {
            Ok(h) => h,
            Err(e) => {
                eprintln!("preview load failed: {e:#}");
                return;
            }
        };
        *preview = Some(handle);
    }

    fn pick_image(&mut self) {
        // rfd's synchronous backend internally drives the xdg-portal future
        // with pollster. Don't wrap it in another block_on — that deadlocks.
        let mut dialog = rfd::FileDialog::new()
            .add_filter("images", &["jpg", "jpeg", "png", "webp"])
            .add_filter("all files", &["*"]);
        if let Some(home) = std::env::var_os("HOME") {
            dialog = dialog.set_directory(home);
        }
        if let Some(path) = dialog.pick_file() {
            self.load_image(&path);
        }
    }
}

impl eframe::App for VisualizerApp {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        self.ensure_preview(ui.ctx());

        egui::Panel::left("side_panel")
            .resizable(true)
            .default_size(420.0)
            .size_range(280.0..=640.0)
            .show_inside(ui, |ui| {
                ui.add_space(8.0);
                self.draw_side_panel(ui);
            });

        egui::CentralPanel::default().show_inside(ui, |ui| match &self.state {
            State::Idle => {
                ui.vertical_centered(|ui| {
                    ui.add_space(ui.available_height() * 0.4);
                    ui.label(
                        RichText::new("Open an image to see its palette")
                            .size(18.0)
                            .weak(),
                    );
                });
            }
            State::Error(msg) => {
                ui.vertical_centered(|ui| {
                    ui.add_space(ui.available_height() * 0.4);
                    ui.colored_label(Color32::from_rgb(220, 120, 120), msg);
                });
            }
            State::Loaded { extraction, .. } => {
                ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.add_space(8.0);
                        draw_palette(ui, extraction);
                    });
            }
        });
    }
}

impl VisualizerApp {
    fn draw_side_panel(&mut self, ui: &mut Ui) {
        // Big "Open image" button at top of side panel — primary action.
        let open_button = egui::Button::new(RichText::new("  Open image…  ").size(15.0))
            .min_size(Vec2::new(ui.available_width(), 36.0));
        if ui.add(open_button).clicked() {
            self.pick_image();
        }
        ui.add_space(10.0);

        if let Some(err) = &self.config_error {
            ui.colored_label(
                Color32::from_rgb(220, 120, 120),
                format!("config error: {err}"),
            );
            ui.add_space(6.0);
        }

        match &self.state {
            State::Loaded {
                image_path,
                extraction,
                preview,
            } => {
                draw_image_preview(ui, preview.as_ref());
                ui.add_space(8.0);
                ui.label(
                    RichText::new(
                        image_path
                            .file_name()
                            .map(|s| s.to_string_lossy().into_owned())
                            .unwrap_or_else(|| image_path.display().to_string()),
                    )
                    .size(12.0)
                    .monospace(),
                );
                ui.add_space(12.0);
                draw_extraction_summary(ui, extraction);
            }
            State::Idle | State::Error(_) => {
                ui.add_space(12.0);
                ui.label(RichText::new("No image loaded.").size(12.0).weak());
            }
        }
    }
}

fn draw_image_preview(ui: &mut Ui, texture: Option<&TextureHandle>) {
    let avail_w = ui.available_width();
    match texture {
        Some(tex) => {
            let size = tex.size_vec2();
            let scale = (avail_w / size.x).min(avail_w / size.y).min(1.0);
            let shown = size * scale;
            ui.add(egui::Image::from_texture(tex).fit_to_exact_size(shown));
        }
        None => {
            let (rect, _) = ui.allocate_exact_size(Vec2::splat(avail_w), Sense::hover());
            ui.painter().rect_filled(rect, 6.0, Color32::from_gray(30));
            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                "loading preview…",
                egui::FontId::proportional(12.0),
                Color32::from_gray(140),
            );
        }
    }
}

fn load_image_as_texture(
    ctx: &egui::Context,
    path: &Path,
) -> anyhow::Result<TextureHandle> {
    use anyhow::Context as _;
    let bytes = std::fs::read(path)
        .with_context(|| format!("read `{}`", path.display()))?;
    let img = image::load_from_memory(&bytes)
        .with_context(|| format!("decode `{}`", path.display()))?;
    // Cap at ~1200px longest edge — preview only, saves VRAM.
    let max_edge = 1200u32;
    let (w, h) = (img.width(), img.height());
    let img = if w.max(h) > max_edge {
        let s = max_edge as f32 / w.max(h) as f32;
        img.resize(
            (w as f32 * s) as u32,
            (h as f32 * s) as u32,
            image::imageops::FilterType::Triangle,
        )
    } else {
        img
    };
    let rgba = img.to_rgba8();
    let size = [rgba.width() as usize, rgba.height() as usize];
    let color_image = ColorImage::from_rgba_unmultiplied(size, rgba.as_flat_samples().as_slice());
    Ok(ctx.load_texture("preview", color_image, TextureOptions::LINEAR))
}

fn draw_extraction_summary(ui: &mut Ui, ex: &Extraction) {
    let rejected = ex.clusters.iter().filter(|c| c.rejected_by.is_some()).count();
    ui.label(
        RichText::new(format!(
            "{} clusters · {} rejected",
            ex.clusters.len(),
            rejected
        ))
        .size(12.0)
        .weak(),
    );

    if !ex.blacklist_applied.is_empty() {
        ui.add_space(6.0);
        ui.label(RichText::new("Blacklist applied").size(12.0).strong());
        for name in &ex.blacklist_applied {
            ui.label(
                RichText::new(format!("• {name}"))
                    .size(12.0)
                    .color(Color32::from_rgb(200, 140, 140)),
            );
        }
    }
}

fn draw_palette(ui: &mut Ui, ex: &Extraction) {
    // Special swatches — three wide, equal size, fill row.
    section_heading(ui, "Special");
    let gap = 8.0;
    let row_w = ui.available_width();
    let special_w = ((row_w - gap * 2.0) / 3.0).max(120.0);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = gap;
        swatch_card(ui, "background", &ex.background, special_w, 96.0);
        swatch_card(ui, "foreground", &ex.foreground, special_w, 96.0);
        swatch_card(ui, "cursor", &ex.cursor, special_w, 96.0);
    });

    ui.add_space(16.0);

    // Accents — six across, wraps if narrow.
    section_heading(ui, "Accents");
    accent_grid(ui, ex);

    ui.add_space(16.0);

    // Full 16-slot pywal palette as a compact strip.
    section_heading(ui, "Terminal palette (16 slots)");
    terminal_strip(ui, ex);

    ui.add_space(16.0);

    egui::CollapsingHeader::new(RichText::new("Clusters (by weight)").size(14.0).strong())
        .id_salt(Id::new("clusters_header"))
        .default_open(true)
        .show(ui, |ui| cluster_list(ui, &ex.clusters));
}

fn section_heading(ui: &mut Ui, text: &str) {
    ui.label(RichText::new(text).size(14.0).strong());
    ui.add_space(6.0);
}

fn accent_grid(ui: &mut Ui, ex: &Extraction) {
    let labels = ["red", "green", "yellow", "blue", "magenta", "cyan"];
    let gap = 8.0;
    let row_w = ui.available_width();
    // 3 cells per row on narrow windows, 6 on wide.
    let per_row = if row_w < 640.0 { 3 } else { 6 };
    let cell_w = ((row_w - gap * (per_row as f32 - 1.0)) / per_row as f32).max(96.0);

    for chunk_start in (0..6).step_by(per_row) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = gap;
            for i in chunk_start..(chunk_start + per_row).min(6) {
                swatch_card(ui, labels[i], &ex.accents[i], cell_w, 72.0);
            }
        });
        ui.add_space(gap);
    }
}

fn terminal_strip(ui: &mut Ui, ex: &Extraction) {
    let bg = ex.background.srgb;
    let fg = ex.foreground.srgb;
    let accents = &ex.accents;
    let bright = |c: Srgb| {
        let mut lab = c.to_oklab();
        lab.l = (lab.l + 0.08).clamp(0.0, 1.0);
        lab.to_srgb()
    };
    // Always 2 rows of 8 — top row normal (0-7), bottom row bright (8-15).
    // Each column is a normal/bright pair, so users can see the pairing.
    let rows: [[(u8, Srgb); 8]; 2] = [
        [
            (0, bg),
            (1, accents[0].srgb),
            (2, accents[1].srgb),
            (3, accents[2].srgb),
            (4, accents[3].srgb),
            (5, accents[4].srgb),
            (6, accents[5].srgb),
            (7, fg),
        ],
        [
            (8, bright(bg)),
            (9, bright(accents[0].srgb)),
            (10, bright(accents[1].srgb)),
            (11, bright(accents[2].srgb)),
            (12, bright(accents[3].srgb)),
            (13, bright(accents[4].srgb)),
            (14, bright(accents[5].srgb)),
            (15, bright(fg)),
        ],
    ];

    let gap = 4.0;
    // available_width() in a ScrollArea already excludes the scrollbar, so
    // just divide evenly. min_w guards against truncation when the window
    // is extremely narrow — user can scroll horizontally if it comes to that.
    let row_w = ui.available_width();
    let cell_w = ((row_w - gap * 7.0) / 8.0).max(32.0);
    let cell_h = 44.0;

    for row in &rows {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = gap;
            for (slot, color) in row {
                terminal_cell(ui, &slot.to_string(), *color, cell_w, cell_h);
            }
        });
        ui.add_space(gap);
    }
}

fn terminal_cell(ui: &mut Ui, label: &str, color: Srgb, w: f32, h: f32) {
    let fill = color32_from_srgb(color);
    let (rect, response) = ui.allocate_exact_size(Vec2::new(w, h), Sense::hover());
    ui.painter().rect_filled(rect, 4.0, fill);
    ui.painter().rect_stroke(
        rect,
        CornerRadius::same(4),
        Stroke::new(1.0, Color32::from_gray(60)),
        StrokeKind::Inside,
    );
    // Label baked into the swatch: label in top-left, hex in bottom-right.
    let text_color = if color.to_oklab().l < 0.5 {
        Color32::from_rgba_unmultiplied(255, 255, 255, 220)
    } else {
        Color32::from_rgba_unmultiplied(0, 0, 0, 220)
    };
    ui.painter().text(
        rect.left_top() + Vec2::new(4.0, 2.0),
        egui::Align2::LEFT_TOP,
        label,
        egui::FontId::monospace(10.0),
        text_color,
    );
    ui.painter().text(
        rect.right_bottom() - Vec2::new(4.0, 2.0),
        egui::Align2::RIGHT_BOTTOM,
        color.hex_with_hash(),
        egui::FontId::monospace(10.0),
        text_color,
    );
    response.on_hover_text(format!("slot {label}: {}", color.hex_with_hash()));
}

fn swatch_card(ui: &mut Ui, label: &str, a: &Assignment, width: f32, swatch_h: f32) {
    let text_h = 58.0;
    let total_h = swatch_h + text_h;
    let (rect, _response) = ui.allocate_exact_size(Vec2::new(width, total_h), Sense::hover());

    // Swatch.
    let swatch_rect = Rect::from_min_size(rect.min, Vec2::new(width, swatch_h));
    let fill = color32_from_srgb(a.srgb);
    ui.painter().rect_filled(swatch_rect, 6.0, fill);
    ui.painter().rect_stroke(
        swatch_rect,
        CornerRadius::same(6),
        Stroke::new(1.0, Color32::from_gray(60)),
        StrokeKind::Inside,
    );

    // Text area below swatch.
    let text_top = swatch_rect.max.y + 6.0;
    let mut cursor_y = text_top;
    let pad_x = 2.0;

    ui.painter().text(
        Pos2::new(rect.min.x + pad_x, cursor_y),
        egui::Align2::LEFT_TOP,
        label,
        egui::FontId::proportional(13.0),
        ui.visuals().text_color(),
    );
    cursor_y += 16.0;

    ui.painter().text(
        Pos2::new(rect.min.x + pad_x, cursor_y),
        egui::Align2::LEFT_TOP,
        a.srgb.hex_with_hash(),
        egui::FontId::monospace(11.0),
        ui.visuals().weak_text_color(),
    );
    cursor_y += 15.0;

    let annotation = if let Some(idx) = a.source_cluster {
        let mut parts = vec![format!("#{idx}")];
        let mu = mutation_label(&a.mutation);
        if !mu.is_empty() {
            parts.push(mu);
        }
        parts.join(" · ")
    } else {
        "synthesized".into()
    };
    ui.painter().text(
        Pos2::new(rect.min.x + pad_x, cursor_y),
        egui::Align2::LEFT_TOP,
        annotation,
        egui::FontId::proportional(10.0),
        Color32::from_rgb(180, 160, 90),
    );
}

fn cluster_list(ui: &mut Ui, clusters: &[RankedCluster]) {
    let row_h = 22.0;
    let bar_max = 160.0;

    for (idx, c) in clusters.iter().enumerate() {
        let row_width = ui.available_width();
        let (rect, response) =
            ui.allocate_exact_size(Vec2::new(row_width, row_h), Sense::hover());

        let mut x = rect.min.x;
        let mid_y = rect.center().y;

        // Swatch.
        let sw = 28.0;
        let sh = 18.0;
        let sw_rect = Rect::from_min_size(Pos2::new(x, mid_y - sh / 2.0), Vec2::new(sw, sh));
        ui.painter().rect_filled(sw_rect, 3.0, color32_from_srgb(c.srgb));
        ui.painter().rect_stroke(
            sw_rect,
            CornerRadius::same(3),
            Stroke::new(1.0, Color32::from_gray(60)),
            StrokeKind::Inside,
        );
        x += sw + 8.0;

        // Index + hex, monospace.
        let hex = c.srgb.hex_with_hash();
        let mut text = RichText::new(format!("#{idx:02}  {hex}"))
            .monospace()
            .size(12.0);
        if c.rejected_by.is_some() {
            text = text.strikethrough().color(Color32::from_rgb(200, 140, 140));
        }
        let galley =
            ui.painter()
                .layout_no_wrap(text.text().into(), egui::FontId::monospace(12.0), {
                    if c.rejected_by.is_some() {
                        Color32::from_rgb(200, 140, 140)
                    } else {
                        ui.visuals().text_color()
                    }
                });
        ui.painter()
            .galley(Pos2::new(x, mid_y - galley.size().y / 2.0), galley, Color32::WHITE);
        x += 140.0;

        // Percent, monospace.
        let pct = format!("{:>5.1}%", c.weight * 100.0);
        let pct_galley = ui.painter().layout_no_wrap(
            pct,
            egui::FontId::monospace(12.0),
            ui.visuals().weak_text_color(),
        );
        ui.painter().galley(
            Pos2::new(x, mid_y - pct_galley.size().y / 2.0),
            pct_galley,
            Color32::WHITE,
        );
        x += 58.0;

        // Bar.
        let bar_available = (rect.max.x - x - 8.0).min(bar_max).max(40.0);
        let bar_rect = Rect::from_min_size(Pos2::new(x, mid_y - 4.0), Vec2::new(bar_available, 8.0));
        ui.painter().rect_filled(bar_rect, 2.0, Color32::from_gray(40));
        let filled_w = bar_rect.width() * c.weight.clamp(0.0, 1.0);
        let filled_rect =
            Rect::from_min_size(bar_rect.min, Vec2::new(filled_w, bar_rect.height()));
        let bar_color = if c.rejected_by.is_some() {
            Color32::from_rgb(180, 70, 70)
        } else {
            Color32::from_rgb(110, 160, 200)
        };
        ui.painter().rect_filled(filled_rect, 2.0, bar_color);
        x += bar_available + 8.0;

        // Rejection reason (if present, fills remaining space).
        if let Some(reason) = &c.rejected_by {
            let reason_galley = ui.painter().layout_no_wrap(
                format!("— {reason}"),
                egui::FontId::proportional(11.0),
                Color32::from_rgb(200, 140, 140),
            );
            ui.painter().galley(
                Pos2::new(x, mid_y - reason_galley.size().y / 2.0),
                reason_galley,
                Color32::WHITE,
            );
        }

        let tip = if let Some(reason) = &c.rejected_by {
            format!(
                "{hex} · {:.1}% · rejected: {reason}",
                c.weight * 100.0
            )
        } else {
            format!("{hex} · {:.1}%", c.weight * 100.0)
        };
        response.on_hover_text(tip);
    }
}

fn mutation_label(m: &Mutation) -> String {
    match m {
        Mutation::None => String::new(),
        Mutation::LightnessClamp(l) => format!("L→{l:.2}"),
        Mutation::HueRotate { from, to } => format!("hue {from:.0}°→{to:.0}°"),
        Mutation::ChromaBoost { from, to } => format!("C {from:.2}→{to:.2}"),
        Mutation::Synthesized => "synth".into(),
    }
}

fn color32_from_srgb(c: Srgb) -> Color32 {
    Color32::from_rgb(c.r, c.g, c.b)
}
