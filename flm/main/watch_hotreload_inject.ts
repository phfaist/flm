//
// This code will be wrapped into a function(){...} body.  The
// following variables are defined in this context:
//
//     window, wsHost, wsPort
//

declare var wsHost: string;
declare var wsPort: number;

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
        action: 'update-element-contents';
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
        } else if (updInfo.action === 'update-element-contents') {
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
}

window.addEventListener("DOMContentLoaded", () => {
    const websocket = new WebSocket(`ws://${wsHost}:${wsPort}/`);
    websocket.addEventListener("message", function (m) {
        console.log("Message!", m);
        const mainContainer = document.getElementById('Main')!;
        const info = JSON.parse(m.data) as UpdateInfo;
        try {
            updateInstructionsReceived(mainContainer, info);
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