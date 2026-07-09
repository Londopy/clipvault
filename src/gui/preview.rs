// gui/preview.rs
// the little preview area at the bottom of the overlay
// shows the full text, or an image thumbnail, or file info depending on the entry type

use egui::{RichText, ScrollArea, Ui};

use super::theme::Palette;
use crate::store::{ClipEntry, ContentType};

pub struct PreviewPane;

impl PreviewPane {
    pub fn show(ui: &mut Ui, entry: &ClipEntry, palette: &Palette) {
        ui.add_space(8.0);

        // preview sits in its own rounded card so it reads as a distinct zone
        egui::Frame::none()
            .fill(palette.bg_secondary)
            .rounding(egui::Rounding::same(10.0))
            .stroke(egui::Stroke::new(1.0, palette.border))
            .inner_margin(egui::Margin::same(10.0))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                match entry.content_type {
                    ContentType::Text => show_text_preview(ui, entry, palette),
                    ContentType::Image => show_image_preview(ui, entry, palette),
                    ContentType::FilePath => show_filepath_preview(ui, entry, palette),
                }
            });
    }
}

fn show_text_preview(ui: &mut Ui, entry: &ClipEntry, palette: &Palette) {
    ui.label(RichText::new("Preview").color(palette.text_dim).small());
    ui.add_space(2.0);

    let text_height = ui.available_height().min(180.0);
    ScrollArea::vertical()
        .max_height(text_height)
        .show(ui, |ui| {
            ui.add(
                egui::TextEdit::multiline(&mut entry.data.clone())
                    .desired_width(f32::INFINITY)
                    .font(egui::TextStyle::Monospace)
                    .interactive(false)
                    .frame(false)
                    .text_color(palette.text),
            );
        });

    // little stats row at the bottom showing char/word/line counts
    ui.add_space(4.0);
    let chars = entry.data.chars().count();
    let words = entry.data.split_whitespace().count();
    let lines = entry.data.lines().count();
    ui.horizontal(|ui| {
        ui.label(
            RichText::new(format!("{chars} chars  ·  {words} words  ·  {lines} lines"))
                .color(palette.text_dim)
                .small(),
        );
    });
}

fn show_image_preview(ui: &mut Ui, entry: &ClipEntry, palette: &Palette) {
    ui.label(
        RichText::new("Image Preview")
            .color(palette.text_dim)
            .small(),
    );
    ui.add_space(4.0);

    // decode + upload once per entry and cache the texture - doing the
    // base64 + webp decode and gpu upload every frame burned cpu constantly
    let tex_id = egui::Id::new(("thumb", &entry.id));
    let cached: Option<egui::TextureHandle> = ui.ctx().data_mut(|d| d.get_temp(tex_id));
    let texture = match cached {
        Some(t) => Some(t),
        None => {
            use base64::{engine::general_purpose::STANDARD as B64, Engine};
            B64.decode(&entry.data)
                .ok()
                .and_then(|bytes| image::load_from_memory(&bytes).ok())
                .map(|img| {
                    let rgba = img.to_rgba8();
                    let (w, h) = rgba.dimensions();
                    let color_image = egui::ColorImage::from_rgba_unmultiplied(
                        [w as usize, h as usize],
                        rgba.as_raw(),
                    );
                    let t = ui.ctx().load_texture(
                        format!("thumb_{}", &entry.id[..8]),
                        color_image,
                        egui::TextureOptions::LINEAR,
                    );
                    ui.ctx().data_mut(|d| d.insert_temp(tex_id, t.clone()));
                    t
                })
        }
    };

    if let Some(texture) = texture {
        let size = texture.size_vec2();
        let max_size = egui::vec2(ui.available_width(), 160.0);
        let scale = (max_size.x / size.x).min(max_size.y / size.y).min(1.0);
        ui.image((texture.id(), size * scale));
    } else {
        ui.label(RichText::new("⚠ Could not decode image").color(palette.danger));
    }
}

fn show_filepath_preview(ui: &mut Ui, entry: &ClipEntry, palette: &Palette) {
    let path = std::path::Path::new(&entry.data);
    let exists = path.exists();

    ui.label(RichText::new("File Path").color(palette.text_dim).small());
    ui.add_space(4.0);
    ui.label(RichText::new(&entry.data).color(palette.text).monospace());
    ui.add_space(4.0);

    if exists {
        ui.label(
            RichText::new("✓ File exists")
                .color(palette.success)
                .small(),
        );
        if let Ok(meta) = std::fs::metadata(path) {
            let size = meta.len();
            let size_str = if size < 1024 {
                format!("{size} B")
            } else if size < 1024 * 1024 {
                format!("{:.1} KB", size as f64 / 1024.0)
            } else {
                format!("{:.1} MB", size as f64 / (1024.0 * 1024.0))
            };
            ui.label(
                RichText::new(format!("Size: {size_str}"))
                    .color(palette.text_dim)
                    .small(),
            );
        }
    } else {
        ui.label(
            RichText::new("✗ File not found")
                .color(palette.danger)
                .small(),
        );
    }
}
