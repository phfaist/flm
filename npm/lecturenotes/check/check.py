# The browser smoke check of the lecture-notes page: renders the sample document
# (`flm/tests/documents/lecturenotes/notes.flm`) and the short documents
# `plain.flm` and `levels.flm` beside this file with the extension (`extensions/data/lecturenotes/`,
# the bundle `dist/` of its compose template as it stands), serves them from a local HTTP
# server, opens them in Google Chrome with no window, and asserts through the
# page's own DOM:
#
#   - the sidebar's entries and the content's headings carry their levels, and are set
#     by the groups of the levels: in the sample (the top heading level 0, the
#     navigation heading level 1), and in `levels.flm`, a document with the levels 0 to
#     3 (the top heading level 0, the navigation heading level 2), where the entries
#     below a unit's entry fold;
#   - in `levels.flm`, the setting `css`, written with the selector of the group `top`,
#     reaches the page in a `<style>` element after the template's stylesheet and sets
#     the headings and the entries of that group in capitals, and only those;
#   - the current entry of the sidebar follows a scroll;
#   - the sidebar button hides the sidebar and shows it again;
#   - hovering a term, a theorem, an equation and the `[3–5]` mark opens the
#     popup with the expected content; a long press does the same on a touch
#     screen;
#   - a document with no footnotes and no references shows no back-matter rule;
#   - with each of the other math setups (MathJax writing MathML, MathJax in its
#     lazy mode, math-passthrough, math-techxt; the sample rendered again as
#     `notes-NAME.html`), hovering an equation reference shows the paragraph of
#     the formula, with its formulas;
#   - in MathJax's lazy mode, which typesets a formula only when it scrolls into
#     view, an equation line has its anchor before its formula is typeset, a
#     popup typesets the formulas it shows, and a link to the line leads to it.
#
# It also writes two pictures of the top of the sample page, at 1280 and at 600
# pixels wide (`top.png`, `top-600.png` in the output folder).
#
# Run by hand, not by the test suite. It needs the Python interpreter that has
# Playwright installed and a built `flm` program:
#
#   /Users/philippe/.pyenv/versions/3.13.2/envs/py313env-cmds/bin/python \
#       npm/lecturenotes/check/check.py target/debug/flm target/lecturenotes-check
#
# The output folder is emptied first. Google Chrome is taken from its usual place
# on macOS, or from the environment variable FLM_CHROMIUM. The page loads the
# formula renderer from the internet; offline, the formulas stay as their source,
# which the checks accept. Each check prints `ok` or `FAILED`; the program ends
# with a failing status if any failed.

import functools
import http.server
import os
import pathlib
import shutil
import socketserver
import subprocess
import sys
import threading

from playwright.sync_api import sync_playwright

HERE = pathlib.Path(__file__).resolve().parent
ROOT = HERE.parents[2]
CHROME = os.environ.get('FLM_CHROMIUM', '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome')

failures = []


def check(name, condition, detail=''):
    print(('ok      ' if condition else 'FAILED  ') + name + (f'  ({detail})' if detail else ''))
    if not condition:
        failures.append(name)


# The other math setups the sample is rendered with: a name, and the options of `flm render`.
MATH_SETUPS = [
    ('mathml', ['-O', 'math-mathjax.mode=mathml']),
    ('lazy', ['-O', 'math-mathjax.lazy=true']),
    ('passthrough', ['--math', 'passthrough']),
    ('techxt', ['--math', 'techxt']),
]

# A formula in the page, whichever processor wrote it: the mark of its outermost element.
FORMULA = '.flm-math-formula'


def render(flm, out):
    if out.exists():
        shutil.rmtree(out)
    out.mkdir(parents=True)
    for name in ['notes.flm', 'cycle.svg']:
        shutil.copy(ROOT / 'flm/tests/documents/lecturenotes' / name, out / name)
    for name in ['plain.flm', 'levels.flm']:
        shutil.copy(HERE / name, out / name)
    runs = [('notes', [], 'notes'), ('plain', [], 'plain'), ('levels', [], 'levels')]
    runs += [('notes', options, f'notes-{name}') for name, options in MATH_SETUPS]
    for document, options, output in runs:
        subprocess.run(
            [flm, 'render', '--extensions-dir', str(ROOT / 'extensions/data'), *options, '-o', f'{output}.html',
             f'{document}.flm'],
            cwd=out, check=True, env={**os.environ, 'HOME': str(out)}, stderr=subprocess.DEVNULL)


