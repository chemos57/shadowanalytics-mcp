use crate::manifest::PdfManifestEntry;
use anyhow::Result;
use pdf_extract::{ColorSpace, MediaBox, OutputDev, OutputError, Path as PdfPath, Transform};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::fmt::Write;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExtractedPage {
    pub doc_id: String,
    pub file_name: String,
    pub page: u32,
    pub text: String,
}

pub fn extract_pdf_pages(entry: &PdfManifestEntry) -> Result<Vec<ExtractedPage>> {
    let page_texts = extract_text_by_pages_single_pass(Path::new(&entry.path))?;
    Ok(pages_from_texts(
        &entry.doc_id,
        &entry.file_name,
        page_texts,
    ))
}

pub fn extract_manifest_pages(entries: &[PdfManifestEntry]) -> Result<Vec<ExtractedPage>> {
    collect_pages_in_manifest_order(entries, extract_pdf_pages)
}

pub fn collect_pages_in_manifest_order<F>(
    entries: &[PdfManifestEntry],
    extract_pages: F,
) -> Result<Vec<ExtractedPage>>
where
    F: Fn(&PdfManifestEntry) -> Result<Vec<ExtractedPage>> + Sync,
{
    let mut extracted = entries
        .par_iter()
        .enumerate()
        .map(|(index, entry)| extract_pages(entry).map(|pages| (index, pages)))
        .collect::<Vec<_>>()
        .into_iter()
        .collect::<Result<Vec<_>>>()?;

    extracted.sort_by_key(|(index, _)| *index);

    Ok(extracted.into_iter().flat_map(|(_, pages)| pages).collect())
}

pub fn pages_from_texts(
    doc_id: &str,
    file_name: &str,
    page_texts: Vec<String>,
) -> Vec<ExtractedPage> {
    page_texts
        .into_iter()
        .enumerate()
        .filter_map(|(index, page_text)| {
            let text = normalize_text(&page_text);
            (!text.is_empty()).then(|| ExtractedPage {
                doc_id: doc_id.to_string(),
                file_name: file_name.to_string(),
                page: index as u32 + 1,
                text,
            })
        })
        .collect()
}

pub fn split_text_into_pages(doc_id: &str, file_name: &str, text: &str) -> Vec<ExtractedPage> {
    let normalized = normalize_text(text);
    normalized
        .split('\u{000c}')
        .enumerate()
        .filter_map(|(index, page_text)| {
            let text = normalize_text(page_text);
            (!text.is_empty()).then(|| ExtractedPage {
                doc_id: doc_id.to_string(),
                file_name: file_name.to_string(),
                page: index as u32 + 1,
                text,
            })
        })
        .collect()
}

pub fn normalize_text(text: &str) -> String {
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

fn extract_text_by_pages_single_pass(path: &Path) -> Result<Vec<String>, OutputError> {
    let mut doc = pdf_extract::Document::load(path)?;
    if doc.is_encrypted() {
        doc.decrypt("")?;
    }

    let mut output = SinglePassPageTextOutput::new();
    pdf_extract::output_doc(&doc, &mut output)?;
    Ok(output.into_pages())
}

pub struct SinglePassPageTextOutput {
    pages: Vec<String>,
    current_page: String,
    last_end: f64,
    last_y: f64,
    first_char: bool,
    flip_ctm: Transform,
}

impl SinglePassPageTextOutput {
    pub fn new() -> Self {
        Self {
            pages: Vec::new(),
            current_page: String::new(),
            last_end: 100000.0,
            last_y: 0.0,
            first_char: false,
            flip_ctm: Transform::identity(),
        }
    }

    pub fn into_pages(self) -> Vec<String> {
        self.pages
    }

    fn reset_page_state(&mut self) {
        self.current_page.clear();
        self.last_end = 100000.0;
        self.last_y = 0.0;
        self.first_char = false;
    }
}

impl Default for SinglePassPageTextOutput {
    fn default() -> Self {
        Self::new()
    }
}

impl OutputDev for SinglePassPageTextOutput {
    fn begin_page(
        &mut self,
        _page_num: u32,
        media_box: &MediaBox,
        _art_box: Option<(f64, f64, f64, f64)>,
    ) -> Result<(), OutputError> {
        self.reset_page_state();
        self.flip_ctm =
            Transform::row_major(1.0, 0.0, 0.0, -1.0, 0.0, media_box.ury - media_box.lly);
        Ok(())
    }

    fn end_page(&mut self) -> Result<(), OutputError> {
        self.pages.push(std::mem::take(&mut self.current_page));
        Ok(())
    }

    fn output_character(
        &mut self,
        trm: &Transform,
        width: f64,
        _spacing: f64,
        font_size: f64,
        char: &str,
    ) -> Result<(), OutputError> {
        let position = trm.post_transform(&self.flip_ctm);
        let transformed_font_size_vec = trm.transform_vector(euclid::vec2(font_size, font_size));
        let transformed_font_size =
            (transformed_font_size_vec.x * transformed_font_size_vec.y).sqrt();
        let (x, y) = (position.m31, position.m32);

        if self.first_char {
            if (y - self.last_y).abs() > transformed_font_size * 1.5 {
                writeln!(self.current_page)?;
            }

            if x < self.last_end && (y - self.last_y).abs() > transformed_font_size * 0.5 {
                writeln!(self.current_page)?;
            }

            if x > self.last_end + transformed_font_size * 0.1 {
                write!(self.current_page, " ")?;
            }
        }

        write!(self.current_page, "{char}")?;
        self.first_char = false;
        self.last_y = y;
        self.last_end = x + width * transformed_font_size;
        Ok(())
    }

    fn begin_word(&mut self) -> Result<(), OutputError> {
        self.first_char = true;
        Ok(())
    }

    fn end_word(&mut self) -> Result<(), OutputError> {
        Ok(())
    }

    fn end_line(&mut self) -> Result<(), OutputError> {
        Ok(())
    }

    fn stroke(
        &mut self,
        _ctm: &Transform,
        _colorspace: &ColorSpace,
        _color: &[f64],
        _path: &PdfPath,
    ) -> Result<(), OutputError> {
        Ok(())
    }

    fn fill(
        &mut self,
        _ctm: &Transform,
        _colorspace: &ColorSpace,
        _color: &[f64],
        _path: &PdfPath,
    ) -> Result<(), OutputError> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_pass_output_collects_one_buffer_per_page() {
        let media_box = MediaBox {
            llx: 0.0,
            lly: 0.0,
            urx: 612.0,
            ury: 792.0,
        };
        let mut output = SinglePassPageTextOutput::new();

        output.begin_page(1, &media_box, None).unwrap();
        output.current_page.push_str("First page");
        output.end_page().unwrap();
        output.begin_page(2, &media_box, None).unwrap();
        output.current_page.push_str("Second page");
        output.end_page().unwrap();

        assert_eq!(
            output.into_pages(),
            vec!["First page".to_string(), "Second page".to_string()]
        );
    }
}
