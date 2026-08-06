//! Génération PDF brandé La Minute pour les comptes-rendus.

use printpdf::path::{PaintMode, WindingOrder};
use printpdf::{BuiltinFont, Color, Mm, PdfDocument, Point, Polygon, Rgb};

use crate::ai::structured_summary::StructuredSummary;
use crate::models::MeetingStatus;

const PAGE_W: f32 = 210.0;
const PAGE_H: f32 = 297.0;
const MARGIN_X: f32 = 18.0;
const MARGIN_TOP: f32 = 20.0;
const MARGIN_BOTTOM: f32 = 18.0;
const CONTENT_WIDTH: f32 = PAGE_W - 2.0 * MARGIN_X;

const NAVY: (f32, f32, f32) = (0.024, 0.180, 0.333); // #062e55
const RED: (f32, f32, f32) = (0.776, 0.290, 0.251); // #c64a40
const CREAM: (f32, f32, f32) = (0.980, 0.965, 0.937); // #faf6ef
const MUTED: (f32, f32, f32) = (0.35, 0.42, 0.50);

pub struct MeetingReportPdfInput<'a> {
    pub title: &'a str,
    pub status: MeetingStatus,
    pub display_date: &'a str,
    pub duration_label: &'a str,
    pub summary: &'a StructuredSummary,
}

struct PdfWriter<'a> {
    doc: &'a printpdf::PdfDocumentReference,
    page: printpdf::PdfPageIndex,
    layer: printpdf::PdfLayerIndex,
    font: &'a printpdf::IndirectFontRef,
    font_bold: &'a printpdf::IndirectFontRef,
    y: f32,
}

impl<'a> PdfWriter<'a> {
    fn layer(&self) -> printpdf::PdfLayerReference {
        self.doc.get_page(self.page).get_layer(self.layer)
    }

    fn ensure_space(&mut self, needed: f32) {
        if self.y - needed >= MARGIN_BOTTOM {
            return;
        }
        let (page, layer) = self.doc.add_page(Mm(PAGE_W), Mm(PAGE_H), "Layer 1");
        self.page = page;
        self.layer = layer;
        self.y = PAGE_H - MARGIN_TOP;
    }

    fn write_wrapped(
        &mut self,
        text: &str,
        font_size: f32,
        line_height: f32,
        bold: bool,
        rgb: (f32, f32, f32),
    ) {
        let sanitized = sanitize_pdf_text(text);
        let max_chars = ((CONTENT_WIDTH / (font_size * 0.45)) as usize).max(20);
        let wrapped = wrap_text(&sanitized, max_chars);
        let font = if bold { self.font_bold } else { self.font };

        for line in wrapped {
            self.ensure_space(line_height + 2.0);
            let layer = self.layer();
            layer.set_fill_color(Color::Rgb(Rgb::new(rgb.0, rgb.1, rgb.2, None)));
            layer.use_text(line, font_size, Mm(MARGIN_X), Mm(self.y - line_height), font);
            self.y -= line_height;
        }
    }

    fn write_section(&mut self, title: &str, lines: &[String], as_bullets: bool) {
        self.ensure_space(16.0);
        self.write_wrapped(title, 12.0, 5.5, true, NAVY);
        self.y -= 2.0;

        for line in lines {
            let text = if as_bullets {
                format!("- {}", sanitize_pdf_text(line))
            } else {
                sanitize_pdf_text(line)
            };
            self.write_wrapped(&text, 10.0, 4.8, false, NAVY);
            self.y -= 1.5;
        }
        self.y -= 4.0;
    }
}