class QuietHandler(http.server.SimpleHTTPRequestHandler):
    def log_message(self, *args):
        pass


def serve(out):
    handler = functools.partial(QuietHandler, directory=str(out))
    server = socketserver.TCPServer(('127.0.0.1', 0), handler)
    threading.Thread(target=server.serve_forever, daemon=True).start()
    return server


# The popup's content: its top-level elements' tags and classes, and its text.
POPUP = r"""
() => {
  const popup = document.querySelector('div.ln-preview');
  if (!popup || !popup.classList.contains('ln-preview-shown')) return null;
  const content = popup.querySelector('.ln-preview-content') || popup;
  return {
    parts: [...content.children].map((e) => e.tagName.toLowerCase() + '.' + [...e.classList].join('.')),
    text: content.textContent.replace(/\s+/g, ' ').trim(),
    items: [...content.querySelectorAll('li')].map((li) => li.textContent.replace(/\s+/g, ' ').trim().slice(0, 4)),
    ids: content.querySelectorAll('[id]').length,
  };
}
"""


def hover_preview(page, href):
    page.mouse.move(5, 300)
    page.wait_for_timeout(450)
    link = page.locator(f'#flm-content a[href="{href}"]').first
    link.scroll_into_view_if_needed()
    page.wait_for_timeout(100)
    # Wait until the link stays in place: formulas typeset after the scroll
    # (MathJax's lazy mode typesets those that come into view) move the text
    # around it, and a link that moves away from the pointer shows no popup.
    place = None
    for _ in range(20):
        now = link.bounding_box()
        if now == place:
            break
        place = now
        page.wait_for_timeout(150)
    link.scroll_into_view_if_needed()
    link.hover()
    page.wait_for_timeout(450)
    return page.evaluate(POPUP)


