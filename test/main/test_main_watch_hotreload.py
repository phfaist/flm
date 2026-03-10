import unittest

from lxml import html as lxml_html

from flm.main.watch_hotreload import diff_html


# ---------------------------------------------------------------------------
# Round-trip helpers
# ---------------------------------------------------------------------------

def _elem_children(el):
    return [c for c in el if isinstance(c.tag, str)]


def _resolve_ref_lxml(root, ref):
    """Resolve a (anchor_id, sibling_offset, child_index) ref in an lxml tree."""
    anchor_id, sibling_offset, child_index = ref
    if anchor_id is None:
        anchor = root
    else:
        matches = root.xpath(f'.//*[@id="{anchor_id}"]')
        if not matches:
            return None
        anchor = matches[0]
    if sibling_offset == 0 and child_index is None:
        return anchor
    parent = anchor.getparent()
    if parent is None:
        return None
    siblings = _elem_children(parent)
    try:
        pos = siblings.index(anchor)
    except ValueError:
        return None
    target_pos = pos + sibling_offset
    if not (0 <= target_pos < len(siblings)):
        return None
    node = siblings[target_pos]
    if child_index is None:
        return node
    kids = _elem_children(node)
    if not (0 <= child_index < len(kids)):
        return None
    return kids[child_index]


def _apply_updates(old_html, updates):
    """Apply diff_html updates to old_html and return the resulting innerHTML."""
    root = lxml_html.fragment_fromstring(old_html, create_parent='div')
    for u in updates:
        action = u['action']
        if action == 'update-main':
            new_root = lxml_html.fragment_fromstring(u['html'], create_parent='div')
            for child in list(root):
                root.remove(child)
            root.text = new_root.text
            for child in list(new_root):
                root.append(child)
        elif action == 'update-element':
            ref = u['ref']
            target = _resolve_ref_lxml(root, ref)
            new_el = lxml_html.fragment_fromstring(u['html'])
            parent = target.getparent()
            idx = list(parent).index(target)
            new_el.tail = target.tail
            parent.remove(target)
            parent.insert(idx, new_el)
        elif action == 'insert-after-element':
            after_ref = u['after_ref']
            new_el = lxml_html.fragment_fromstring(u['html'])
            if after_ref is None:
                root.insert(0, new_el)
            else:
                anchor = _resolve_ref_lxml(root, after_ref)
                parent = anchor.getparent()
                idx = list(parent).index(anchor)
                parent.insert(idx + 1, new_el)
        elif action == 'delete-element':
            target = _resolve_ref_lxml(root, u['ref'])
            target.getparent().remove(target)
    parts = [root.text or '']
    for child in root:
        parts.append(lxml_html.tostring(child, encoding='unicode'))
    return ''.join(parts)


def _normalize_html(html_str):
    """Parse and re-serialize, stripping inter-element whitespace."""
    root = lxml_html.fragment_fromstring(html_str, create_parent='div')
    for el in root.iter():
        if el.text:
            el.text = el.text.strip() or None
        if el.tail:
            el.tail = el.tail.strip() or None
    return lxml_html.tostring(root, encoding='unicode')


def _actions(result):
    return [u['action'] for u in result]

def _refs(result):
    """Return the set of (anchor_id, sibling_offset, child_index) tuples
    (or None for the root container) across all updates in result."""
    refs = set()
    for u in result:
        ref = u.get('ref')
        refs.add(tuple(ref) if ref is not None else None)
    return refs

def _anchor_ids(result):
    """Return the set of anchor ids (ref[0]) from all updates, or None for
    root-container updates (ref=None)."""
    ids = set()
    for u in result:
        ref = u.get('ref')
        ids.add(ref[0] if ref is not None else None)
    return ids

def _by_action(result, action):
    return [u for u in result if u['action'] == action]



