// The Viewer tab: the viewer settings, which say how the author wants the preview shown,
// whatever the document. Each control writes its setting at the change (`POST viewer`) and
// shows the setting as the stream reports it; the green check shows once the server has
// written it. No control reverts a setting. While the file of the settings cannot be read,
// the tab says why and every control is disabled.

import type { ComponentChildren } from 'preact';
import { Choice, Switch } from '../controls/fields';
import { Row, useSaved } from '../controls/row';
import { changeViewer } from '../requests';
import type { Option } from '../state/settings';
import { preview, viewer } from '../state/store';
import type { ViewerChange } from '../../shared/viewer';

/** The themes, as the choice names them. */
const THEME_OPTIONS: Option[] = [
  { label: 'Same as system', value: 'system' },
  { label: 'Light', value: 'light' },
  { label: 'Dark', value: 'dark' },
];

/** The settings that a switch writes. */
type SwitchSetting = 'scroll_to_changes' | 'open_problems_on_errors' | 'wrap_lines';

/** Whether the row's green check shows, and the function that writes `change` from the row. */
function useWrite(): [boolean, (change: ViewerChange) => void] {
  const [saved, run] = useSaved();
  return [saved, (change) => void run(changeViewer(change))];
}

/** A row of a choice: its label, the value shown, the options, what a choice writes, and what shows below it. */
function ChoiceRow({ label, value, options, disabled, change, below }: {
  label: string;
  value: unknown;
  options: readonly Option[];
  disabled: boolean;
  change: (value: unknown) => ViewerChange;
  below?: ComponentChildren;
}) {
  const [saved, write] = useWrite();
  const control = <Choice value={value} options={options} disabled={disabled} onChange={(chosen) => write(change(chosen))} />;
  return <Row label={label} control={control} saved={saved} labels="metadata" disabled={disabled} below={below} />;
}

/** A row of a switch, which writes `setting`. */
function SwitchRow({ label, setting, checked, disabled }: {
  label: string;
  setting: SwitchSetting;
  checked: boolean;
  disabled: boolean;
}) {
  const [saved, write] = useWrite();
  const control = <Switch checked={checked} disabled={disabled} onChange={(on) => write({ [setting]: on })} />;
  return <Row label={label} control={control} saved={saved} labels="metadata" switchFirst disabled={disabled} />;
}

export function ViewerTab() {
  const { editors, settings } = viewer.value;
  const disabled = preview.value.connection === 'disconnected' || settings.unreadable !== null;
  // Every editor the server knows; those not found are grayed out. Choosing the custom
  // command is always possible: the server writes a placeholder command when there is none.
  const editorOptions: Option[] = editors.map((editor) => ({ label: editor.label, value: editor.name, disabled: !editor.available }));
  if (settings.editor === null) editorOptions.unshift({ label: 'None found', value: null, disabled: true });
  return (
    <div class="flm-rows flm-viewer">
      <h3 class="flm-section-heading">Viewer settings</h3>
      {settings.unreadable !== null && <p class="flm-viewer-unreadable">{settings.unreadable}</p>}
      <ChoiceRow
        label="Theme"
        value={settings.theme}
        options={THEME_OPTIONS}
        disabled={disabled}
        change={(theme) => ({ theme: theme as ViewerChange['theme'] })}
      />
      <SwitchRow label="Scroll to changes after each build" setting="scroll_to_changes" checked={settings.scrollToChanges} disabled={disabled} />
      <SwitchRow label="Open Problems when a build has errors" setting="open_problems_on_errors" checked={settings.openProblemsOnErrors} disabled={disabled} />
      <SwitchRow label="Wrap long lines in text outputs" setting="wrap_lines" checked={settings.wrapLines} disabled={disabled} />
      <ChoiceRow
        label="Editor"
        value={settings.editor}
        options={editorOptions}
        disabled={disabled}
        change={(name) => ({ editor: name as string })}
        below={settings.editor === 'custom' && (
          <p class="flm-viewer-hint">Configure your custom editor command in <code>~/.flm/watch.json</code></p>
        )}
      />
      <p class="flm-viewer-file">Edit viewer settings in <code>~/.flm/watch.json</code>.</p>
    </div>
  );
}