def check_sample(browser, base):
    context = browser.new_context(viewport={'width': 1280, 'height': 900})
    page = context.new_page()
    page.goto(base + 'notes.html')
    page.wait_for_load_state('load')
    page.wait_for_timeout(500)
    page.screenshot(path=str(OUT / 'top.png'))

    # The sidebar's entries and the content's headings, set by the groups of their levels.
    # The sample's top heading level is 0 (its parts) and its navigation heading level 1.
    check_groups(page, 'the sample', navigation=1, levels={'0', '1', '2'})
    family = page.evaluate("() => getComputedStyle(document.body).fontFamily")
    check('the text is set in the font family of the settings', family.startswith('Georgia'), family)
    bar_links = page.evaluate("() => [...document.querySelectorAll('.ln-topbar .ln-topbar-links a.ln-topbar-link')].map((a) => a.textContent)")
    check('the bar holds the links of the settings', bar_links == ['Course page', 'Exercises'], str(bar_links))

    # The current entry follows a scroll.
    def current_after_scroll(target):
        page.evaluate(f"() => document.getElementById('{target}').scrollIntoView()")
        page.evaluate("() => window.scrollBy(0, 40)")
        page.wait_for_timeout(700)
        return page.evaluate("""() => {
            const link = document.querySelector('.ln-sidebar a[aria-current="true"]');
            return link ? link.getAttribute('href') : null; }""")
    first = current_after_scroll('sec-encoding')
    check('the current entry is the section scrolled to', first == '#sec-encoding', str(first))
    second = current_after_scroll('sec-distance')
    check('the current entry follows a second scroll', second == '#sec-distance', str(second))
    unit = page.evaluate("() => document.querySelector('.ln-current-unit').textContent.trim()")
    check('the bar shows the unit being read', len(unit) > 0, unit)
    page.evaluate("() => window.scrollTo(0, 0)")
    page.wait_for_timeout(300)

    # The sidebar button.
    def sidebar_shown():
        return page.evaluate("() => { const s = document.querySelector('.ln-sidebar'); "
                             "return getComputedStyle(s).display !== 'none' && s.getBoundingClientRect().width > 0; }")
    check('the sidebar is shown at first', sidebar_shown())
    page.click('.ln-sidebar-toggle')
    page.wait_for_timeout(200)
    check('the sidebar button hides the sidebar', not sidebar_shown())
    page.click('.ln-sidebar-toggle')
    page.wait_for_timeout(200)
    check('the sidebar button shows it again', sidebar_shown())

    # The hover previews.
    term = hover_preview(page, '#defterm-code')
    check('hovering a term shows its definition',
          term is not None and any('flm-defterm' in part for part in term['parts']), str(term and term['parts']))
    theorem = hover_preview(page, '#theorem-1')
    check('hovering a theorem reference shows the theorem',
          theorem is not None and 'Theorem' in theorem['text'] and theorem['ids'] == 0,
          str(theorem and theorem['text'][:60]))
    equation = hover_preview(page, '#eq-tp')
    parts = equation and equation['parts']
    check('hovering an equation reference shows the paragraph the formula stands in',
          parts == ['p.flm-pres-par'] and 'The channel' in equation['text'] and 'Condition' in equation['text'],
          str(parts))
    mark = hover_preview(page, '#citation-5')
    check('hovering the [3–5] mark lists the works 3, 4 and 5',
          mark is not None and [i[:3] for i in mark['items']] == ['[3]', '[4]', '[5]'],
          str(mark and mark['items']))
    context.close()

    # The long press, on a touch screen.
    context = browser.new_context(viewport={'width': 600, 'height': 900}, has_touch=True, is_mobile=True)
    page = context.new_page()
    page.goto(base + 'notes.html')
    page.wait_for_load_state('load')
    page.wait_for_timeout(500)
    page.screenshot(path=str(OUT / 'top-600.png'))
    page.evaluate("() => document.querySelector('#flm-content a[href=\"#citation-3\"]').scrollIntoView({block: 'center'})")
    page.wait_for_timeout(300)
    press = """(phase) => {
        const link = document.querySelector('#flm-content a[href="#citation-3"]');
        const box = link.getBoundingClientRect();
        link.dispatchEvent(new PointerEvent(phase, {bubbles: true, cancelable: true, pointerType: 'touch',
            pointerId: 7, isPrimary: true, clientX: box.left + 2, clientY: box.top + 2}));
    }"""
    page.evaluate(press, 'pointerdown')
    page.wait_for_timeout(250)
    early = page.evaluate(POPUP)
    page.wait_for_timeout(450)
    held = page.evaluate(POPUP)
    page.evaluate(press, 'pointerup')
    check('a short touch opens no popup', early is None)
    check('a long press on [3] lists the works 3, 4 and 5',
          held is not None and [i[:3] for i in held['items']] == ['[3]', '[4]', '[5]'], str(held and held['items']))
    context.close()


# The sidebar's entries: the level of each one's heading, the level it carries, and the
# weight and the color of its link; and the headings of the content, with their levels and
# whether they are set as a band. The table is all open while it is read.
GROUPS = r"""
() => {
  document.documentElement.classList.add('ln-toc-all');
  const entries = [...document.querySelectorAll('.ln-sidebar li')].map((li) => {
    const link = li.querySelector(':scope > a');
    const heading = document.getElementById(decodeURIComponent(link.getAttribute('href').slice(1)));
    const style = getComputedStyle(link);
    return {level: heading.getAttribute('data-flm-level'), entry_level: li.getAttribute('data-flm-level'),
            weight: style.fontWeight, color: style.color, space: getComputedStyle(li).marginTop,
            current: link.getAttribute('aria-current') === 'true'};
  });
  document.documentElement.classList.remove('ln-toc-all');
  const headings = [...document.querySelectorAll('#flm-content .flm-pres-block--heading')].map((h) => ({
    level: h.getAttribute('data-flm-level'),
    band: getComputedStyle(h).borderLeftStyle !== 'none',
    weight: getComputedStyle(h).getPropertyValue('--flm-heading-weight').trim()}));
  return {entries, headings};
}
"""


