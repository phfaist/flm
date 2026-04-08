//
// This code will be injected in the HTML page sent to the client.
//
// This is the TypeScript source and may be more verbose for easy
// development and for readability.  It will be compiled to optimized
// JS source via parcel. Run "yarn build" in this folder to do that.
// The optimized JS is then output to ./dist/watch_hotreload_inject.js.
//
// Any accompanying CSS style should be written in ./watch_hotreload_style.css.
// It is automatically imported here.

//
// This code will be wrapped into a function(){...} body.  The
// following variables are defined in this context:
//
//     window, wsHost, wsPort
//
const document = window.document;

import morphdom from 'morphdom';

import cssText from 'bundle-text:./watch_hotreload_style.css';

declare var wsHost: string;
declare var wsPort: number;

const COMPILING_STATES = ['compiling', 'idle'];
type CompilingState = 'compiling' | 'idle';

type UpdateInfo =
    | {
        action: 'update-main-content'|'error-display',
        content_html: string,
    }
    | {
        action: 'set-compiling-state',
        state: CompilingState,
    }

function updateMainContentIncremental(mainContainer : HTMLElement, html : string)
{
    //
    // use MORPHDOM to replace only the parts that have actually changed.
    //
    
    const maybeMathSpanType = (el: HTMLElement) => {
        if (el.tagName !== "SPAN") {
            return null;
        }
        if (el.classList.contains('display-math')) {
            return 'display-math';
        }
        if (el.classList.contains('inline-math')) {
            return 'inline-math';
        }
        return null;
    };

    // let mathToTypeset : HTMLElement[] = [];

    console.log('About to call morphdom():', {mainContainer, html});

    morphdom(mainContainer, '<article>'+html+'</article>', {

        childrenOnly: true,

        getNodeKey(node : Node) {
            // Only element nodes
            if (node.nodeType !== Node.ELEMENT_NODE) {
                return undefined;
            }
            const elNode = node as HTMLElement;

            // Prefer explicit id
            const id = elNode.id;
            if (id) {
                return `id:${id}`;
            }

            return undefined;

            // if (['MATH', 'MO', 'MI', 'MN', 'MSUP', 'MSUB'].includes(elNode.tagName.toUpperCase())
            //     || elNode.tagName.startsWith('MJX-')) {
            //     // will have to skip diffs on these.
            //     return null;
            // }

            // // if a math span, use the source text.
            // const mathType = maybeMathSpanType(elNode);
            // if (mathType != null) {
            //     const mathLatexSource =
            //         elNode.dataset.mathLatexSource ?? elNode.textContent.trim();
                
            //     const t = mathLatexSource.replace(/[^A-Za-z0-9-_\\()[]]/g, '').slice(0,100);
            //     const key = `${mathType}:${t}`;
            //     console.log('getNodeKey:', node, {key}, {mathLatexSource});
            //     return key;
            // }

            // Build a structural fingerprint for non-id elements
            // const tag = elNode.tagName;
            // const childCount = elNode.childElementCount;

            // Create a simple text sample for the node.
            // Use only the element's *own* direct text nodes, not descendants',
            // so that a wrapper div doesn't get keyed by deeply nested content.
            // let text = '';
            // for (const child of elNode.childNodes) {
            //     if (child.nodeType === Node.TEXT_NODE) {
            //         text += child.nodeValue;
            //     }
            // }
            // const textSample = text
            //     .trim()
            //     //.replace(/\s+/g, '-')
            //     .replace(/[^a-z0-9-]/g, '')
            //     .slice(0, 200);
            
            //const key = `el:${tag}:${childCount}`; //:${textSample}`;
            //console.log('getNodeKey:', node, key);
            //return key;
        },

        onBeforeElUpdated(fromEl : HTMLElement, toEl : HTMLElement) {
            //console.log(`onBeforeElUpdated()`, {fromEl, toEl});
            const toMathType = maybeMathSpanType(toEl);
            if (toMathType == null) {
                //console.log(`[new node is not math, normal diff requested]`);
                return true; // new node is not a math node, normal diffing
            }
            // New node is a math node.  Before anything else, make sure
            // we stamp the latex source for that node.
            stampMathContentSourcesElement(toEl);

            const fromMathType = maybeMathSpanType(fromEl);
            if (fromMathType != toMathType) {
                //console.log(`[different math type, normal diff requested]`, {fromMathType, toMathType});
                // mathToTypeset.push(toEl);
                return true; // different math types, let morphdom replace the node
            }

            // Both are math spans of same type — compare source
            const oldSrc = fromEl.dataset.mathLatexSource;
            const newSrc = toEl.dataset.mathLatexSource;
            //console.log({oldSrc, newSrc});

            if (oldSrc === newSrc) {
                //console.log(`[math unchanged, skip update!]`);
                // Math unchanged — keep the existing typeset node entirely
                return false;
            }

            // Math changed — substitute in the new node ourselves.
            // Then maybe queue for typesetting:
            // mathToTypeset.push(toEl);
            //console.log(`[math changed, forced manual update.]`);
            fromEl.parentNode!.replaceChild(toEl.cloneNode(true), fromEl);
            return false;
        },

        //onElUpdated(el : HTMLElement) {
            //console.log(`Element updated!`, el);
        //},

        //onNodeAdded(node : Node) {
            //console.log(`Node added!`, node);
            // if (node.nodeType === 1 && isMathSpan(node)) {
            //     mathToTypeset.push(node);
            // }
        //    return node;
        //},

        //onNodeDiscarded(node: Node) {
            // console.log(`Node discarded`, node);
        //}
    });

    console.log('morphdom finished :)');

    // This should be taken care by the templates' flmSetup() callback
    //
    // // Typeset all new/changed math
    // if (mathToTypeset.length > 0) {
    //     // After morphdom, toEl references may have been inserted into the
    //     // live DOM.  But morphdom may also have reused fromEl nodes.  So
    //     // we re-query to be safe.
    //     const elements = mathToTypeset
    //         .map(el => (el.isConnected ? el : null))
    //         .filter(Boolean);
    //     if (elements.length > 0) {
    //         const MathJax = (window as any).MathJax;
    //         if (MathJax) {
    //             MathJax.typesetClear(elements);
    //             MathJax.typesetPromise(elements);
    //         }
    //     }
    // }
}

