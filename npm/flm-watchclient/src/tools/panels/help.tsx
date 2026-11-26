// The Help tab, and the help browser alone (the tools page at `/.flm-preview/help`): the tool
// row (Back, Forward, Home, the search field, and in the tab Open this page in a new tab),
// then the page shown or, while the search field holds text, the pages that match it. A page
// shows its path line, its title and kind, and its content as the program rendered it; a
// link between pages is taken by the panel, which adds the page to its history. A result of
// the search, clicked, adds the search and then its page, so that Back shows the results.

import { Fragment } from 'preact';
import { useEffect, useRef, useState } from 'preact/hooks';
import type { HelpIndex, HelpPage, IndexEntry, PageAnswer } from '../../shared/help';
import { HELP_BROWSER } from '../../shared/help';
import { Icon } from '../controls/icon';
import { helpView, showFirst, showPage, showResult, showVisit, useHelpIndex } from '../help';
import type { Answer } from '../requests';
import { helpPage } from '../requests';
import {
  back, canGoBack, canGoForward, currentPage, entryAt, forward, fragmentOf, homePage, pathOfFragment, pathOfLink, search,
} from '../state/help';
import { preview, showSettingsOf } from '../state/store';

/** The most results listed at once. */
const MOST_RESULTS = 100;

/** A page's kind as the panel shows it: `feature-index` as "feature index". */
function kindText(kind: string): string {
  return kind.replace(/-/g, ' ');
}

/**
 * The page's content, as the program rendered it. A click on a link of the kind
 * `flmdoc.page` shows its page in the panel; a link within the page scrolls to its target;
 * any other link opens in a new tab of the browser.
 */
function Content({ html }: { html: string }) {
  const element = useRef<HTMLDivElement>(null);
  const click = (event: MouseEvent) => {
    const link = (event.target as Element | null)?.closest('a[href]');
    const href = link?.getAttribute('href');
    if (link == null || href == null) return;
    event.preventDefault();
    if (link.getAttribute('data-flm-kind') === 'flmdoc.page') showPage(pathOfLink(href));
    else if (href.startsWith('#')) element.current?.querySelector(`[id="${CSS.escape(href.slice(1))}"]`)?.scrollIntoView();
    else window.open(href, '_blank', 'noopener');
  };
  return <div ref={element} class="flm-help-content" onClick={click} dangerouslySetInnerHTML={{ __html: html }} />;
}

/** The page's path line, as the program gives it: every segment but the last, the page itself, links to its page. */
function PathLine({ segments }: { segments: HelpPage['pathLine'] }) {
  if (segments.length < 2) return null;
  return (
    <nav class="flm-help-path" aria-label="Path">
      {segments.map((segment, at) => (
        <Fragment key={at}>
          {at > 0 && <span aria-hidden="true">›</span>}
          {at < segments.length - 1
            ? <button type="button" class="flm-help-link" onClick={() => showPage(segment.path)}>{segment.text}</button>
            : <span class="flm-help-path-here">{segment.text}</span>}
        </Fragment>
      ))}
    </nav>
  );
}

/** The page of `answer`, or the line that says that the path has none, with the nearest pages. */
function PageView({ answer, alone }: { answer: PageAnswer; alone: boolean }) {
  const { page } = answer;
  if (page === null) {
    return (
      <div class="flm-help-missing">
        <p class="flm-help-line">No help page ‘{answer.path}’</p>
        <ul class="flm-help-list">
          {answer.nearest.map((each) => (
            <li key={each.path}>
              <button type="button" class="flm-help-link" onClick={() => showPage(each.path)}>{each.title}</button>
            </li>
          ))}
        </ul>
      </div>
    );
  }
  const feature = page.settingsFeature;
  return (
    <article class="flm-help-page">
      <PathLine segments={page.pathLine} />
      <div class="flm-help-title">
        <h2>{page.title}</h2>
        <span class="flm-help-kind">{kindText(page.kind)}</span>
      </div>
      <Content html={page.content} />
      {feature !== null && !alone && (
        <div class="flm-help-settings">
          <button type="button" class="flm-help-link" onClick={() => showSettingsOf(feature)}>Settings of this feature</button>
        </div>
      )}
    </article>
  );
}

/** The pages that match the search, best first, with the toggle of the internal names. */
function Results({ pages, query, internal }: { pages: readonly IndexEntry[]; query: string; internal: boolean }) {
  const found = search(pages, query, internal);
  return (
    <div class="flm-help-results">
      <label class="flm-checkbox flm-help-internal">
        <input
          type="checkbox"
          checked={internal}
          onChange={(event) => (helpView.value = { ...helpView.value, internal: (event.currentTarget as HTMLInputElement).checked })}
        />
        <span>Internal names</span>
      </label>
      {found.length === 0 && <p class="flm-help-line">No help page matches ‘{query.trim()}’</p>}
      <ul class="flm-help-list">
        {found.slice(0, MOST_RESULTS).map((entry) => (
          <li key={entry.path}>
            <button type="button" class="flm-help-result" onClick={() => showResult(entry.path)}>
              <span class="flm-help-result-head">
                <span class="flm-help-result-title">{entry.title}</span>
                <span class="flm-help-kind">{kindText(entry.kind)}</span>
              </span>
              {entry.summary !== '' && <span class="flm-help-result-summary">{entry.summary}</span>}
            </button>
          </li>
        ))}
      </ul>
      {found.length > MOST_RESULTS && <p class="flm-help-line">{found.length - MOST_RESULTS} more</p>}
    </div>
  );
}

