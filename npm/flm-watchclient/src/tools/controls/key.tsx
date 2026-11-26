// The row of one key of the front matter. Its control is the one a description gives, or
// else the one that follows from the shape of the key's schema (`state/settings.ts`,
// `controlOf`): a switch, a choice, a number field, a text field, the chips of a list of
// texts, or *Edit as YAML* with the YAML editor under the row. A choice can end with
// "Custom…", which opens the YAML editor of a path. A key whose schema admits `null` has a
// checkbox before its control: unchecked, the key is given no value (`null`). The row shows
// the value that the front matter writes, else the value in effect, else the default of the
// key's schema, else the default that a description gives. The icon at the right end tells
// where the value comes from, and its tooltip each source's value and the marks; a change
// writes the key, and the revert arrow removes it. A value that a higher source decides is
// named beside the control, which still writes the front matter.

import type { ComponentChildren } from 'preact';
import { useState } from 'preact/hooks';
import type { SettingsState } from '../../shared/settings';
import { write } from '../front-matter';
import type { Control, Edit, Path, Source } from '../state/settings';
import { changed, isWritten, oneLine, preview, reverted, savedYaml, sameValue, textsOf } from '../state/settings';
import { givenEdit, keyControl, marksAt, nullableAt, outdatedAt, shownValue, sourceAt, valueAt, writtenAt } from '../state/settings-panel';
import { Choice, NumberField, Switch, TextField } from './fields';
import type { Labels } from './row';
import { Label, Row, SourceIcon, UnknownMark, useSaved } from './row';
import { TextList } from './texts';
import { YamlEditor, EditAsYaml } from './yaml';

export function KeyRow({ label, path, state, control: given, fallback, custom, after, labels, disabled, unknown = false, id }: {
  /** The row's label; a name between backquotes is code. */
  label: string;
  /** The keys from the root of the front matter to the key. */
  path: Path;
  state: SettingsState;
  /** The control that a description gives; else the one of the key's schema (`keyControl`). */
  control?: Control;
  /** The key's default, for a key whose schema gives none: the value shown where no source writes the key. */
  fallback?: unknown;
  /** For a choice: the path whose YAML editor the option "Custom…" opens. */
  custom?: Path;
  /** A link that follows the control. */
  after?: ComponentChildren;
  labels: Labels;
  disabled: boolean;
  /** Whether the row carries the mark that no package in use has the key. */
  unknown?: boolean;
  /** The id of the row's element, for a link that shows the row. */
  id?: string;
}) {
  const [saved, run] = useSaved();
  const [editing, setEditing] = useState(false);
  // Whether the author checked the box of a key given no value, before its control gives one.
  const [giving, setGiving] = useState(false);
  const control = given ?? keyControl(state, path);
  const nullable = nullableAt(state, path);
  const written = valueAt(state.frontMatter, path);
  // The custom path, when it is another key: the front matter may write it beside the key.
  const other = custom !== undefined && !sameValue(custom, path) ? custom : null;
  const otherWritten = other !== null && isWritten(valueAt(state.frontMatter, other));
  const pathSource = sourceAt(state, path, nullable);
  let source: Source = pathSource;
  if (otherWritten) source = pathSource.kind === 'decided' ? { ...pathSource, written: true } : { kind: 'written' };
  // After a preparation that failed, a value that the front matter does not write is that of
  // the last successful build: the chips, which write the list they show, are disabled, and
  // the YAML editor starts empty.
  const outdated = source.kind === 'outdated';
  const shown = shownValue(state, path, nullable);
  const value = shown === undefined ? fallback : shown;
  const hasValue = !nullable || giving || (value !== null && value !== undefined);
  const locked = disabled || !hasValue;
  // The control's own name, where the checkbox before it keeps the row's label from naming it.
  const name = nullable ? label.replace(/`/g, '') : undefined;
  const send = (edit: Edit) => run(write([edit]));
  // The writes of the revert arrow: the key, and the custom path where the front matter writes it.
  const reverts = [
    ...(writtenAt(state, path, nullable) ? [reverted(path)] : []),
    ...(otherWritten && other !== null ? [reverted(other)] : []),
  ];
  const yamlPath = control.kind === 'choice' && custom !== undefined ? custom : path;
  // The YAML editor of the key itself starts from the default that the description gives.
  const yamlFallback = fallback !== undefined && yamlPath === path ? oneLine(fallback) : undefined;
  let field;
  switch (control.kind) {
    case 'switch':
      field = <Switch checked={value === true} name={name} disabled={locked} onChange={(checked) => void send(changed(path, checked))} />;
      break;
    case 'choice': {
      const customShown = custom !== undefined
        && (editing || otherWritten || (value !== undefined && value !== null && !control.options.some((option) => sameValue(option.value, value))));
      field = (
        <Choice
          value={value}
          options={control.options}
          name={name}
          disabled={locked}
          custom={custom === undefined ? undefined : { shown: customShown, onChoose: () => setEditing(true) }}
          onChange={(chosen) => {
            setEditing(false);
            // A choice of an option replaces the value of the custom path that the front matter writes.
            void run(write([changed(path, chosen), ...(otherWritten && other !== null ? [reverted(other)] : [])]));
          }}
        />
      );
      break;
    }
    case 'number':
      field = (
        <NumberField
          value={typeof value === 'number' ? value : null}
          integer={control.integer}
          name={name}
          disabled={locked}
          onCommit={(number) => send(changed(path, number))}
        />
      );
      break;
    case 'text':
      field = <TextField value={typeof value === 'string' ? value : ''} name={name} disabled={locked} onCommit={(text) => send(changed(path, text))} />;
      break;
    case 'texts':
      field = <TextList texts={textsOf(value)} disabled={locked || outdated} name={label} add="Add" onChange={(texts) => send(changed(path, texts))} />;
      break;
    case 'yaml':
      field = <EditAsYaml preview={preview(written)} open={editing} disabled={locked} onOpen={() => setEditing(true)} />;
      break;
  }
  const box = nullable && (
    <input
      type="checkbox"
      class="flm-given"
      aria-label={`Give ${label.replace(/`/g, '')} a value`}
      title="Unchecked: no value (null)"
      checked={hasValue}
      disabled={disabled}
      onChange={(event) => {
        const checked = (event.currentTarget as HTMLInputElement).checked;
        setGiving(checked);
        const edit = givenEdit(path, checked);
        if (edit !== null) void send(edit);
      }}
    />
  );
  return (
    <Row
      labels={labels}
      label={<><Label text={label} />{unknown && <UnknownMark />}</>}
      control={box ? <>{box}{field}</> : field}
      after={after}
      saved={saved}
      switchFirst={control.kind === 'switch'}
      named={!nullable && control.kind !== 'texts' && control.kind !== 'yaml'}
      disabled={disabled}
      outdated={outdated}
      id={id}
      end={(
        <SourceIcon
          source={source}
          disabled={disabled}
          tip={{ path, marks: marksAt(state, path) }}
          onRevert={() => {
            setGiving(false);
            setEditing(false);
            void run(write(reverts));
          }}
        />
      )}
      below={editing && (
        <YamlEditor
          path={yamlPath}
          fresh={outdatedAt(state, yamlPath)}
          fallback={yamlFallback}
          disabled={locked}
          onSave={(yaml) => send(savedYaml(yamlPath, yaml))}
          onClose={() => setEditing(false)}
        />
      )}
    />
  );
}