class TestLongerDiffs(unittest.TestCase):

    #
    # @CLAUDE.AI: YOU ARE NOT ALLOWED TO CHANGE ANY OF THE TESTS IN THIS CLASS.
    # IF YOU SUSPECT A MISTAKE YOU MUST REPORT IT TO ME AND ASK FOR FURTHER COURSE
    # OF ACTION.
    #

    def test_1(self):
        h = '<div id="A"><p>One</p><p>Two</p><p>Three</p></div>'
        h2 = '<div id="A"><p>One</p><p>Two</p><p>THREE</p></div>'
        updates = diff_html(h, h)
        self.assertEqual(len(updates), 1)
        self.assertEqual(updates[0], {
            'action': 'update-element',
            'ref': [ 'A', 0, 2 ],
            'html': '<p>THREE</p>',
        });

    def test_2(self):
        h = '<p>One</p><p>Two</p><p>three</p>'
        h2 = '<p>One</p><p>Two</p><p>THREE</p>'
        updates = diff_html(h, h)
        self.assertEqual(len(updates), 1)
        self.assertEqual(updates[0], {
            'action': 'update-element',
            'ref': [ None, 0, 2 ],
            'html':
        });



class TestDiffHtmlNoChange(unittest.TestCase):

    def test_identical_returns_empty(self):
        h = '<div id="root"><p id="p1">Hello world</p></div>'
        self.assertEqual(diff_html(h, h), [])

    def test_whitespace_only_identical(self):
        h = '<div id="root">  <p id="p1">text</p>  </div>'
        self.assertEqual(diff_html(h, h), [])


class TestDiffHtmlTextChange(unittest.TestCase):

    def test_text_change_in_id_element(self):
        old = '<div id="root"><p id="p1">Hello world</p></div>'
        new = '<div id="root"><p id="p1">Hello there</p></div>'
        result = diff_html(old, new)
        self.assertEqual(len(result), 1)
        self.assertEqual(result[0]['action'], 'update-element-contents')
        # p#p1 is referenced as itself: (p1, offset=0, child_index=None)
        self.assertEqual(result[0]['ref'], ['p1', 0, None])
        self.assertTrue('Hello there' in result[0]['html'])

    def test_text_change_in_anonymous_child_addressed_directly(self):
        # The <span> has no id but is the 0th element child of p#p1 — it can
        # be addressed directly as (p1, sibling_offset=0, child_index=0).
        old = '<div id="root"><p id="p1"><span>old text</span></p></div>'
        new = '<div id="root"><p id="p1"><span>new text</span></p></div>'
        result = diff_html(old, new)
        self.assertEqual(len(result), 1)
        self.assertEqual(result[0]['action'], 'update-element-contents')
        self.assertEqual(result[0]['ref'], ['p1', 0, 0])

    def test_change_bubbles_to_container_reports_none(self):
        # <p> has no id and no id'd ancestor → root container → ref=None
        old = '<p>Hello world</p>'
        new = '<p>Hello there</p>'
        result = diff_html(old, new)
        self.assertEqual(len(result), 1)
        self.assertEqual(result[0]['action'], 'update-element-contents')
        self.assertIsNone(result[0]['ref'])

    def test_html_output_reflects_new_content(self):
        old = '<div id="sec1"><p id="p1">Before</p></div>'
        new = '<div id="sec1"><p id="p1">After</p></div>'
        result = diff_html(old, new)
        self.assertEqual(len(result), 1)
        self.assertTrue('After' in result[0]['html'])
        self.assertFalse('Before' in result[0]['html'])