/** The page at `path`, read again when the environment changes; the page read last while it is read. */
function usePage(path: string | null): Answer<PageAnswer> | null {
  const environment = preview.value.environment;
  const [shown, setShown] = useState<{ path: string; answer: Answer<PageAnswer> } | null>(null);
  useEffect(() => {
    if (path === null) return undefined;
    let wanted = true;
    void helpPage(path).then((answer) => {
      if (wanted) setShown({ path, answer });
    });
    return () => {
      wanted = false;
    };
  }, [path, environment]);
  return shown?.answer ?? null;
}

/** The body of the panel: the results of the search, or the page shown. */
function Body({ index, alone }: { index: HelpIndex; alone: boolean }) {
  const { history, query, internal } = helpView.value;
  const path = currentPage(history);
  const answer = usePage(path);
  const body = useRef<HTMLDivElement>(null);
  // The first page shown is the home page.
  useEffect(() => {
    const home = homePage(index.pages);
    if (path === null && home !== null) showFirst(home);
  }, [path, index]);
  useEffect(() => {
    body.current?.scrollTo({ top: 0 });
  }, [answer, query === '']);
  let content = null;
  if (query.trim() !== '') content = <Results pages={index.pages} query={query} internal={internal} />;
  else if (answer !== null && 'failure' in answer) content = <p class="flm-help-line">{answer.failure}</p>;
  else if (answer !== null) content = <PageView answer={answer.value} alone={alone} />;
  return <div ref={body} class="flm-help-body">{content}</div>;
}

/** The tool row; `home` is the path of the home page, `null` while it is not known. */
function ToolRow({ alone, home }: { alone: boolean; home: string | null }) {
  const { history, query } = helpView.value;
  const path = currentPage(history);
  return (
    <div class="flm-help-tools">
      <button type="button" class="flm-icon-button" aria-label="Back" title="Back" disabled={!canGoBack(history)} onClick={() => showVisit(back(history))}>
        <Icon name="chevron-left" size={16} />
      </button>
      <button type="button" class="flm-icon-button" aria-label="Forward" title="Forward" disabled={!canGoForward(history)} onClick={() => showVisit(forward(history))}>
        <Icon name="chevron-right" size={16} />
      </button>
      <button type="button" class="flm-icon-button" aria-label="Home" title="Home: all the help pages" disabled={home === null} onClick={() => home !== null && showPage(home)}>
        <Icon name="house" size={15} />
      </button>
      <label class="flm-search">
        <Icon name="search" size={14} />
        <input
          type="search"
          aria-label="Search the help pages"
          placeholder="Search commands, settings, features"
          spellcheck={false}
          value={query}
          onInput={(event) => (helpView.value = { ...helpView.value, query: (event.currentTarget as HTMLInputElement).value })}
        />
      </label>
      {!alone && (
        <button
          type="button"
          class="flm-icon-button"
          aria-label="Open this page in a new tab"
          title="Open this page in a new tab"
          disabled={path === null}
          onClick={() => window.open(`${HELP_BROWSER}#${fragmentOf(path ?? '')}`, '_blank', 'noopener')}
        >
          <Icon name="square-arrow-out-up-right" size={15} />
        </button>
      )}
    </div>
  );
}

/** The help: in the panel's tab, or alone (`alone`), in the help browser. */
function Help({ alone }: { alone: boolean }) {
  const index = useHelpIndex();
  let body;
  const home = index !== null && 'value' in index ? homePage(index.value.pages) : null;
  if (index === null) body = <p class="flm-help-line flm-help-reading">Reading the help pages…</p>;
  else if ('failure' in index) body = <p class="flm-help-line">{index.failure}</p>;
  else body = <Body index={index.value} alone={alone} />;
  return (
    <div class="flm-help" data-alone={alone ? 'true' : undefined}>
      <ToolRow alone={alone} home={home} />
      {body}
    </div>
  );
}

export function HelpTab() {
  return <Help alone={false} />;
}

/**
 * The help browser alone: the page that the address's fragment names, else the first of the
 * index. The fragment follows the page shown, and a fragment changed by hand shows its page.
 */
export function HelpBrowser() {
  const { history } = helpView.value;
  const path = currentPage(history);
  const index = useHelpIndex();
  const title = index !== null && 'value' in index && path !== null ? entryAt(index.value.pages, path)?.title : undefined;
  useEffect(() => {
    const follow = () => {
      const wanted = pathOfFragment(window.location.hash);
      if (wanted !== null) showPage(wanted);
    };
    follow();
    window.addEventListener('hashchange', follow);
    return () => window.removeEventListener('hashchange', follow);
  }, []);
  useEffect(() => {
    if (path === null) return;
    const fragment = `#${fragmentOf(path)}`;
    if (window.location.hash !== fragment) window.history.replaceState(null, '', fragment);
  }, [path]);
  useEffect(() => {
    document.title = title !== undefined ? `${title} – FLM help` : 'FLM help';
  }, [title]);
  return <Help alone />;
}
