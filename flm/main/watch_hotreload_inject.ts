//
// This code will be wrapped into a function(){...} body.  The
// following variables are defined in this context:
//
//     window, wsHost, wsPort
//

import morphdom from 'morphdom';

declare var wsHost: string;
declare var wsPort: number;

/*
type TupleElementRef = [
    id: string,
    sibling_offset: number,
    child_index: number | null
];
type ContentDOMUpdate =
    | {
        action: 'update-main';
        html: string
      }
    | {
        action: 'update-element';
        ref: TupleElementRef;
        html: string
      }
    | {
        action: 'insert-after-element';
        after_ref: TupleElementRef;
        html: string
      }
    | {
        action: 'delete-element';
        ref: TupleElementRef
      }
    ;
type UpdateInfo = {
    updates: ContentDOMUpdate[];
    content_html: string | null;
};
function resolveElementRef({
    container, ref
} : { container: HTMLElement, ref: TupleElementRef }) : HTMLElement|null
{
    const [id, sibling_offset, child_index] = ref;

    // Locate the anchor: the element with the given id, or the container
    // itself when id is null (root ref).
    var anchor =
        (id != null)
        ? (container.querySelector('#' + CSS.escape(id)) as HTMLElement)
        : container
        ;
    if (anchor == null) {
        return null;
    }

    if (sibling_offset === 0 && child_index == null) {
        // The anchor itself is the target.
        return anchor;
    }

    // Walk sibling_offset element-siblings forward from the anchor.
    // Text nodes, comment nodes, and whitespace-only nodes are skipped.
    let node: Node | null = anchor;
    for (let i = 0; i < sibling_offset; i++) {
        node = node.nextSibling;
        while (node != null && node.nodeType !== Node.ELEMENT_NODE) {
            node = node.nextSibling;
        }
        if (node == null) {
            return null;
        }
    }

    if (child_index == null) {
        // (anchor, sibling_offset, null): the sibling node is the target.
        return node as HTMLElement;
    }

    // (anchor, sibling_offset, child_index): count element children of node.
    let child = node.firstChild;
    let idx = 0;
    while (child != null) {
        if (child.nodeType === Node.ELEMENT_NODE) {
            if (idx === child_index) {
                return child as HTMLElement;
            }
            idx++;
        }
        child = child.nextSibling;
    }
    return null;
}
function updateInstructionsReceived(mainContainer: HTMLElement, info : UpdateInfo)
{
    for (const updInfo of info.updates) {
        console.log(`Applying content update: ${JSON.stringify(updInfo)}`);
        if (updInfo.action === 'update-main') {
            if (mainContainer == null || info.content_html == null) {
                // either no "Main" element or the server couldn't extract
                // the new body content ... need a full reload.
                //window.location.reload();
                throw new Error(`No main element!’`);
                return;
            } else {
                // replace "Main" content:
                mainContainer.innerHTML = info.content_html;
            }
        } else if (updInfo.action === 'update-element') {
            const target = resolveElementRef({
                container: mainContainer,
                ref: updInfo.ref,
            });
            if (target == null) {
                throw new Error(`Cannot resolve content element ref ‘${JSON.stringify(updInfo.ref)}’`);
            }
            const html = updInfo.html;
            target.outerHTML = html;
        } else if (updInfo.action === 'insert-after-element') {
            const afterTarget = resolveElementRef({
                container: mainContainer,
                ref: updInfo.after_ref,
            });
            if (afterTarget == null) {
                throw new Error(`Cannot resolve content element ref ‘${JSON.stringify(updInfo.after_ref)}’`);
            }
            const html = updInfo.html;
            // create DOM stuff from HTML fragment
            const t = document.createElement("template");
            t.innerHTML = html.trim();
            const newNode = t.content.firstChild as HTMLElement;
            afterTarget.after(newNode);
        } else if (updInfo.action === 'delete-element') {
            const delTarget = resolveElementRef({
                container: mainContainer,
                ref: updInfo.ref,
            });
            if (delTarget == null) {
                throw new Error(`Cannot resolve content element ref ‘${JSON.stringify(updInfo.ref)}’`);
            }
            delTarget.remove();
        } else {
            throw new Error(`Unknown/invalid action! ‘${JSON.stringify(updInfo)}’`);
        }
    }
    // see if there is any further setup to do (MathJaX,
    // build toc, etc.) -- depends on the template
    if ((window as any).flmSetup) {
        (window as any).flmSetup();
    }
}*/

type UpdateInfo = {
    action: 'update-main-content',
    content_html: string,
};

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

window.addEventListener("DOMContentLoaded", () => {

    const mainContainer = document.getElementById('Main')!;
    //
    // First of all, stamp all our equations with the source math data to help
    // us process updates.
    //
    stampMathContentSources(mainContainer);

    //
    // Set up the WebSocket to listen for updates.
    //
    const websocket = new WebSocket(`ws://${wsHost}:${wsPort}/`);
    websocket.addEventListener("message", function (m) {
        console.log("Message!", m);
        const info = JSON.parse(m.data) as UpdateInfo;
        try {
            updateMainContent(mainContainer, info);
        } catch (err) {
            // failure in the incremental update, so reload everything ... :/
            window.location.reload();
        }
    });
    websocket.addEventListener("open", function () { console.log("websocket open"); });
    websocket.addEventListener("close", function () { console.log("websocket closed"); });
    websocket.addEventListener("error", function (err) {
        console.log("websocket error", err);
    } );
    console.log("Started websocket and listening for update messages.");
});