class TestDiffHtmlMultipleChanges(unittest.TestCase):

    def test_two_independent_sections_changed(self):
        old = (
            '<section id="s1"><p>Alpha</p></section>'
            '<section id="s2"><p>Beta</p></section>'
        )
        new = (
            '<section id="s1"><p>Alpha modified</p></section>'
            '<section id="s2"><p>Beta modified</p></section>'
        )
        result = diff_html(old, new)
        ids = _anchor_ids(result)
        self.assertTrue('s1' in ids)
        self.assertTrue('s2' in ids)

    def test_only_one_section_changed(self):
        old = (
            '<section id="s1"><p>Alpha</p></section>'
            '<section id="s2"><p>Beta</p></section>'
        )
        new = (
            '<section id="s1"><p>Alpha modified</p></section>'
            '<section id="s2"><p>Beta</p></section>'
        )
        result = diff_html(old, new)
        ids = _anchor_ids(result)
        self.assertTrue('s1' in ids)
        self.assertFalse('s2' in ids)

    def test_container_update_eclipses_finer_updates(self):
        # When a change can't be attributed to any element (anonymous element
        # before all id'd siblings), a root container update (ref=None) is
        # generated.  Any finer-grained updates within the same container are
        # eclipsed and must NOT appear alongside it.
        old = '<p>changed</p><p id="p1">also changed</p>'
        new = '<p>new</p><p id="p1">new p1</p>'
        result = diff_html(old, new)
        # Only the root container update should survive
        self.assertEqual(len(result), 1)
        self.assertEqual(result[0]['action'], 'update-element-contents')
        self.assertIsNone(result[0]['ref'])

    def test_container_update_eclipses_within_id_parent(self):
        # Same eclipse rule when the container has an id.  An anonymous element
        # before all id'd siblings forces a container update for #outer; the
        # finer-grained update for #inner must be dropped.
        old = '<div id="outer">text <span>old</span><p id="inner">old</p></div>'
        new = '<div id="outer">text <span>new</span><p id="inner">new</p></div>'
        result = diff_html(old, new)
        # Only the #outer container update should survive
        self.assertEqual(len(result), 1)
        self.assertEqual(result[0]['action'], 'update-element-contents')
        self.assertEqual(result[0]['ref'], ['outer', 0, None])

    def test_independent_changes_same_parent_both_reported(self):
        # Two sibling changes inside #outer: one to #inner (has id), one to
        # an anonymous <p> (no id, but reachable as sibling_offset=1 after #inner).
        # Both are reported independently — no need to fall back to #outer.
        old = '<div id="outer"><p id="inner">old</p><p>other old</p></div>'
        new = '<div id="outer"><p id="inner">new</p><p>other new</p></div>'
        result = diff_html(old, new)
        refs = _refs(result)
        # p#inner referenced as itself
        self.assertTrue(('inner', 0, None) in refs)
        # anonymous <p> referenced as 1st element sibling after #inner
        self.assertTrue(('inner', 1, None) in refs)


class TestDiffHtmlStructuralChanges(unittest.TestCase):

    def test_element_added(self):
        # A new id'd sibling is appended; reported as insert-after-element.
        old = '<p id="p1">First</p>'
        new = '<p id="p1">First</p><p id="p2">Second</p>'
        result = diff_html(old, new)
        inserts = _by_action(result, 'insert-after-element')
        self.assertEqual(len(inserts), 1)
        # after_ref points to p1 itself (offset 0 from anchor 'p1') → ['p1', 0, None]
        self.assertEqual(inserts[0]['after_ref'], ['p1', 0, None])
        self.assertTrue('Second' in inserts[0]['html'])

    def test_element_removed(self):
        # An id'd element is removed; ref is ['p2', 0, None] (the element itself).
        old = '<p id="p1">First</p><p id="p2">Second</p>'
        new = '<p id="p1">First</p>'
        result = diff_html(old, new)
        deletes = _by_action(result, 'delete-element')
        self.assertEqual(len(deletes), 1)
        self.assertEqual(deletes[0]['ref'], ['p2', 0, None])

    def test_attribute_change_reports_element_itself(self):
        # An attribute on an id'd element changes.  The element's own
        # outerHTML changed, so it is reported directly (not its parent).
        old = '<div id="root"><a id="link1" href="http://a.com">click</a></div>'
        new = '<div id="root"><a id="link1" href="http://b.com">click</a></div>'
        result = diff_html(old, new)
        updates = _by_action(result, 'update-element-contents')
        refs = {tuple(u['ref']) for u in updates}
        self.assertTrue(('link1', 0, None) in refs)
        link1 = next(u for u in updates if u['ref'] == ['link1', 0, None])
        self.assertTrue('http://b.com' in link1['html'])


