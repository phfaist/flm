// The attributes the page script sets on the document's elements. They are the page
// script's own: the update of a region does not count them as changes of the document.

/** The attribute marking an element a problem lies in; its value is the problem's severity. */
export const MARK = 'data-flm-preview-problem';

/** The attribute an element carries while it flashes. */
export const FLASH = 'data-flm-preview-flash';

/** The attribute of the element that Inspect selected. */
export const INSPECTED = 'data-flm-preview-inspected';

/** The attributes above. */
export const OWN_ATTRIBUTES: readonly string[] = [MARK, FLASH, INSPECTED];