function updateMainContent(mainContainer : HTMLElement, info : UpdateInfo)
{
    if (info.action !== 'update-main-content') {
        return;
    }
    if (info.content_html == null) {
        // The server couldn't extract the new body content ... need a full reload.
        throw new Error(`No main element!’`);
        return;
    } else {
        // replace "Main" content (simple version):
        //mainContainer.innerHTML = info.content_html;
        //
        // use MORPHDOM to replace only the parts that have actually changed.
        updateMainContentIncremental(mainContainer, info.content_html);
    }
    // see if there is any further setup to do (MathJaX,
    // build toc, etc.) -- depends on the template
    if ((window as any).flmSetup) {
        (window as any).flmSetup();
    }
}

function stampMathContentSourcesElement(spanEl : HTMLElement)
{
    const textContent = spanEl.textContent.trim();
    spanEl.dataset.mathLatexSource = textContent;
}
function stampMathContentSources(mainContainer: HTMLElement, elements: HTMLElement[]|null = null)
{
    // Stamp source onto each wrapper
    const els =
        (elements != null)
        ? elements
        : mainContainer.querySelectorAll('span.inline-math, span.display-math')
        ;
    for (const el1 of els) {
        const el = el1 as HTMLElement;
        if (el.children.length > 0) {
            console.warn(`Math element`, el, `is already typeset, cannot store source!`);
        }
        stampMathContentSourcesElement(el);
    }
}




type SourceLocation = [source_path: string, line: number|undefined, col: number|undefined];


class ErrorPanel
{
    private panelDiv: HTMLElement;
    private bodyDiv: HTMLElement;
    private badgeDiv: HTMLElement;

    constructor()
    {
        // panel container
        const panel = document.createElement('div');
        panel.setAttribute('id', 'ErrorOverlay');

        // header bar
        const header = document.createElement('div');
        header.className = 'error-panel-header';
        const title = document.createElement('span');
        title.className = 'error-panel-title';
        title.textContent = '\u{1F4A5} ERROR';
        const collapseBtn = document.createElement('button');
        collapseBtn.className = 'error-panel-collapse-btn';
        collapseBtn.textContent = '\u2039';
        collapseBtn.addEventListener('click', () => this.collapse());
        header.appendChild(collapseBtn);
        header.appendChild(title);
        panel.appendChild(header);

        // scrollable body
        const body = document.createElement('div');
        body.className = 'error-panel-body';
        panel.appendChild(body);

        document.body.appendChild(panel);
        this.panelDiv = panel;
        this.bodyDiv = body;

        // collapsed badge icon
        const badge = document.createElement('div');
        badge.setAttribute('id', 'ErrorCollapsedBadge');
        badge.textContent = '\u{1F4A5}';
        badge.title = 'Show error panel';
        badge.addEventListener('click', () => this.expand());
        document.body.appendChild(badge);
        this.badgeDiv = badge;
    }

    clear() : void
    {
        this.panelDiv.classList.remove('error-overlay-shown', 'error-overlay-collapsed');
        this.badgeDiv.classList.remove('badge-shown');
    }

    show(contentHtml: string) : void
    {
        this.panelDiv.classList.remove('error-overlay-collapsed');
        this.panelDiv.classList.add('error-overlay-shown');
        this.badgeDiv.classList.remove('badge-shown');
        this.bodyDiv.innerHTML = contentHtml;
    }

