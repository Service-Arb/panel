import { http, ignoreBody } from "@/shared/api";
import { object, str } from "@/shared/lib/parse";

import { type LeadCard, type LeadPage, type LeadRef, type Stage, leadCardParser, leadPageParser, leadPath } from "../model/lead";

export interface LeadFilter {
  stage: Stage | null;
  brand: string | null;
  location: string | null;
  overdue: boolean;
}

export function fetchLeads(filter: LeadFilter, cursor: string | null, limit = 50): Promise<LeadPage> {
  return http.get("/api/v1/leads", leadPageParser, {
    stage: filter.stage,
    brand: filter.brand,
    location: filter.location,
    overdue: filter.overdue ? true : null,
    cursor,
    limit,
  });
}

export function fetchLeadCard(ref: LeadRef): Promise<LeadCard> {
  return http.get(leadPath(ref), leadCardParser);
}

export interface NewLead {
  brand: string;
  location: string;
  need: string;
  phone: string | null;
}

const createdParser = object({ brand: str, lead_id: str, event_id: str });

export function createLead(lead: NewLead): Promise<{ brand: string; lead_id: string }> {
  const body = { brand: lead.brand, location: lead.location, need: lead.need, ...(lead.phone ? { phone: lead.phone } : {}) };
  return http.send("POST", "/api/v1/leads", body, createdParser);
}

/** The stage bodies `POST …/stage` takes (`panel_server::api::StageBody`). */
export type StageMove =
  | { stage: "contacted"; channel?: string }
  | { stage: "quoted"; amount?: number; currency?: string }
  | { stage: "won"; job_id?: string }
  | { stage: "lost"; reason: string; note?: string }
  | { stage: "completed" };

export async function moveLead(ref: LeadRef, move: StageMove): Promise<void> {
  await http.send("POST", `${leadPath(ref)}/stage`, move, ignoreBody);
}

export interface PaymentInput {
  billed: number;
  commission: number;
  currency: string;
}

export async function recordPayment(ref: LeadRef, payment: PaymentInput): Promise<void> {
  await http.send("POST", `${leadPath(ref)}/payments`, payment, ignoreBody);
}