pub fn build_meeting_report_pdf(input: MeetingReportPdfInput<'_>) -> Result<Vec<u8>, String> {
    let (doc, page_index, layer_index) =
        PdfDocument::new("La Minute — compte-rendu", Mm(PAGE_W), Mm(PAGE_H), "Layer 1");

    let font = doc
        .add_builtin_font(BuiltinFont::Helvetica)
        .map_err(|err| err.to_string())?;
    let font_bold = doc
        .add_builtin_font(BuiltinFont::HelveticaBold)
        .map_err(|err| err.to_string())?;

    {
        let layer = doc.get_page(page_index).get_layer(layer_index);
        fill_rect(&layer, 0.0, PAGE_H - 22.0, PAGE_W, 22.0, NAVY);
        fill_rect(&layer, 0.0, PAGE_H - 24.0, PAGE_W, 2.0, RED);
        layer.set_fill_color(Color::Rgb(Rgb::new(CREAM.0, CREAM.1, CREAM.2, None)));
        layer.use_text(
            "La Minute",
            16.0,
            Mm(MARGIN_X),
            Mm(PAGE_H - 15.0),
            &font_bold,
        );
        layer.set_fill_color(Color::Rgb(Rgb::new(0.85, 0.88, 0.92, None)));
        layer.use_text(
            "Compte-rendu de reunion",
            9.0,
            Mm(PAGE_W - MARGIN_X - 48.0),
            Mm(PAGE_H - 14.5),
            &font,
        );
    }

    let mut writer = PdfWriter {
        doc: &doc,
        page: page_index,
        layer: layer_index,
        font: &font,
        font_bold: &font_bold,
        y: PAGE_H - 34.0,
    };

    writer.write_wrapped(input.title, 18.0, 8.0, true, NAVY);
    writer.y -= 4.0;

    let meta = format!(
        "Statut : {}   |   Date : {}   |   Duree : {}",
        status_label_fr(input.status),
        sanitize_pdf_text(input.display_date),
        sanitize_pdf_text(input.duration_label),
    );
    writer.write_wrapped(&meta, 9.0, 4.5, false, MUTED);

    writer.y -= 6.0;
    {
        let layer = writer.layer();
        fill_rect(&layer, MARGIN_X, writer.y, CONTENT_WIDTH, 0.4, RED);
    }
    writer.y -= 8.0;

    writer.write_section(
        "Synthese",
        std::slice::from_ref(&input.summary.synthese),
        false,
    );

    let decisions: Vec<String> = if input.summary.decisions.is_empty() {
        vec!["Aucune decision identifiee.".into()]
    } else {
        input.summary.decisions.clone()
    };
    writer.write_section(
        "Decisions",
        &decisions,
        !input.summary.decisions.is_empty(),
    );

    let action_lines: Vec<String> = if input.summary.actions.is_empty() {
        vec!["Aucune action identifiee.".into()]
    } else {
        input
            .summary
            .actions
            .iter()
            .map(|action| {
                let mut line = action.titre.clone();
                if let Some(desc) = action.description.as_deref().filter(|s| !s.is_empty()) {
                    line.push_str(" — ");
                    line.push_str(desc);
                }
                let mut meta_bits = Vec::new();
                if let Some(r) = action.responsable.as_deref().filter(|s| !s.is_empty()) {
                    meta_bits.push(format!("responsable : {r}"));
                }
                if let Some(e) = action.echeance.as_deref().filter(|s| !s.is_empty()) {
                    meta_bits.push(format!("echeance : {e}"));
                }
                if !meta_bits.is_empty() {
                    line.push_str(" (");
                    line.push_str(&meta_bits.join(", "));
                    line.push(')');
                }
                line
            })
            .collect()
    };
    writer.write_section(
        "Actions",
        &action_lines,
        !input.summary.actions.is_empty(),
    );

    if !input.summary.risques.is_empty() {
        writer.write_section("Risques", &input.summary.risques, true);
    }

    if !input.summary.questions_ouvertes.is_empty() {
        writer.write_section(
            "Questions ouvertes",
            &input.summary.questions_ouvertes,
            true,
        );
    }

    {
        let layer = writer.layer();
        layer.set_fill_color(Color::Rgb(Rgb::new(MUTED.0, MUTED.1, MUTED.2, None)));
        layer.use_text("Genere par La Minute", 8.0, Mm(MARGIN_X), Mm(10.0), &font);
    }

    let mut bytes = Vec::new();
    doc.save(&mut std::io::BufWriter::new(&mut bytes))
        .map_err(|err| err.to_string())?;
    Ok(bytes)
}

