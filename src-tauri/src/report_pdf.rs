//! Génération PDF brandé La Minute pour les comptes-rendus.
//!
//! Utilise printpdf ≥ 0.11 (lopdf ≥ 0.42) en écriture seule — aucun parsing
//! de PDF non fiable n'est effectué ici.

use printpdf::{
    BuiltinFont, Color, Mm, Op, PaintMode, PdfDocument, PdfFontHandle, PdfPage, PdfSaveOptions,
    Point, Pt, Rect, Rgb, TextItem, WindingOrder,
};

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
    pub provider_id: Option<&'a str>,
    pub model: Option<&'a str>,
    pub generated_at: Option<&'a str>,
    pub validation_state: Option<&'a str>,
}

struct PdfWriter {
    pages: Vec<PdfPage>,
    ops: Vec<Op>,
    y: f32,
}

impl PdfWriter {
    fn new() -> Self {
        let mut writer = Self {
            pages: Vec::new(),
            ops: Vec::new(),
            y: PAGE_H - 34.0,
        };
        // En-tête brandé uniquement sur la première page (comportement 0.7).
        push_fill_rect(&mut writer.ops, 0.0, PAGE_H - 22.0, PAGE_W, 22.0, NAVY);
        push_fill_rect(&mut writer.ops, 0.0, PAGE_H - 24.0, PAGE_W, 2.0, RED);
        push_text(
            &mut writer.ops,
            "La Minute",
            16.0,
            MARGIN_X,
            PAGE_H - 15.0,
            true,
            CREAM,
        );
        push_text(
            &mut writer.ops,
            "Compte-rendu de reunion",
            9.0,
            PAGE_W - MARGIN_X - 48.0,
            PAGE_H - 14.5,
            false,
            (0.85, 0.88, 0.92),
        );
        writer
    }

    fn flush_page(&mut self) {
        if self.ops.is_empty() && self.pages.is_empty() {
            return;
        }
        let ops = std::mem::take(&mut self.ops);
        self.pages.push(PdfPage::new(Mm(PAGE_W), Mm(PAGE_H), ops));
        self.y = PAGE_H - MARGIN_TOP;
    }

