// The YAML editor: a text area with Cancel and Save, the key's path as its label. It starts
// from the text that the server gives for the path (the value the front matter writes,
// else the value in effect, else the default of the schema), else from the default that the
// row gives, or from nothing for a key that is added.
// Save writes the text as the value at the path; the server reads it and checks the value.
// A refused Save keeps the editor open with the author's text.

import { useEffect, useRef, useState } from 'preact/hooks';
import { valueAsYaml } from '../requests';

/** The button *Edit as YAML*, with the beginning of the written value beside it. */
export function EditAsYaml({ preview, open, disabled, onOpen }: {
  preview: string;
  open: boolean;
  disabled: boolean;
  onOpen: () => void;
}) {
  return (
    <span class="flm-edit-yaml">
      {preview !== '' && <span class="flm-yaml-preview">{preview}</span>}
      <button type="button" class="flm-add" aria-expanded={open} disabled={disabled || open} onClick={onOpen}>
        Edit as YAML
      </button>
    </span>
  );
}

export function YamlEditor({ path, fresh = false, fallback = '', disabled, onSave, onClose }: {
  path: readonly string[];
  /**
   * Whether the editor starts from nothing: the key is added, or the value the row shows is
   * that of the last successful build, which a save would copy into the front matter.
   */
  fresh?: boolean;
  /** The text that the editor starts from where the server gives none: the default of a key whose schema gives none. */
  fallback?: string;
  disabled: boolean;
  /** Writes the text; gives whether the server made the change. */
  onSave: (yaml: string) => Promise<boolean>;
  onClose: () => void;
}) {
  const [text, setText] = useState<string | null>(fresh ? '' : null);
  const [saving, setSaving] = useState(false);
  const area = useRef<HTMLTextAreaElement>(null);
  const label = path.join('.');
  const loaded = text !== null;
  // The text area takes the focus once it holds its text, as the button that opened it is
  // then disabled.
  useEffect(() => {
    if (loaded) area.current?.focus();
  }, [loaded]);
  useEffect(() => {
    if (fresh) return undefined;
    let current = true;
    void valueAsYaml(path).then((yaml) => {
      if (current) setText(yaml === null || yaml === '' ? fallback : yaml);
    });
    return () => {
      current = false;
    };
  }, [label, fresh]);
  const save = async () => {
    if (text === null) return;
    setSaving(true);
    const done = await onSave(text);
    setSaving(false);
    if (done) onClose();
  };
  return (
    <div class="flm-yaml-editor">
      <span class="flm-yaml-path">{label}</span>
      <textarea
        class="flm-yaml-text"
        aria-label={`The value of ${label}, in YAML`}
        rows={4}
        spellcheck={false}
        value={text ?? ''}
        disabled={disabled || text === null}
        ref={area}
        onInput={(event) => setText((event.currentTarget as HTMLTextAreaElement).value)}
        onKeyDown={(event) => {
          if (event.key === 'Escape') {
            event.preventDefault();
            onClose();
          }
        }}
      />
      <div class="flm-yaml-buttons">
        <button type="button" class="flm-button" onClick={onClose}>Cancel</button>
        <button type="button" class="flm-button flm-button-primary" disabled={disabled || text === null || saving} onClick={() => void save()}>
          Save
        </button>
      </div>
    </div>
  );
}