class TestDiffHtmlReturnFields(unittest.TestCase):

    def test_update_has_required_keys(self):
        old = '<div id="root"><p id="p1">old</p></div>'
        new = '<div id="root"><p id="p1">new</p></div>'
        result = diff_html(old, new)
        self.assertEqual(len(result), 1)
        entry = result[0]
        self.assertTrue('action' in entry)
        self.assertTrue('ref' in entry)
        self.assertTrue('html' in entry)

    def test_html_field_is_string(self):
        old = '<div id="root"><p id="p1">old</p></div>'
        new = '<div id="root"><p id="p1">new</p></div>'
        result = diff_html(old, new)
        self.assertTrue(isinstance(result[0]['html'], str))

    def test_html_field_contains_element_tag(self):
        old = '<div id="root"><p id="p1">old text</p></div>'
        new = '<div id="root"><p id="p1">new text</p></div>'
        result = diff_html(old, new)
        self.assertTrue(result[0]['html'].startswith('<p'))

    def test_insert_has_required_keys(self):
        old = '<p id="p1">First</p>'
        new = '<p id="p1">First</p><p id="p2">Second</p>'
        result = diff_html(old, new)
        inserts = _by_action(result, 'insert-after-element')
        self.assertEqual(len(inserts), 1)
        entry = inserts[0]
        self.assertTrue('action' in entry)
        self.assertTrue('after_ref' in entry)
        self.assertTrue('html' in entry)

    def test_delete_has_required_keys(self):
        old = '<p id="p1">First</p><p id="p2">Second</p>'
        new = '<p id="p1">First</p>'
        result = diff_html(old, new)
        deletes = _by_action(result, 'delete-element')
        self.assertEqual(len(deletes), 1)
        entry = deletes[0]
        self.assertTrue('action' in entry)
        self.assertTrue('ref' in entry)


class TestDiffHtmlPositionalRef(unittest.TestCase):

    def test_insert_after_anonymous_sibling_uses_anchor_ref(self):
        # New <p> inserted after an anonymous <p> that follows an id'd heading.
        # after_ref should anchor to the id'd heading and count forward.
        old = '<h2 id="sec">Title</h2><p>Para one</p>'
        new = '<h2 id="sec">Title</h2><p>Para one</p><p>Para two</p>'
        result = diff_html(old, new)
        inserts = _by_action(result, 'insert-after-element')
        self.assertEqual(len(inserts), 1)
        # Preceding sibling is the anonymous <p> at sibling_offset=1 after #sec
        self.assertEqual(inserts[0]['after_ref'], ['sec', 1, None])
        self.assertTrue('Para two' in inserts[0]['html'])

    def test_delete_anonymous_element_uses_positional_ref(self):
        # Anonymous <p> is removed; it was 1 sibling after #sec.
        old = '<h2 id="sec">Title</h2><p>Para one</p>'
        new = '<h2 id="sec">Title</h2>'
        result = diff_html(old, new)
        deletes = _by_action(result, 'delete-element')
        self.assertEqual(len(deletes), 1)
        self.assertEqual(deletes[0]['ref'], ['sec', 1, None])

    def test_ref_uses_child_index_when_no_id_siblings(self):
        # Element inside an id'd parent with no id'd siblings: ref uses
        # [parent_id, 0, child_index].
        old = '<div id="box"><p>A</p><p>B</p></div>'
        new = '<div id="box"><p>A</p></div>'
        result = diff_html(old, new)
        deletes = _by_action(result, 'delete-element')
        self.assertEqual(len(deletes), 1)
        # <p>B</p> is the 2nd element child (index 1) of #box, no id'd preceding sibling
        self.assertEqual(deletes[0]['ref'], ['box', 0, 1])

    def test_nonwhitespace_tail_text_prevents_any_positional_ref(self):
        # Non-whitespace text between elements means neither sibling_offset
        # nor child_index can be resolved reliably (the text is in h2.tail,
        # which is between anchor #sec and the target <p>).
        # Falls back to a container update for the parent element.
        old = '<div id="box"><h2 id="sec">Title</h2> some text <p>Para</p></div>'
        new = '<div id="box"><h2 id="sec">Title</h2> some text </div>'
        result = diff_html(old, new)
        deletes = _by_action(result, 'delete-element')
        updates = _by_action(result, 'update-element-contents')
        # No delete-element because no reliable ref can be formed
        self.assertEqual(len(deletes), 0)
        # Container update for #box instead: ref = ['box', 0, None]
        self.assertEqual(len(updates), 1)
        self.assertEqual(updates[0]['ref'], ['box', 0, None])

    def test_whitespace_only_tail_does_not_block_sibling_offset_ref(self):
        # Whitespace-only tail text between siblings is fine; sibling_offset
        # ref should still be generated.
        old = '<h2 id="sec">Title</h2>\n<p>Para one</p>'
        new = '<h2 id="sec">Title</h2>\n<p>Para one</p><p>Para two</p>'
        result = diff_html(old, new)
        inserts = _by_action(result, 'insert-after-element')
        self.assertEqual(len(inserts), 1)
        # Preceding sibling is the anonymous <p> at sibling_offset=1 after #sec
        # (whitespace tail on #sec does not block)
        self.assertEqual(inserts[0]['after_ref'], ['sec', 1, None])

    def test_comment_between_siblings_does_not_block_sibling_offset_ref(self):
        # HTML comments between element siblings are fine; sibling_offset
        # counts only HTML elements (comments are skipped by the client).
        # div.b is the 3rd element sibling after #a: div.one(1), div.two(2), div.b(3).
        old = (
            "<p id='a'>bla</p><div>one</div>      <div>two</div>"
            "<!-- ignore me --><div class='b'>three</div>"
        )
        new = (
            "<p id='a'>bla</p><div>one</div>      <div>two</div>"
            "<!-- ignore me --><div class='b'>changed</div>"
        )
        result = diff_html(old, new)
        updates = _by_action(result, 'update-element-contents')
        self.assertEqual(len(updates), 1)
        # div.b has no id; referenced as 3rd element sibling after #a
        self.assertEqual(updates[0]['ref'], ['a', 3, None])
        self.assertTrue('changed' in updates[0]['html'])