fn status_label_fr(status: MeetingStatus) -> &'static str {
    match status {
        MeetingStatus::Draft => "Brouillon",
        MeetingStatus::Recording => "Enregistrement",
        MeetingStatus::Processing => "Traitement",
        MeetingStatus::Completed => "Terminee",
    }
}

/// Remplace les caractères hors WinAnsi courant pour les polices PDF intégrées.
fn sanitize_pdf_text(input: &str) -> String {
    input
        .chars()
        .map(|c| match c {
            'à' | 'á' | 'â' | 'ä' | 'ã' => 'a',
            'À' | 'Á' | 'Â' | 'Ä' | 'Ã' => 'A',
            'è' | 'é' | 'ê' | 'ë' => 'e',
            'È' | 'É' | 'Ê' | 'Ë' => 'E',
            'ì' | 'í' | 'î' | 'ï' => 'i',
            'Ì' | 'Í' | 'Î' | 'Ï' => 'I',
            'ò' | 'ó' | 'ô' | 'ö' | 'õ' => 'o',
            'Ò' | 'Ó' | 'Ô' | 'Ö' | 'Õ' => 'O',
            'ù' | 'ú' | 'û' | 'ü' => 'u',
            'Ù' | 'Ú' | 'Û' | 'Ü' => 'U',
            'ç' => 'c',
            'Ç' => 'C',
            'ñ' => 'n',
            'Ñ' => 'N',
            'œ' => 'o',
            'Œ' => 'O',
            'æ' => 'a',
            'Æ' => 'A',
            '’' | '‘' | '`' => '\'',
            '“' | '”' => '"',
            '–' | '—' => '-',
            '…' => '.',
            '€' => 'E',
            c if c.is_ascii() => c,
            _ => '?',
        })
        .collect()
}

fn fill_rect(
    layer: &printpdf::PdfLayerReference,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    rgb: (f32, f32, f32),
) {
    layer.set_fill_color(Color::Rgb(Rgb::new(rgb.0, rgb.1, rgb.2, None)));
    let points = vec![
        (Point::new(Mm(x), Mm(y)), false),
        (Point::new(Mm(x + w), Mm(y)), false),
        (Point::new(Mm(x + w), Mm(y + h)), false),
        (Point::new(Mm(x), Mm(y + h)), false),
    ];
    let poly = Polygon {
        rings: vec![points],
        mode: PaintMode::Fill,
        winding_order: WindingOrder::NonZero,
    };
    layer.add_polygon(poly);
}

fn wrap_text(text: &str, max_chars: usize) -> Vec<String> {
    if text.is_empty() {
        return vec![String::new()];
    }
    let mut lines = Vec::new();
    let mut current = String::new();
    for word in text.split_whitespace() {
        if current.is_empty() {
            current.push_str(word);
            continue;
        }
        if current.len() + 1 + word.len() <= max_chars {
            current.push(' ');
            current.push_str(word);
        } else {
            lines.push(current);
            current = word.to_string();
        }
    }
    if !current.is_empty() {
        lines.push(current);
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::structured_summary::{StructuredActionItem, StructuredSummary};

    #[test]
    fn builds_non_empty_pdf() {
        let summary = StructuredSummary {
            synthese: "Point d'avancement du trimestre.".into(),
            decisions: vec!["Valider le roadmap".into()],
            actions: vec![StructuredActionItem {
                titre: "Rediger le brief".into(),
                description: Some("Version courte".into()),
                responsable: Some("Alice".into()),
                echeance: Some("2026-08-12".into()),
            }],
            risques: vec!["Delai serre".into()],
            questions_ouvertes: vec!["Budget ?".into()],
        };

        let bytes = build_meeting_report_pdf(MeetingReportPdfInput {
            title: "Comite produit",
            status: MeetingStatus::Completed,
            display_date: "5 aout 2026",
            duration_label: "12:30",
            summary: &summary,
        })
        .expect("pdf");

        assert!(bytes.starts_with(b"%PDF"));
        assert!(bytes.len() > 500);
    }

    #[test]
    fn sanitize_strips_accents() {
        assert_eq!(sanitize_pdf_text("été café"), "ete cafe");
    }
}
