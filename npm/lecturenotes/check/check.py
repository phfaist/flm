# The browser smoke check of the lecture-notes page: renders the sample document
# (`flm-cli/tests/documents/lecturenotes/notes.flm`) and the short document
# `plain.flm` beside this file with the extension (`extensions/data/lecturenotes/`,
# the bundle `dist/` of its compose template as it stands), serves them from a local HTTP
# server, opens them in Google Chrome with no window, and asserts through the
# page's own DOM:
#
#   - the sidebar's entries carry their levels, and the chapters are marked;
#   - the current entry of the sidebar follows a scroll;
#   - the sidebar button hides the sidebar and shows it again;
#   - hovering a term, a theorem, an equation and the `[3–5]` mark opens the
#     popup with the expected content; a long press does the same on a touch
#     screen;
#   - a document with no footnotes and no references shows no back-matter rule.
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


def render(flm, out):
    if out.exists():
        shutil.rmtree(out)
    out.mkdir(parents=True)
    for name in ['notes.flm', 'cycle.svg']:
        shutil.copy(ROOT / 'flm-cli/tests/documents/lecturenotes' / name, out / name)
    shutil.copy(HERE / 'plain.flm', out / 'plain.flm')
    for document in ['notes', 'plain']:
        subprocess.run(
            [flm, 'render', '--extensions-dir', str(ROOT / 'extensions/data'), '-o', f'{document}.html',
             f'{document}.flm'],
            cwd=out, check=True, env={**os.environ, 'HOME': str(out)})


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

    # The sidebar's entries and their levels.
    levels = page.evaluate("""() => [...document.querySelectorAll('.ln-sidebar li')].map(
        (li) => [li.getAttribute('data-ln-level'), li.classList.contains('ln-chapter')])""")
    check('the sidebar has entries', len(levels) > 5, f'{len(levels)} entries')
    check('every sidebar entry carries its level', all(level is not None for level, _ in levels))
    check('the levels 0, 1 and 2 are present', {'0', '1', '2'} <= {level for level, _ in levels},
          str(sorted({level for level, _ in levels})))
    check('the chapters are the entries of level 1', all((level == '1') == chapter for level, chapter in levels))
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
    chapter = page.evaluate("() => document.querySelector('.ln-current-chapter').textContent.trim()")
    check('the bar shows the chapter being read', len(chapter) > 0, chapter)
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
    check('hovering an equation reference shows the paragraph before, the formula and the paragraph after',
          parts is not None and len(parts) == 3 and parts[0].startswith('p.') and parts[2].startswith('p.'),
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
            browser.close()
    finally:
        server.shutdown()
        server.server_close()
    print(f'{len(failures)} failed' if failures else 'all checks passed')
    sys.exit(1 if failures else 0)