class TestDiffHtmlReorder(unittest.TestCase):

    def test_id_children_swapped_delete_and_reinsert(self):
        # Two id'd siblings swap positions.  The moved element is reported as
        # delete-element (from old position) + insert-after-element (at new position).
        old = (
            '<section id="s1"><p>Alpha</p></section>'
            '<section id="s2"><p>Beta</p></section>'
        )
        new = (
            '<section id="s2"><p>Beta</p></section>'
            '<section id="s1"><p>Alpha</p></section>'
        )
        result = diff_html(old, new)
        actions = set(_actions(result))
        # The reorder must produce at least a delete or an insert
        self.assertTrue('delete-element' in actions or 'insert-after-element' in actions)
        # s1 is the element that was moved (deleted from position 0, reinserted after s2)
        deletes = _by_action(result, 'delete-element')
        inserts = _by_action(result, 'insert-after-element')
        self.assertTrue(len(deletes) > 0 or len(inserts) > 0)

    def test_id_children_swapped_reinsert_after_correct_sibling(self):
        # After the swap, s1 is re-inserted after s2 → after_ref = ['s2', 0, None]
        # (s2 is the preceding element, referenced as the anchor itself at offset 0).
        old = (
            '<section id="s1"><p>Alpha</p></section>'
            '<section id="s2"><p>Beta</p></section>'
        )
        new = (
            '<section id="s2"><p>Beta</p></section>'
            '<section id="s1"><p>Alpha</p></section>'
        )
        result = diff_html(old, new)
        inserts = _by_action(result, 'insert-after-element')
        self.assertEqual(len(inserts), 1)
        self.assertEqual(inserts[0]['after_ref'], ['s2', 0, None])
        self.assertTrue('Alpha' in inserts[0]['html'])

    def test_no_id_children_reordered_reports_none_ref(self):
        # Top-level children without ids reordered; no id'd parent → ref=None.
        old = '<p>First</p><p>Second</p>'
        new = '<p>Second</p><p>First</p>'
        result = diff_html(old, new)
        refs = _refs(result)
        self.assertTrue(None in refs)

    def test_no_id_children_reordered_html_reflects_new_content(self):
        old = '<p>First</p><p>Second</p>'
        new = '<p>Second</p><p>First</p>'
        result = diff_html(old, new)
        none_entries = [r for r in result if r.get('ref') is None]
        self.assertEqual(len(none_entries), 1)
        container_html = none_entries[0]['html']
        self.assertTrue('Second' in container_html)
        self.assertTrue('First' in container_html)

    def test_three_children_rotated(self):
        # Three id'd top-level children rotated: s3 moved to front.
        # Results in a delete-element + insert-after-element for the moved element.
        old = (
            '<section id="s1"><p>A</p></section>'
            '<section id="s2"><p>B</p></section>'
            '<section id="s3"><p>C</p></section>'
        )
        new = (
            '<section id="s3"><p>C</p></section>'
            '<section id="s1"><p>A</p></section>'
            '<section id="s2"><p>B</p></section>'
        )
        result = diff_html(old, new)
        actions = set(_actions(result))
        self.assertTrue('delete-element' in actions or 'insert-after-element' in actions)


