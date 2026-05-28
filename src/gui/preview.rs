// gui/preview.rs
// the little preview area at the bottom of the overlay
// shows the full text, or an image thumbnail, or file info depending on the entry type

use egui::{Color32, RichText, ScrollArea, Ui};

use crate::store::{ClipEntry, ContentType};
use super::theme::Palette;

pub struct PreviewPane;

impl PreviewPane {
    pub fn show(ui: &mut Ui, entry: &ClipEntry, palette: &Palette) {
        ui.separator();
        ui.add_space(4.0);

        match entry.content_type {
            ContentType::Text => {
                show_text_preview(ui, entry, palette);
            }
            ContentType::Image => {
                show_image_preview(ui, entry, palette);
            }
            ContentType::FilePath => {
                show_filepath_preview(ui, entry, palette);
            }
        }
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
        ui.label(RichText::new(format!("{chars} chars  ·  {words} words  ·  {lines} lines"))
            .color(palette.text_dim)
            .small());
    });
}

fn show_image_preview(ui: &mut Ui, entry: &ClipEntry, palette: &Palette) {
    ui.label(RichText::new("Image Preview").color(palette.text_dim).small());
    ui.add_space(4.0);

    // decode the base64 webp thumbnail we stored earlier and render it
    use base64::{engine::general_purpose::STANDARD as B64, Engine};
    if let Ok(bytes) = B64.decode(&entry.data) {
        if let Ok(img) = image::load_from_memory(&bytes) {
            let rgba   = img.to_rgba8();
            let (w, h) = rgba.dimensions();
            let color_image = egui::ColorImage::from_rgba_unmultiplied(
                [w as usize, h as usize],
                rgba.as_raw(),
            );
            // ideally we'd cache the texture but reloading each frame is fine for a preview pane
            let texture = ui.ctx().load_texture(
                format!("thumb_{}", &entry.id[..8]),
                color_image,
                egui::TextureOptions::LINEAR,
            );
            let max_size = egui::vec2(ui.available_width(), 160.0);
            let img_size = egui::vec2(w as f32, h as f32);
            let scale    = (max_size.x / img_size.x).min(max_size.y / img_size.y).min(1.0);
            ui.image((texture.id(), img_size * scale));
        } else {
            ui.label(RichText::new("⚠ Could not decode image").color(palette.danger));
        }
    } else {
        ui.label(RichText::new("⚠ Invalid base64 thumbnail").color(palette.danger));
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
        ui.label(RichText::new("✓ File exists").color(Color32::from_rgb(80, 200, 100)).small());
        if let Ok(meta) = std::fs::metadata(path) {
            let size = meta.len();
            let size_str = if size < 1024 {
                format!("{size} B")
            } else if size < 1024 * 1024 {
                format!("{:.1} KB", size as f64 / 1024.0)
            } else {
                format!("{:.1} MB", size as f64 / (1024.0 * 1024.0))
            };
            ui.label(RichText::new(format!("Size: {size_str}")).color(palette.text_dim).small());
        }
    } else {
        ui.label(RichText::new("✗ File not found").color(palette.danger).small());
    }
}
