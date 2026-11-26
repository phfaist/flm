// The marks of the last build's problems: the elements each error or warning lies in
// carry the attribute `MARK`, whose value is the worst severity among them, and the
// stylesheet outlines them.

import type { Problem, Severity } from '../shared/protocol';
import { SEVERITIES, partOf } from '../shared/protocol';
import { MARK } from './attributes';
import type { Placed } from './places';
import { elementsOf } from './places';

/** The severities that mark the elements a problem lies in. */
const MARKED: readonly Severity[] = ['error', 'warning'];

/**
 * The mark of each element `problems` lie in, among `placed`: the worst severity among
 * the errors and warnings that lie in it.
 */
export function marksOf(problems: readonly Problem[], placed: readonly Placed[]): Map<Element, Severity> {
  const marks = new Map<Element, Severity>();
  for (const problem of problems) {
    const part = partOf(problem);
    if (!MARKED.includes(problem.severity) || part === null) continue;
    for (const element of elementsOf(part, placed)) {
      const now = marks.get(element);
      if (now === undefined || SEVERITIES.indexOf(problem.severity) < SEVERITIES.indexOf(now)) {
        marks.set(element, problem.severity);
      }
    }
  }
  return marks;
}

/** Marks the elements `problems` lie in, and takes the marks of earlier problems off. */
export function mark(problems: readonly Problem[], placed: readonly Placed[]): void {
  for (const element of Array.from(document.querySelectorAll(`[${MARK}]`))) element.removeAttribute(MARK);
  for (const [element, severity] of marksOf(problems, placed)) element.setAttribute(MARK, severity);
}
