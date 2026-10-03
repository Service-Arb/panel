/**
 * The kit's Textarea sizes to its content (`field-sizing: content`), so `rows`
 * is ignored and a pasted model would push the dialog past the window's top
 * and bottom. Capped, it scrolls inside; with the refusals' own cap the dialog
 * fits a 720 px laptop window. The phone's drawer scrolls its body anyway.
 */
export const PASTE_FIELD_CLASS = "min-h-40 max-h-[30dvh] overflow-y-auto font-mono text-xs";
export const PROBLEMS_CLASS = "flex max-h-[20dvh] flex-col gap-0.5 overflow-y-auto";
