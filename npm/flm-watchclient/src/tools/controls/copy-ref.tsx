// The buttons that copy the code that refers to a label (`referenceCode`: `\ref{…}`, or
// `\term{…}` for a term) to the clipboard, then say so for a moment: one that reads what it
// copies, and one with a copy icon; and the copy they are made with (`useCopy`), which other
// buttons share. A failed copy shows its line in the status bar.

import { useEffect, useRef, useState } from 'preact/hooks';
import { referenceCode } from '../state/description';
import { dispatch } from '../state/store';
import { Icon } from './icon';

/** How long the button says that it copied, in milliseconds. */
const COPIED_MS = 1500;

/**
 * Copies `text` to the clipboard: gives whether the text was copied a moment ago, and the
 * function that copies it. A failed copy shows its line in the status bar.
 */
export function useCopy(text: string): [boolean, () => void] {
  const [copied, setCopied] = useState(false);
  const timer = useRef<number | null>(null);
  useEffect(() => () => {
    if (timer.current !== null) window.clearTimeout(timer.current);
  }, []);
  const copy = async () => {
    try {
      await navigator.clipboard.writeText(text);
    } catch {
      dispatch({ type: 'failed', text: `Failed to copy ‘${text}’ to the clipboard` });
      return;
    }
    if (timer.current !== null) window.clearTimeout(timer.current);
    setCopied(true);
    timer.current = window.setTimeout(() => setCopied(false), COPIED_MS);
  };
  return [copied, () => void copy()];
}

/** Copies the code that refers to `label`; the button reads "Copy" and the code. `className` gives its look. */
export function CopyRef({ label, className }: { label: string; className: string }) {
  const code = referenceCode(label);
  const [copied, copy] = useCopy(code);
  return (
    <button type="button" class={className} aria-label={`Copy ‘${code}’`} onClick={copy}>
      {copied ? 'Copied' : `Copy ${code}`}
    </button>
  );
}

/** Copies `code`, the code that refers to a label; the button shows a copy icon, then a check once copied. */
export function CopyCode({ code }: { code: string }) {
  const [copied, copy] = useCopy(code);
  return (
    <button type="button" class="flm-icon-button flm-copy-code" aria-label={`Copy ‘${code}’`} title={`Copy ${code}`} onClick={copy}>
      <Icon name={copied ? 'check' : 'copy'} size={14} />
    </button>
  );
}