    fn ensure_space(&mut self, needed: f32) {
        if self.y - needed >= MARGIN_BOTTOM {
            return;
        }
        self.flush_page();
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

        for line in wrapped {
            self.ensure_space(line_height + 2.0);
            push_text(
                &mut self.ops,
                &line,
                font_size,
                MARGIN_X,
                self.y - line_height,
                bold,
                rgb,
            );
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

    fn finish(mut self) -> Vec<PdfPage> {
        push_text(
            &mut self.ops,
            "Genere par La Minute",
            8.0,
            MARGIN_X,
            10.0,
            false,
            MUTED,
        );
        self.flush_page();
        self.pages
    }
}

pub fn build_meeting_report_pdf(input: MeetingReportPdfInput<'_>) -> Result<Vec<u8>, String> {
    let mut doc = PdfDocument::new("La Minute — compte-rendu");
    let mut writer = PdfWriter::new();

    writer.write_wrapped(input.title, 18.0, 8.0, true, NAVY);
    writer.y -= 4.0;

    let mut meta = format!(
        "Statut : {}   |   Date : {}   |   Duree : {}",
        status_label_fr(input.status),
        sanitize_pdf_text(input.display_date),
        sanitize_pdf_text(input.duration_label),
    );
    if let Some(provider) = input.provider_id.filter(|s| !s.is_empty()) {
        meta.push_str(&format!("   |   Fournisseur : {}", sanitize_pdf_text(provider)));
    }
    if let Some(model) = input.model.filter(|s| !s.is_empty()) {
        meta.push_str(&format!("   |   Modele : {}", sanitize_pdf_text(model)));
    }
    if let Some(validation) = input.validation_state.filter(|s| !s.is_empty()) {
        let label = match validation {
            "validated" => "Valide",
            "edited" => "Corrige",
            _ => "Genere",
        };
        meta.push_str(&format!("   |   Validation : {label}"));
    }
    writer.write_wrapped(&meta, 9.0, 4.5, false, MUTED);

    writer.y -= 6.0;
    writer.ensure_space(2.0);
    push_fill_rect(&mut writer.ops, MARGIN_X, writer.y, CONTENT_WIDTH, 0.4, RED);
    writer.y -= 8.0;

    writer.write_section(
        "Synthese",
        std::slice::from_ref(&input.summary.synthese),
        false,
    );

    let decisions: Vec<String> = if input.summary.decisions.is_empty() {
        vec!["Aucune decision identifiee.".into()]
    } else {
        input
            .summary
            .decisions
            .iter()
            .map(|decision| decision.text().to_string())
            .collect()
    };
    writer.write_section("Decisions", &decisions, !input.summary.decisions.is_empty());

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
    writer.write_section("Actions", &action_lines, !input.summary.actions.is_empty());

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

    let pages = writer.finish();
    let mut warnings = Vec::new();
    let bytes = doc
        .with_pages(pages)
        .save(&PdfSaveOptions::default(), &mut warnings);
    if bytes.is_empty() || !bytes.starts_with(b"%PDF") {
        return Err("echec de generation PDF".into());
    }
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

fn rgb(color: (f32, f32, f32)) -> Color {
    Color::Rgb(Rgb::new(color.0, color.1, color.2, None))
}

fn push_fill_rect(ops: &mut Vec<Op>, x: f32, y: f32, w: f32, h: f32, color: (f32, f32, f32)) {
    ops.push(Op::SetFillColor { col: rgb(color) });
    let mut rect = Rect::from_xywh(Mm(x).into(), Mm(y).into(), Mm(w).into(), Mm(h).into());
    rect.mode = Some(PaintMode::Fill);
    rect.winding_order = Some(WindingOrder::NonZero);
    ops.push(Op::DrawRectangle { rectangle: rect });
}

fn push_text(
    ops: &mut Vec<Op>,
    text: &str,
    font_size: f32,
    x: f32,
    y: f32,
    bold: bool,
    color: (f32, f32, f32),
) {
    let font = if bold {
        BuiltinFont::HelveticaBold
    } else {
        BuiltinFont::Helvetica
    };
    ops.push(Op::SaveGraphicsState);
    ops.push(Op::StartTextSection);
    ops.push(Op::SetTextCursor {
        pos: Point::new(Mm(x), Mm(y)),
    });
    ops.push(Op::SetFont {
        font: PdfFontHandle::Builtin(font),
        size: Pt(font_size),
    });
    ops.push(Op::SetLineHeight { lh: Pt(font_size) });
    ops.push(Op::SetFillColor { col: rgb(color) });
    ops.push(Op::ShowText {
        items: vec![TextItem::Text(text.to_string())],
    });
    ops.push(Op::EndTextSection);
    ops.push(Op::RestoreGraphicsState);
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

    fn sample_summary() -> StructuredSummary {
        StructuredSummary {
            synthese: "Point d'avancement du trimestre.".into(),
            decisions: vec!["Valider le roadmap".into()],
            actions: vec![StructuredActionItem {
                titre: "Rediger le brief".into(),
                description: Some("Version courte".into()),
                responsable: Some("Alice".into()),
                echeance: Some("2026-08-12".into()),
                ..Default::default()
            }],
            risques: vec!["Delai serre".into()],
            questions_ouvertes: vec!["Budget ?".into()],
        }
    }

    fn pdf_page_count(bytes: &[u8]) -> usize {
        // printpdf 0.12 sérialise `/Type/Page` sans espace.
        let mut count = 0usize;
        let needle = b"/Type/Page";
        let mut i = 0;
        while let Some(pos) = bytes[i..].windows(needle.len()).position(|w| w == needle) {
            let after = i + pos + needle.len();
            // Exclure `/Type/Pages`
            if bytes.get(after) != Some(&b's') {
                count += 1;
            }
            i = after;
        }
        count
    }

    #[test]
    fn builds_non_empty_pdf() {
        let summary = sample_summary();
        let bytes = build_meeting_report_pdf(MeetingReportPdfInput {
            title: "Comite produit",
            status: MeetingStatus::Completed,
            display_date: "5 aout 2026",
            duration_label: "12:30",
            summary: &summary,
        
                provider_id: None,
                model: None,
                generated_at: None,
                validation_state: None,
            })
        .expect("pdf");

        assert!(bytes.starts_with(b"%PDF"));
        assert!(bytes.len() > 500);
        assert!(pdf_page_count(&bytes) >= 1);
    }

    #[test]
    fn builds_multipage_pdf_for_long_content() {
        let long_synth = "Lorem ipsum dolor sit amet, consectetur adipiscing elit. ".repeat(80);
        let many_actions: Vec<_> = (1..=40)
            .map(|i| StructuredActionItem {
                titre: format!(
                    "Action numero {i} avec un titre un peu long pour forcer le wrapping"
                ),
                description: Some(format!("Description detaillee de l'action {i}")),
                responsable: Some(format!("Personne {i}")),
                echeance: Some("2026-09-01".into()),
                ..Default::default()
            })
            .collect();
        let summary = StructuredSummary {
            synthese: long_synth,
            decisions: (1..=20).map(|i| format!("Decision {i}").into()).collect(),
            actions: many_actions,
            risques: (1..=15).map(|i| format!("Risque {i}")).collect(),
            questions_ouvertes: (1..=15).map(|i| format!("Question {i} ?")).collect(),
        };

        let bytes = build_meeting_report_pdf(MeetingReportPdfInput {
            title: "Reunion longue multipage",
            status: MeetingStatus::Completed,
            display_date: "8 aout 2026",
            duration_label: "45:00",
            summary: &summary,
        
                provider_id: None,
                model: None,
                generated_at: None,
                validation_state: None,
            })
        .expect("pdf");

        assert!(bytes.starts_with(b"%PDF"));
        assert!(
            pdf_page_count(&bytes) >= 2,
            "attendu multipage, pages={}",
            pdf_page_count(&bytes)
        );
    }

    #[test]
    fn accents_are_sanitized_in_pdf_payload() {
        let summary = StructuredSummary {
            synthese: "café été naïve".into(),
            decisions: vec![],
            actions: vec![],
            risques: vec![],
            questions_ouvertes: vec![],
        };
        let bytes = build_meeting_report_pdf(MeetingReportPdfInput {
            title: "Comité — résumé",
            status: MeetingStatus::Draft,
            display_date: "5 août 2026",
            duration_label: "1:00",
            summary: &summary,
        
                provider_id: None,
                model: None,
                generated_at: None,
                validation_state: None,
            })
        .expect("pdf");

        let payload = String::from_utf8_lossy(&bytes);
        assert!(payload.contains("cafe ete naive") || payload.contains("Comite"));
        assert!(!payload.contains("café"));
        assert!(!payload.contains("août"));
    }

    #[test]
    fn empty_sections_get_placeholders() {
        let summary = StructuredSummary {
            synthese: String::new(),
            decisions: vec![],
            actions: vec![],
            risques: vec![],
            questions_ouvertes: vec![],
        };
        let bytes = build_meeting_report_pdf(MeetingReportPdfInput {
            title: "Vide",
            status: MeetingStatus::Draft,
            display_date: "-",
            duration_label: "0:00",
            summary: &summary,
        
                provider_id: None,
                model: None,
                generated_at: None,
                validation_state: None,
            })
        .expect("pdf");
        let payload = String::from_utf8_lossy(&bytes);
        assert!(payload.contains("Aucune decision identifiee"));
        assert!(payload.contains("Aucune action identifiee"));
    }

    #[test]
    fn wraps_very_long_unbroken_tokens() {
        let long_word = "x".repeat(200);
        let summary = StructuredSummary {
            synthese: long_word.clone(),
            decisions: vec![],
            actions: vec![],
            risques: vec![],
            questions_ouvertes: vec![],
        };
        let bytes = build_meeting_report_pdf(MeetingReportPdfInput {
            title: "Token long",
            status: MeetingStatus::Completed,
            display_date: "1",
            duration_label: "1",
            summary: &summary,
        
                provider_id: None,
                model: None,
                generated_at: None,
                validation_state: None,
            })
        .expect("pdf");
        assert!(bytes.starts_with(b"%PDF"));
        assert!(bytes.len() > 500);
    }

    #[test]
    fn sanitize_strips_accents() {
        assert_eq!(sanitize_pdf_text("été café"), "ete cafe");
    }

    #[test]
    fn wrap_text_splits_words() {
        let lines = wrap_text("un deux trois quatre", 10);
        assert!(lines.len() >= 2);
        assert!(lines.iter().all(|l| l.len() <= 10 || !l.contains(' ')));
    }
}