def check_groups(page, document, navigation, levels):
    """The looks of the groups `major` (a level of at most `navigation`) and `minor` (a
    greater level), for the headings and the entries of a page whose levels are `levels`."""
    found = page.evaluate(GROUPS)
    entries, headings = found['entries'], found['headings']
    major = lambda e: int(e['level']) <= navigation
    check(f'{document}: the sidebar has entries at the levels {", ".join(sorted(levels))}',
          {e['level'] for e in entries} == levels, f'{len(entries)} entries')
    check(f"{document}: each entry carries its heading's level", all(e['entry_level'] == e['level'] for e in entries))
    others = [e for e in entries if not e['current']]
    looks_major = {(e['weight'], e['color'], e['space']) for e in others if major(e)}
    looks_minor = {(e['weight'], e['color'], e['space']) for e in others if not major(e)}
    check(f'{document}: the entries at the navigation level and above are in bold, in one color, with a space above',
          len(looks_major) == 1 and next(iter(looks_major))[0] == '600' and next(iter(looks_major))[2] != '0px',
          str(looks_major))
    check(f'{document}: the entries below the navigation level are not in bold, in one other color',
          len(looks_minor) == 1 and next(iter(looks_minor))[0] == '400'
          and {l[1] for l in looks_minor} != {l[1] for l in looks_major}, str(looks_minor))
    check(f'{document}: the headings at the navigation level and above are bands, and the others are not',
          {h['level'] for h in headings} == levels and all(h['band'] == major(h) for h in headings),
          str(sorted({(h['level'], h['band']) for h in headings})))
    weights = {h['weight'] for h in headings if not major(h)}
    check(f'{document}: the headings below the navigation level have the weight of their group',
          weights == {'bold'}, str(weights))


def check_levels(browser, base):
    context = browser.new_context(viewport={'width': 1280, 'height': 900})
    page = context.new_page()
    page.goto(base + 'levels.html')
    page.wait_for_load_state('load')
    page.wait_for_timeout(300)
    check_groups(page, 'four levels', navigation=2, levels={'0', '1', '2', '3'})
    # Folded, at the top of the page: the entries below a unit's entry (level 2) are
    # hidden, the entries at the levels 0 to 2 shown.
    page.evaluate("() => document.documentElement.classList.remove('ln-toc-all')")
    page.evaluate("() => window.scrollTo(0, 0)")
    page.wait_for_timeout(300)
    shown = page.evaluate("""() => [...document.querySelectorAll('.ln-sidebar li')].map((li) => [
        li.getAttribute('data-flm-level'), li.getClientRects().length > 0])""")
    check('four levels: folded, the entries below the units are hidden and the others shown',
          all(visible == (level != '3') for level, visible in shown), str(shown))
    # The setting `css`: one `<style>` element right after the link to the template's
    # stylesheet, whose rule of the group `top` sets the level 0 in capitals.
    after = page.evaluate("""() => {
        const link = document.querySelector('head link[rel="stylesheet"][href$="-style.css"]');
        const next = link && link.nextElementSibling;
        return next ? [next.tagName, next.textContent.includes('[data-flm-level="0"]')] : null; }""")
    check("four levels: the setting `css` is a `<style>` element after the template's stylesheet",
          after == ['STYLE', True], str(after))
    capitals = page.evaluate("""() => [
        ...[...document.querySelectorAll('#flm-content .flm-pres-block--heading')].map((h) => [
            h.getAttribute('data-flm-level'), getComputedStyle(h).textTransform]),
        ...[...document.querySelectorAll('.ln-sidebar li')].map((li) => [
            li.getAttribute('data-flm-level'), getComputedStyle(li.querySelector(':scope > a')).textTransform])]""")
    check('four levels: the setting `css` sets the headings and the entries of the group top in capitals, and only those',
          all((transform == 'uppercase') == (level == '0') for level, transform in capitals), str(capitals))
    context.close()


