// A small hand-written DOM for the unit tests: elements with attributes,
// children and text, and the few methods the tested functions call. Selectors
// are lists of compound selectors (a tag, classes, `[name]`, `[name="value"]`,
// `[name~="value"]`, `[name^="value"]`), with no combinators.

export class FakeText {
  readonly nodeType = 3;
  parentNode: FakeElement | null = null;
  constructor(public textContent: string) {}
  cloneNode(): FakeText {
    return new FakeText(this.textContent);
  }
}

type FakeNode = FakeElement | FakeText;

interface Condition {
  tag: string | null;
  classes: string[];
  attributes: { name: string; op: string; value: string | null }[];
}

function parseCompound(selector: string): Condition {
  const condition: Condition = { tag: null, classes: [], attributes: [] };
  const pattern = /([a-zA-Z][\w-]*)|\.([\w-]+)|\[([\w-]+)(?:([~^]?=)"([^"]*)")?\]/g;
  const rest = selector.trim();
  let match: RegExpExecArray | null;
  let consumed = 0;
  while ((match = pattern.exec(rest)) !== null) {
    if (match.index !== consumed) throw new Error(`unsupported selector: ${selector}`);
    consumed = pattern.lastIndex;
    if (match[1] !== undefined) condition.tag = match[1].toUpperCase();
    else if (match[2] !== undefined) condition.classes.push(match[2]);
    else condition.attributes.push({ name: match[3], op: match[4] ?? '', value: match[5] ?? null });
  }
  if (consumed !== rest.length) throw new Error(`unsupported selector: ${selector}`);
  return condition;
}

export class FakeElement {
  readonly nodeType = 1;
  readonly tagName: string;
  readonly attributes = new Map<string, string>();
  childNodes: FakeNode[] = [];
  parentNode: FakeElement | null = null;

  constructor(tag: string, attributes: Record<string, string> = {}, children: (FakeNode | string)[] = []) {
    this.tagName = tag.toUpperCase();
    for (const [name, value] of Object.entries(attributes)) this.attributes.set(name, value);
    for (const child of children) this.appendChild(typeof child === 'string' ? new FakeText(child) : child);
  }

  get parentElement(): FakeElement | null {
    return this.parentNode;
  }
  get children(): FakeElement[] {
    return this.childNodes.filter((n): n is FakeElement => n.nodeType === 1);
  }
  get id(): string {
    return this.attributes.get('id') ?? '';
  }
  get className(): string {
    return this.attributes.get('class') ?? '';
  }
  set className(value: string) {
    this.attributes.set('class', value);
  }
  get textContent(): string {
    return this.childNodes.map((n) => n.textContent).join('');
  }
  set textContent(value: string) {
    this.childNodes = [];
    this.appendChild(new FakeText(value));
  }
  getAttribute(name: string): string | null {
    return this.attributes.get(name) ?? null;
  }
  setAttribute(name: string, value: string): void {
    this.attributes.set(name, value);
  }
  hasAttribute(name: string): boolean {
    return this.attributes.has(name);
  }
  removeAttribute(name: string): void {
    this.attributes.delete(name);
  }
  appendChild<T extends FakeNode>(child: T): T {
    if (child.parentNode !== null) child.parentNode.childNodes = child.parentNode.childNodes.filter((n) => n !== child);
    child.parentNode = this;
    this.childNodes.push(child);
    return child;
  }
  remove(): void {
    if (this.parentNode === null) return;
    this.parentNode.childNodes = this.parentNode.childNodes.filter((n) => n !== this);
    this.parentNode = null;
  }
  private sibling(step: number): FakeElement | null {
    const siblings = this.parentNode?.children ?? [];
    return siblings[siblings.indexOf(this) + step] ?? null;
  }
  get nextElementSibling(): FakeElement | null {
    return this.sibling(1);
  }
  get previousElementSibling(): FakeElement | null {
    return this.sibling(-1);
  }
  matches(selector: string): boolean {
    return selector.split(',').some((part) => {
      const c = parseCompound(part);
      if (c.tag !== null && c.tag !== this.tagName) return false;
      const classes = this.className.split(/\s+/);
      if (!c.classes.every((name) => classes.includes(name))) return false;
      return c.attributes.every(({ name, op, value }) => {
        const actual = this.getAttribute(name);
        if (actual === null) return false;
        if (value === null) return true;
        if (op === '=') return actual === value;
        if (op === '~=') return actual.split(/\s+/).includes(value);
        return actual.startsWith(value);
      });
    });
  }
  closest(selector: string): FakeElement | null {
    for (let e: FakeElement | null = this; e !== null; e = e.parentNode) if (e.matches(selector)) return e;
    return null;
  }
  querySelectorAll(selector: string): FakeElement[] {
    const out: FakeElement[] = [];
    const walk = (e: FakeElement): void => {
      for (const child of e.children) {
        if (child.matches(selector)) out.push(child);
        walk(child);
      }
    };
    walk(this);
    return out;
  }
  querySelector(selector: string): FakeElement | null {
    return this.querySelectorAll(selector)[0] ?? null;
  }
  contains(other: FakeNode | null): boolean {
    for (let e: FakeNode | null = other; e !== null; e = e.parentNode) if (e === this) return true;
    return false;
  }
  cloneNode(deep = false): FakeElement {
    const copy = new FakeElement(this.tagName);
    for (const [name, value] of this.attributes) copy.attributes.set(name, value);
    if (deep) for (const child of this.childNodes) copy.appendChild(child.cloneNode(true));
    return copy;
  }
}

/** A document: its body, `getElementById` and `createElement`. */
export class FakeDocument {
  readonly body: FakeElement;
  constructor(children: FakeElement[]) {
    this.body = new FakeElement('body', {}, children);
  }
  getElementById(id: string): FakeElement | null {
    return this.body.querySelectorAll('[id]').find((e) => e.id === id) ?? null;
  }
  createElement(tag: string): FakeElement {
    return new FakeElement(tag);
  }
}

/** An element: `h('p.flm-pres-par#x', { 'data-flm-kind': 'ref' }, ...children)`. */
export function h(spec: string, attributes: Record<string, string> = {}, ...children: (FakeNode | string)[]): FakeElement {
  const [, tag, rest] = /^([a-z][\w-]*)(.*)$/.exec(spec) ?? ['', 'div', spec];
  const all: Record<string, string> = { ...attributes };
  const classes: string[] = [];
  for (const [, kind, name] of rest.matchAll(/([.#])([\w-]+)/g)) {
    if (kind === '#') all.id = name;
    else classes.push(name);
  }
  if (classes.length > 0) all.class = classes.join(' ');
  return new FakeElement(tag, all, children);
}

/** The outline of an element: its tag, id and classes, then its children's outlines; text is left out. */
export function outline(e: FakeElement): string {
  const own = e.tagName.toLowerCase() + (e.id ? '#' + e.id : '')
    + e.className.split(/\s+/).filter(Boolean).map((c) => '.' + c).join('');
  const kids = e.children.map(outline);
  return kids.length > 0 ? `${own}(${kids.join(' ')})` : own;
}
