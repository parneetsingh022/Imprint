//! Multi-document model with tab management, cropping, and undo/redo history.
//!
//! Provides document encapsulation for open portrait photographs,
//! Photoshop-style crop execution, and non-destructive undo/redo history stacks.

use crate::image_ops::{CropArea, ImageOpsError, LoadedImage, crop_to_dynamic_image};
use ::image::GenericImageView;
use eframe::egui;

/// Snapshot of document state for undo/redo history.
#[derive(Clone)]
pub struct DocumentSnapshot {
    pub loaded_image: LoadedImage,
    pub crop_area: CropArea,
}

/// An open image document representing a single tab.
pub struct ImageDocument {
    pub id: usize,
    pub title: String,
    pub loaded_image: LoadedImage,
    pub source_texture: Option<egui::TextureHandle>,
    pub cropped_texture: Option<egui::TextureHandle>,
    pub crop_area: CropArea,
    pub texture_needs_update: bool,
    pub undo_stack: Vec<DocumentSnapshot>,
    pub redo_stack: Vec<DocumentSnapshot>,
}

impl ImageDocument {
    /// Creates a new image document tab.
    pub fn new(id: usize, loaded: LoadedImage, target_aspect: f32) -> Self {
        let crop_area = CropArea::default_for_aspect(loaded.width, loaded.height, target_aspect);
        let title = loaded.file_name.clone();
        Self {
            id,
            title,
            loaded_image: loaded,
            source_texture: None,
            cropped_texture: None,
            crop_area,
            texture_needs_update: true,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
        }
    }

    /// Returns `true` if an undo operation is available.
    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    /// Returns `true` if a redo operation is available.
    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    /// Restores the most recent snapshot from the undo stack.
    pub fn undo(&mut self) -> bool {
        if let Some(snapshot) = self.undo_stack.pop() {
            let current = DocumentSnapshot {
                loaded_image: self.loaded_image.clone(),
                crop_area: self.crop_area,
            };
            self.redo_stack.push(current);

            self.loaded_image = snapshot.loaded_image;
            self.crop_area = snapshot.crop_area;
            self.source_texture = None;
            self.cropped_texture = None;
            self.texture_needs_update = true;
            true
        } else {
            false
        }
    }

    /// Restores the most recent snapshot from the redo stack.
    pub fn redo(&mut self) -> bool {
        if let Some(snapshot) = self.redo_stack.pop() {
            let current = DocumentSnapshot {
                loaded_image: self.loaded_image.clone(),
                crop_area: self.crop_area,
            };
            self.undo_stack.push(current);

            self.loaded_image = snapshot.loaded_image;
            self.crop_area = snapshot.crop_area;
            self.source_texture = None;
            self.cropped_texture = None;
            self.texture_needs_update = true;
            true
        } else {
            false
        }
    }

    /// Commits the current crop area to the underlying image (Photoshop-style crop execution).
    /// Saves the pre-crop state to the undo stack and resizes the image to the cropped pixel bounds.
    pub fn commit_crop(&mut self, target_aspect: f32) -> Result<(), ImageOpsError> {
        let snapshot = DocumentSnapshot {
            loaded_image: self.loaded_image.clone(),
            crop_area: self.crop_area,
        };
        self.undo_stack.push(snapshot);
        self.redo_stack.clear();

        let cropped = crop_to_dynamic_image(&self.loaded_image.original, &self.crop_area)?;
        let (new_w, new_h) = cropped.dimensions();
        self.loaded_image.original = cropped;
        self.loaded_image.width = new_w;
        self.loaded_image.height = new_h;

        self.crop_area = CropArea::default_for_aspect(new_w, new_h, target_aspect);
        self.source_texture = None;
        self.cropped_texture = None;
        self.texture_needs_update = true;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ::image::{DynamicImage, RgbaImage};

    #[test]
    fn test_document_creation() {
        let img = DynamicImage::ImageRgba8(RgbaImage::new(800, 600));
        let loaded = LoadedImage::from_dynamic_image(img, "test.png");
        let doc = ImageDocument::new(1, loaded, 1.0);

        assert_eq!(doc.id, 1);
        assert_eq!(doc.title, "test.png");
        assert_eq!(doc.loaded_image.width, 800);
        assert_eq!(doc.loaded_image.height, 600);
        assert!(!doc.can_undo());
        assert!(!doc.can_redo());
    }

    #[test]
    fn test_document_commit_crop_and_undo_redo() {
        let img = DynamicImage::ImageRgba8(RgbaImage::new(1000, 1000));
        let loaded = LoadedImage::from_dynamic_image(img, "portrait.jpg");
        let mut doc = ImageDocument::new(1, loaded, 1.0);

        // Set crop area to central half
        doc.crop_area = CropArea::new(0.25, 0.25, 0.5, 0.5);

        // Commit crop
        assert!(doc.commit_crop(1.0).is_ok());
        assert_eq!(doc.loaded_image.width, 500);
        assert_eq!(doc.loaded_image.height, 500);
        assert!(doc.can_undo());
        assert!(!doc.can_redo());

        // Undo crop
        assert!(doc.undo());
        assert_eq!(doc.loaded_image.width, 1000);
        assert_eq!(doc.loaded_image.height, 1000);
        assert!(!doc.can_undo());
        assert!(doc.can_redo());

        // Redo crop
        assert!(doc.redo());
        assert_eq!(doc.loaded_image.width, 500);
        assert_eq!(doc.loaded_image.height, 500);
        assert!(doc.can_undo());
        assert!(!doc.can_redo());
    }
}