    collapse() : void
    {
        this.panelDiv.classList.add('error-overlay-collapsed');
        this.badgeDiv.classList.add('badge-shown');
    }

    expand() : void
    {
        this.panelDiv.classList.remove('error-overlay-collapsed');
        this.badgeDiv.classList.remove('badge-shown');
    }
}


class HotReloadClient
{
    private ws: WebSocket;
    private mainContainer: HTMLElement;
    private errorPanel: ErrorPanel;
    private compilingWidgetDiv: HTMLElement;

    constructor(url: string, mainContainer: HTMLElement)
    {
        this.mainContainer = mainContainer;
        this.ws = new WebSocket(url);
        this.ws.addEventListener("message", (m) => this._onMessage(m));
        this.ws.addEventListener("open",  () => console.log("websocket open"));
        this.ws.addEventListener("close", () => console.log("websocket closed"));
        this.ws.addEventListener("error", (err) => console.warn("websocket error", err));
        console.log("Started websocket and listening for update messages.");

        this.errorPanel = new ErrorPanel();

        const compilingWidgetDiv = document.createElement('div');
        compilingWidgetDiv.setAttribute('id', 'CompilingStateWidget');
        document.body.appendChild(compilingWidgetDiv);
        this.compilingWidgetDiv = compilingWidgetDiv;
    }

    private _onMessage(m: MessageEvent) : void
    {
        try {
            this._processMessage(m);
        } catch (err) {
            console.error('Error while processing server message!', err);
        }
    }
    private _processMessage(m: MessageEvent) : void
    {
        console.log("Message!", m);
        const info = JSON.parse(m.data) as UpdateInfo;
        if (info.action === 'update-main-content') {
            this.errorPanel.clear();
            this.setCompilingState('idle');
            try {
                updateMainContent(this.mainContainer, info);
            } catch (err) {
                // failure in the incremental update, so reload everything ... :/
                window.location.reload();
            }
        } else if (info.action === 'error-display') {
            this.errorPanel.show(info.content_html);
            this.setCompilingState('idle');
        } else if (info.action === 'set-compiling-state') {
            this.setCompilingState(info.state);
        } else {
            console.error("Invalid update info action!", info);
        }
    }

    setCompilingState(state : CompilingState)
    {
        this.compilingWidgetDiv.classList.remove(
            ...COMPILING_STATES.filter( s => s != state )
        );
        this.compilingWidgetDiv.classList.add(state);
    }

    sendCommand(action: string, params: Record<string, unknown> = {}) : void
    {
        if (this.ws.readyState !== WebSocket.OPEN) {
            console.warn("HotReloadClient.sendCommand: websocket not open, dropping message", action, params);
            return;
        }
        this.ws.send(JSON.stringify({ action, ...params }));
    }

    openEditor(source_path: string, line?: number, col?: number, preferEditor?: string) : void
    {
        const params: Record<string, unknown> = { source_path };
        if (line != null) params.line = line;
        if (col != null) params.col = col;
        if (preferEditor != null) params.prefer_editor = preferEditor;
        this.sendCommand('open-editor', params);
    }

    // Walk up the DOM from el. At each level: check the node itself, then scan
    // preceding siblings backwards (closest first) for data-sourcepath. Stop
    // before leaving mainContainer. Returns [source_path, line, col] or null.
    findElementSourceLocation(el: HTMLElement): SourceLocation | null {
        const tryParse = (raw: string | undefined, ctx: HTMLElement): SourceLocation | null => {
            if (raw == null) return null;
            try {
                const [source_path, line, col] = JSON.parse(raw);
                return [source_path, line, col];
            } catch (e) {
                console.error('findElementSourceLocation: invalid data-sourcepath JSON', raw, ctx, e);
                return null;
            }
        };
        let node: HTMLElement | null = el;
        while (node !== null && node !== this.mainContainer) {
            // 1. Check the node itself.
            const nodeLoc = tryParse(node.dataset.sourcepath, node);
            if (nodeLoc != null) {
                return nodeLoc;
            }
            // 2. Scan preceding siblings, closest first.
            let sibling = node.previousElementSibling as HTMLElement | null;
            while (sibling !== null) {
                const sibLoc = tryParse(sibling.dataset.sourcepath, sibling);
                if (sibLoc != null) {
                    return sibLoc;
                }
                sibling = sibling.previousElementSibling as HTMLElement | null;
            }
            // 3. Move up.
            node = node.parentElement as HTMLElement | null;
        }
        console.warn('findElementSourceLocation: no data-sourcepath found for element', el);
        return null;
    }

