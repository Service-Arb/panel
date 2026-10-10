import { Button, type ButtonSize } from "@evinvest/uikit";
import { MessageSquareHeart } from "lucide-react";
import type { ComponentProps } from "react";

/**
 * "Ask for a review". Off is `aria-disabled`, not `disabled`: a native disabled
 * button leaves the tab order and its `aria-describedby` reason is never read.
 */
export function ReviewTrigger({ label, off, reasonId, size, onPress, ...rest }: { label: string; off: boolean; reasonId: string | undefined; size: ButtonSize; onPress: (() => void) | undefined } & Omit<ComponentProps<typeof Button>, "size" | "onClick">) {
  return (
    <Button
      {...rest}
      type="button"
      variant="outline"
      size={size}
      className="self-start aria-disabled:cursor-not-allowed aria-disabled:opacity-50"
      aria-disabled={off}
      aria-describedby={off ? reasonId : undefined}
      onClick={off ? undefined : onPress}
    >
      <MessageSquareHeart aria-hidden />
      {label}
    </Button>
  );
}
