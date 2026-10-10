import { DropdownMenuItem } from "@evinvest/uikit";

import type { AskPlan } from "../model/ask";

/**
 * The menu's entries, one per way to ask. The kit's item runs `onClick` for a
 * press and for Enter or Space (it clicks itself) and then closes the menu; it
 * has no `onSelect`, which would land on the div as a text-selection event.
 */
export function planItems(plans: readonly AskPlan[], label: (plan: AskPlan) => string, pick: (plan: AskPlan) => void) {
  return plans.map((plan) => (
    <DropdownMenuItem key={plan.channel} onClick={() => pick(plan)}>
      {label(plan)}
    </DropdownMenuItem>
  ));
}