class TestDiffHtmlOrdering(unittest.TestCase):

    def test_deletes_come_before_inserts_and_updates(self):
        # The result must have all deletions first, then insertions, then updates.
        old = (
            '<section id="s1"><p>Alpha</p></section>'
            '<section id="s2"><p>Beta</p></section>'
        )
        new = (
            '<section id="s2"><p>Beta modified</p></section>'
            '<section id="s1"><p>Alpha</p></section>'
        )
        result = diff_html(old, new)
        actions = _actions(result)
        seen_insert = False
        seen_update = False
        for a in actions:
            if a == 'insert-after-element':
                seen_insert = True
            if a == 'update-element-contents':
                seen_update = True
            if a == 'delete-element':
                self.assertFalse(seen_insert, "delete after insert")
                self.assertFalse(seen_update, "delete after update")

    def test_multiple_deletes_are_back_to_front(self):
        # Removing two anonymous siblings: the one with higher sibling_offset
        # must appear first so the earlier one's position is not shifted.
        old = '<h2 id="sec">T</h2><p>A</p><p>B</p>'
        new  = '<h2 id="sec">T</h2>'
        result = diff_html(old, new)
        deletes = _by_action(result, 'delete-element')
        self.assertEqual(len(deletes), 2)
        offsets = [d['ref'][1] for d in deletes]
        self.assertEqual(offsets, sorted(offsets, reverse=True))

    def test_multiple_inserts_are_front_to_back(self):
        # Inserting two new siblings: the one with lower after_ref offset must
        # appear first so the second one can anchor to the first.
        old = '<h2 id="sec">T</h2>'
        new  = '<h2 id="sec">T</h2><p>A</p><p>B</p>'
        result = diff_html(old, new)
        inserts = _by_action(result, 'insert-after-element')
        self.assertEqual(len(inserts), 2)
        offsets = [i['after_ref'][1] if i['after_ref'] else -1 for i in inserts]
        self.assertEqual(offsets, sorted(offsets))

    def test_updates_come_after_structural_changes(self):
        # A content update must follow any deletions and insertions so that
        # its new-tree positional ref resolves against the final DOM shape.
        old = '<h2 id="sec">T</h2><p>old</p>'
        new  = '<h2 id="sec">T</h2><span>new-sib</span><p>new</p>'
        result = diff_html(old, new)
        actions = _actions(result)
        last_structural = max(
            (i for i, a in enumerate(actions) if a != 'update-element-contents'),
            default=-1
        )
        first_update = next(
            (i for i, a in enumerate(actions) if a == 'update-element-contents'),
            len(actions)
        )
        self.assertTrue(first_update >= last_structural)


if __name__ == '__main__':
    unittest.main()
