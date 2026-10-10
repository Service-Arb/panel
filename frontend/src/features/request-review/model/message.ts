import type { LeadLocale } from "@/entities/lead";

/** The customer's language: the landing's, French when it said none. */
export function messageLocale(locale: LeadLocale | null): LeadLocale {
  return locale ?? "fr";
}

/** "Jean Dupont" → "Jean": the greeting uses the first name only; none, no name. */
export function firstNameOf(name: string | null): string | null {
  return name?.trim().split(/\s+/)[0] || null;
}

/** The name the customer knows: the place's `brandName`, else the brand's slug capitalised. */
export function brandLabel(brandName: string | null, slug: string): string {
  const name = brandName?.trim();
  return name || slug.charAt(0).toUpperCase() + slug.slice(1);
}

export interface MessageParts {
  locale: LeadLocale | null;
  name: string | null;
  brand: string;
  link: string;
}

/**
 * The same text for every finished job: no filter by rating or mood and no
 * reward, as Google's review policy asks. Written here, not by the server, so
 * the operator's copy is the customer's.
 */
export function reviewMessage({ locale, name, brand, link }: MessageParts): string {
  const first = firstNameOf(name);
  if (messageLocale(locale) === "en") {
    return `Hello${first ? ` ${first}` : ""}, thank you for choosing ${brand}. If you have a minute, your review helps us a lot: ${link}`;
  }
  return `Bonjour${first ? ` ${first}` : ""}, merci d'avoir fait appel à ${brand}. Si vous avez une minute, votre avis nous aide beaucoup : ${link}`;
}
