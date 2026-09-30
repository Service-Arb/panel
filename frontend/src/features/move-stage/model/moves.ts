import type { Stage } from "@/entities/lead";

export type MoveKind = "contacted" | "quoted" | "won" | "lost" | "completed";

/**
 * The next steps offered from a stage. Stages only move forward; `lost` is
 * reachable from anywhere before payment, and a lost lead can be reopened by
 * progress (`panel_core::lead::fold`). A paid lead has nowhere left to go.
 */
export const MOVES: Record<Stage, readonly MoveKind[]> = {
  created: ["contacted", "quoted", "lost"],
  contacted: ["quoted", "won", "lost"],
  quoted: ["won", "lost"],
  won: ["completed", "lost"],
  completed: [],
  paid: [],
  lost: ["contacted", "quoted", "won"],
};

/**
 * Why a lead was lost, as slugs the reports group by. The backend takes any
 * slug; this list is the panel's vocabulary, kept short so the reasons stay
 * comparable.
 */
export const LOST_REASONS = ["too_expensive", "no_answer", "went_elsewhere", "out_of_area", "not_needed", "spam", "other"] as const;
export type LostReason = (typeof LOST_REASONS)[number];
