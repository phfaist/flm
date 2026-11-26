// The Problems tab: the problems of the last build, by file, one row for problems alike.
// A click on a row selects it: the document scrolls to the problem's elements and flashes
// them, and the row shows the line of the source, the "inside" chain and, when the program
// gives the help pages, the Related line. A click on the selected row collapses it. Open
// in editor, or Enter on a row, opens the problem's place.

import { useEffect, useState } from 'preact/hooks';
import type { Construct } from '../../shared/help';
import type { Frame, Problem } from '../../shared/protocol';
import { openTargetOf, partOf } from '../../shared/protocol';
import { Icon } from '../controls/icon';
import type { IconName } from '../controls/icon';
import { searchHelp, showHelp, useHelpIndex } from '../help';
import { constructsAt, openPlace } from '../requests';
import { relatedOf } from '../state/help';
import type { Row } from '../state/problems';
import { placeText, rowsOf } from '../state/problems';
import { preview, selectedProblem } from '../state/store';
import { toPage } from '../views/channel';

/** The icon of each severity, and its name. */
export const SEVERITY_ICONS: Record<Problem['severity'], { icon: IconName; label: string }> = {
  error: { icon: 'circle-x', label: 'Error' },
  warning: { icon: 'triangle-alert', label: 'Warning' },
  note: { icon: 'info', label: 'Note' },
};

/** The count beside the tab's name: the number of problems, or none when there is none. */
export function problemCount(): string | null {
  const count = preview.value.problems.length;
  return count > 0 ? String(count) : null;
}

/** Opens the place of `problem` in the editor, when it has one to open (`openTargetOf`). */
function open(problem: Problem): void {
  const target = openTargetOf(problem);
  if (target !== null) void openPlace(target);
}

/** Selects `row`, and shows its problem in the document; or collapses it when selected. */
function pick(row: Row): void {
  if (selectedProblem.value === row.key) {
    selectedProblem.value = null;
    return;
  }
  selectedProblem.value = row.key;
  const part = partOf(row.problems[0]!);
  if (part !== null) toPage({ type: 'show-place', place: part });
}

function Excerpt({ problem }: { problem: Problem }) {
  const { excerpt } = problem;
  if (excerpt === null) return null;
  return (
    <div class="flm-excerpt">
      <span class="flm-excerpt-line">{excerpt.line}  </span>
      {excerpt.before}
      <span class="flm-excerpt-covered" data-severity={problem.severity}>{excerpt.covered}</span>
      {excerpt.after}
    </div>
  );
}

/** One "inside" chain, with each place beside the file `file`. */
function Inside({ frames, file }: { frames: readonly Frame[]; file: string | null }) {
  return (
    <ol class="flm-inside">
      {frames.map((frame, index) => (
        <li key={index}>
          <span class="flm-inside-word">inside</span> {frame.title}{' '}
          <span class="flm-place">{placeText(frame.place, file)}</span>
        </li>
      ))}
    </ol>
  );
}

function ProblemRow({ row, file, disconnected }: { row: Row; file: string | null; disconnected: boolean }) {
  const problem = row.problems[0]!;
  const selected = selectedProblem.value === row.key;
  const { icon, label } = SEVERITY_ICONS[problem.severity];
  const count = row.problems.length;
  const openable = openTargetOf(problem) !== null;
  const chains = row.problems.filter((each) => each.frames.length > 0);
  return (
    <li class="flm-problem" data-selected={selected}>
      <div class="flm-problem-head">
        <button
          type="button"
          class="flm-problem-row"
          aria-expanded={selected}
          onClick={() => pick(row)}
          onKeyDown={(event) => {
            if (event.key !== 'Enter') return;
            event.preventDefault();
            if (openable && !disconnected) open(problem);
          }}
        >
          <span class={`flm-severity flm-tone-${problem.severity}`}>
            <Icon name={icon} size={16} label={label} />
          </span>
          <span class="flm-problem-text">
            <span class="flm-problem-message">
              {problem.message}
              {count > 1 && <span class="flm-problem-times" aria-label={`${count} times`}>×{count}</span>}
            </span>
            <span class="flm-place">{placeText(problem.place, file)}</span>
          </span>
        </button>
        <button
          type="button"
          class="flm-icon-button flm-open"
          aria-label="Open in editor"
          title="Open in editor"
          disabled={!openable || disconnected}
          onClick={() => open(problem)}
        >
          <Icon name="square-arrow-out-up-right" size={15} />
        </button>
      </div>
      {selected && (
        <div class="flm-problem-details">
          <Excerpt problem={problem} />
          {chains.map((each, index) => (
            <Inside key={index} frames={each.frames} file={file} />
          ))}
          {preview.value.help && <Related problem={problem} />}
        </div>
      )}
    </li>
  );
}

/** The Related line of a selected problem that lies in a part of a source file, when the help pages are given. */
function Related({ problem }: { problem: Problem }) {
  const part = partOf(problem);
  const index = useHelpIndex();
  const build = preview.value.build;
  const [constructs, setConstructs] = useState<Construct[] | null>(null);
  useEffect(() => {
    if (part === null) return undefined;
    let wanted = true;
    void constructsAt(part.file, part.start).then((answer) => {
      if (wanted) setConstructs('value' in answer ? answer.value.constructs : null);
    });
    return () => {
      wanted = false;
    };
  }, [part?.file, part?.start, build]);
  const pages = index !== null && 'value' in index ? index.value.pages : [];
  const links = constructs === null ? [] : relatedOf(constructs, pages);
  if (links.length === 0) return null;
  return (
    <div class="flm-related">
      <span class="flm-related-word">Related:</span>
      {links.map((link) => (
        <button
          key={link.text}
          type="button"
          class="flm-link"
          onClick={() => ('page' in link ? showHelp(link.page) : searchHelp(link.search))}
        >
          {link.text}
        </button>
      ))}
    </div>
  );
}

export function ProblemsTab() {
  const state = preview.value;
  if (state.durationMs === null) return null;
  if (state.problems.length === 0) return <p class="flm-panel-line">No problems</p>;
  const disconnected = state.connection === 'disconnected';
  return (
    <div class="flm-problems">
      {rowsOf(state.problems).map((group) => (
        <section key={group.file ?? ''} aria-label={group.file ?? 'Problems'}>
          {group.file !== null && <h2 class="flm-heading">{group.file}</h2>}
          <ul class="flm-problem-list">
            {group.rows.map((row) => (
              <ProblemRow key={row.key} row={row} file={group.file} disconnected={disconnected} />
            ))}
          </ul>
        </section>
      ))}
    </div>
  );
}