    onElementLocationOpenEditor(el: HTMLElement) : void {
        const result = this.findElementSourceLocation(el);
        if (result == null) {
            console.warn(`No location availble for element`, el);
            return;
        }
        const [source_path, line, col] = result;
        this.openEditor(source_path, line, col);
        // Flash the clicked element as visual confirmation.
        el.classList.remove('flm-source-flash'); // reset if already animating
        void el.offsetWidth;                     // force reflow to restart animation
        el.classList.add('flm-source-flash');
        el.addEventListener('animationend', () => el.classList.remove('flm-source-flash'), { once: true });
    }
}


function getFlmRefsData(): any | null
{
    const scriptEl = document.getElementById('FlmRefsData');
    if (scriptEl == null) {
        console.warn('copyElementFlmRefCode: no <script id="FlmRefsData"> tag found');
        return null;
    }
    try {
        return JSON.parse(scriptEl.textContent ?? '{}');
    } catch (e) {
        console.error('copyElementFlmRefCode: failed to parse FlmRefsData JSON', e);
        return null;
    }
}

function copyElementFlmRefCode(mainContainer : HTMLElement, el : HTMLElement)
{
    // Step 1: Walk up the DOM from el to find an ancestor with an id attribute
    let targetId: string | null = null;
    let node: HTMLElement | null = el;
    while (node !== null && node !== mainContainer) {
        if (node.id) {
            targetId = node.id;
            break;
        }
        node = node.parentElement as HTMLElement | null;
    }
    if (targetId == null) {
        console.warn('copyElementFlmRefCode: no element with id found for element', el);
        return;
    }
    const targetNode : HTMLElement = node!;

    // Step 2: Look up the id in the FlmRefsData JSON database via target_href
    const refsData = getFlmRefsData();
    //console.log('got refsData = ', refsData);
    if (refsData == null) {
        return;
    }
    const hrefTarget = '#' + targetId;
    const refEntry = refsData.targets.find(
        (entry: any) => (entry.ref_label != null) && (entry.target_href === hrefTarget)
    );
    if (refEntry == null) {
        console.log('copyElementFlmRefCode: no ref entry found for id', targetId);
        return;
    }
    const ref_type: string = refEntry.ref_type;
    const ref_label: string = refEntry.ref_label;

    console.log('copyElementFlmRefCode: found ref', { ref_type, ref_label, refEntry });

    // Decide appropriate FLM code to copy to generate this reference.
    let flmrefcode = null;
    if (ref_type === 'defterm') {
        flmrefcode = '\\term{' + ref_label + '}';
    } else {
        flmrefcode = `\\ref{${ref_type}:${ref_label}}`;
    }
    // Copy flmrefcode to clipboard
    navigator.clipboard.writeText(flmrefcode).then(() => {
        console.log('copyElementFlmRefCode: copied to clipboard:', flmrefcode);
    }).catch((e) => {
        console.error('copyElementFlmRefCode: clipboard write failed', e);
    });

    // Flash the element as visual confirmation
    targetNode.classList.remove('flm-copied-to-clipboard');
    void targetNode.offsetWidth; // force reflow to restart animation
    targetNode.classList.add('flm-copied-to-clipboard');
    targetNode.addEventListener('animationend', () => targetNode.classList.remove('flm-copied-to-clipboard'), { once: true });
}




window.addEventListener("DOMContentLoaded", () => {

    //
    // inject our style tag, mainly for the error overlay display
    //
    let style = document.createElement('style');
    style.textContent = cssText;
    document.head.appendChild(style);

    //
    // Our main container div
    //
    const mainContainer = document.getElementById('Main')!;

    //
    // First of all, stamp all our equations with the source math data to help
    // us process updates.
    //
    stampMathContentSources(mainContainer);

    //
    // Set up the WebSocket client.
    //
    const hotReloadClient = new HotReloadClient(`ws://${wsHost}:${wsPort}/`, mainContainer);

    // Expose public API for use by page templates
    (window as any).flmHotReload = hotReloadClient;

    //
    // CMD+SHIFT+click (Mac) or CTRL+SHIFT+click (other) opens the source
    // location of the clicked element in the editor.
    //
    mainContainer.addEventListener("mousedown", (e: MouseEvent) => {
        const modifierHeld = (e.metaKey || e.ctrlKey) && e.shiftKey;
        if (!modifierHeld) {
            return;
        }
        e.preventDefault();
        e.stopPropagation();
        hotReloadClient.onElementLocationOpenEditor(e.target as HTMLElement);
    });

    //
    // CMD+click (Mac) or CTRL+click (other) copies a reference to the pointed
    // object to the clipboard (and provides visual flash confirmation).
    //
    mainContainer.addEventListener("mousedown", (e: MouseEvent) => {
        const modifierHeld = (e.metaKey || e.ctrlKey) && !e.shiftKey;
        if (!modifierHeld) {
            return;
        }
        e.preventDefault();
        e.stopPropagation();
        copyElementFlmRefCode(mainContainer, e.target as HTMLElement);
    });
});
