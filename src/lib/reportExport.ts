import type { StructuredSummary } from "./ai/types";
import {
  formatDurationMs,
  meetingDisplayDate,
  meetingDurationMs,
  meetingStatusLabel,
  type MeetingDetail,
} from "./meetings";

export type ReportExportFormat = "markdown" | "pdf" | "json";

export interface ReportExportMeta {
  title: string;
  statusLabel: string;
  displayDate: string;
  durationLabel: string;
}

export function reportExportMeta(detail: MeetingDetail): ReportExportMeta {
  return {
    title: detail.title,
    statusLabel: meetingStatusLabel(detail.status),
    displayDate: meetingDisplayDate(detail),
    durationLabel: formatDurationMs(meetingDurationMs(detail)),
  };
}

function bulletList(items: string[]): string {
  if (items.length === 0) {
    return "_Aucun élément._";
  }
  return items.map((item) => `- ${item}`).join("\n");
}

function formatActions(actions: StructuredSummary["actions"]): string {
  if (actions.length === 0) {
    return "_Aucune action identifiée._";
  }
  return actions
    .map((action) => {
      const bits = [`**${action.titre}**`];
      if (action.description) {
        bits.push(action.description);
      }
      const meta: string[] = [];
      if (action.responsable) {
        meta.push(`responsable : ${action.responsable}`);
      }
      if (action.echeance) {
        meta.push(`échéance : ${action.echeance}`);
      }
      const line = bits.join(" — ");
      return meta.length > 0 ? `- ${line} (${meta.join(", ")})` : `- ${line}`;
    })
    .join("\n");
}

/** Construit un Markdown exploitable à partir du compte-rendu affiché. */
export function buildReportMarkdown(
  meta: ReportExportMeta,
  summary: StructuredSummary,
): string {
  const sections: string[] = [
    `# ${meta.title}`,
    "",
    `*Compte-rendu exporté depuis La Minute*`,
    "",
    `| | |`,
    `| --- | --- |`,
    `| Statut | ${meta.statusLabel} |`,
    `| Date | ${meta.displayDate} |`,
    `| Durée | ${meta.durationLabel} |`,
    "",
    `## Synthèse`,
    "",
    summary.synthese.trim() || "_Synthèse vide._",
    "",
    `## Décisions`,
    "",
    bulletList(summary.decisions),
    "",
    `## Actions`,
    "",
    formatActions(summary.actions),
  ];

  if (summary.risques.length > 0) {
    sections.push("", `## Risques`, "", bulletList(summary.risques));
  }

  if (summary.questionsOuvertes.length > 0) {
    sections.push("", `## Questions ouvertes`, "", bulletList(summary.questionsOuvertes));
  }

  sections.push("");
  return sections.join("\n");
}