def check_plain(browser, base):
    context = browser.new_context(viewport={'width': 1280, 'height': 900})
    page = context.new_page()
    page.goto(base + 'plain.html')
    page.wait_for_load_state('load')
    page.wait_for_timeout(300)
    rule = page.evaluate("""() => { const b = document.querySelector('.ln-backmatter');
        const s = getComputedStyle(b); return [s.display, s.borderTopWidth, b.getBoundingClientRect().height]; }""")
    check('a document with no footnotes and no references shows no back-matter rule', rule[0] == 'none', str(rule))
    context.close()


def check_math(browser, base):
    for name, _ in MATH_SETUPS:
        context = browser.new_context(viewport={'width': 1280, 'height': 900})
        page = context.new_page()
        page.goto(base + f'notes-{name}.html')
        page.wait_for_load_state('load')
        page.wait_for_timeout(500)
        equation = hover_preview(page, '#eq-tp')
        parts = equation and equation['parts']
        check(f'{name}: hovering an equation reference shows the paragraph the formula stands in',
              parts is not None and parts[:1] == ['p.flm-pres-par'] and 'The channel' in equation['text'],
              str(parts))
        count = page.evaluate("(s) => document.querySelectorAll('div.ln-preview .ln-preview-content ' + s).length", FORMULA)
        check(f'{name}: the popup holds the formulas of the paragraph', count >= 2, f'{count} × {FORMULA}')
        context.close()


def check_lazy(browser, base):
    context = browser.new_context(viewport={'width': 1280, 'height': 900})
    page = context.new_page()
    page.goto(base + 'notes-lazy.html')
    page.wait_for_load_state('load')
    page.wait_for_timeout(1500)
    waiting = page.evaluate("""() => {
      const anchor = document.getElementById('eq-kl');
      return anchor === null ? null : anchor.closest('.flm-math-formula').querySelector('mjx-lazy') !== null;
    }""")
    check('lazy: an equation line has its anchor before its formula is typeset', waiting is True, str(waiting))
    # The first reference of the page, near its top, made a link to that line.
    page.evaluate("""() => document.querySelector('#flm-content a.flm-pres-link[data-flm-kind="ref"]')
      .setAttribute('href', '#eq-kl')""")
    hover_preview(page, '#eq-kl')
    page.wait_for_timeout(1500)
    counts = page.evaluate("""() => {
      const content = document.querySelector('div.ln-preview .ln-preview-content');
      return content && [content.querySelectorAll('mjx-container').length, content.querySelectorAll('mjx-lazy').length];
    }""")
    check('lazy: the popup typesets the formulas the page has not typeset yet',
          counts is not None and counts[0] >= 1 and counts[1] == 0, f'{counts} (typeset, waiting)')
    page.locator('#flm-content a[href="#eq-kl"]').first.click()
    page.wait_for_timeout(1000)
    where = page.evaluate("""() => {
      const anchor = document.getElementById('eq-kl');
      const top = anchor === null ? null : anchor.getBoundingClientRect().top;
      return [location.hash, top !== null && top >= 0 && top < window.innerHeight];
    }""")
    check('lazy: following a link to the line shows its formula', where == ['#eq-kl', True], str(where))
    context.close()


if __name__ == '__main__':
    if len(sys.argv) != 3:
        sys.exit('usage: check.py FLM_PROGRAM OUTPUT_FOLDER')
    FLM = str(pathlib.Path(sys.argv[1]).resolve())
    OUT = pathlib.Path(sys.argv[2]).resolve()
    render(FLM, OUT)
    server = serve(OUT)
    base = f'http://127.0.0.1:{server.server_address[1]}/'
    try:
        with sync_playwright() as playwright:
            browser = playwright.chromium.launch(executable_path=CHROME, args=['--headless=new'])
            check_sample(browser, base)
            check_plain(browser, base)
            check_levels(browser, base)
            check_math(browser, base)
            check_lazy(browser, base)
            browser.close()
    finally:
        server.shutdown()
        server.server_close()
    print(f'{len(failures)} failed' if failures else 'all checks passed')
    sys.exit(1 if failures else 0)
