import html
import re

import logging
logger = logging.getLogger(__name__)

from ._base import FragmentRenderer


class HtmlFragmentRenderer(FragmentRenderer):

    supports_delayed_render_markers = True
    """
    We use the marker ``<LLM:DLYD:delayed_key/>`` for delayed content, which
    cannot be confused with the rest of the HTML code that can be generated from
    this code generator.
    """

    use_link_target_blank = False
    """
    Links will never open in a new tab.  Set to `True` on a specific instance to
    open links in a new tab (but this never applies to anchor links, i.e., urls
    that begin with '#').

    Set this attribute to a callable to decide whether or not to set
    `target="_blank"` on a per-URL basis.  The callable accepts a single
    argument, the URL given as a string, and returns True (open in new tab) or
    False (don't).
    """
    
    
    #fix_punctuation_line_wrapping = True  # TODO!
    """
    Enable a fix that prevents punctuation marks (e.g., period, comma, etc.)
    from appearing on a new line after content wrapped in a tag, such as a
    citation or a footnote mark.

    FIXME: NOT SURE HOW TO DO THIS!
    """


    heading_tags_by_level = {
        j: f"h{j}"
        for j in range(1,7) # i.e. [1, 2, 3, 4, 5, 6]
    }


    # ------------------

    

    # ------------------

    def htmlescape(self, value):
        return html.escape(value)

    def wrap_in_tag(self, tagname, content_html, *,
                    attrs=None, class_names=None):
        s = f'<{tagname}'
        if attrs:
            for aname, aval in dict(attrs).items():
                s += f''' {aname}="{self.htmlescape(aval)}"'''
        if class_names:
            s += f''' class="{self.htmlescape(' '.join(class_names))}"'''
        s += '>'
        s += str(content_html)
        s += f'</{tagname}>'
        return s

    def wrap_in_link(self, display_html, target_href, *, class_names=None):
        attrs = {
            'href': self.htmlescape(target_href)
        }
        if callable(self.use_link_target_blank):
            if self.use_link_target_blank(target_href):
                attrs['target'] = '_blank'
        elif self.use_link_target_blank and not target_href.startswith('#'):
            attrs['target'] = '_blank'
        return self.wrap_in_tag(
            'a',
            display_html,
            attrs=attrs,
            class_names=class_names,
        )

    # -----------------

    def render_build_paragraph(self, nodelist, render_context):
        return (
            "<p>"
            + self.render_inline_content(nodelist, render_context)
            + "</p>"
        )

    def render_inline_content(self, nodelist, render_context):
        return self.render_join(
            [ self.render_node(n, render_context) for n in nodelist ]
        )

    def render_join(self, content_list):
        r"""
        Join together a collection of pieces of content that have already been
        rendered.  Usually you'd want to simply join the strings together with
        no joiner, which is what the default implementation does.
        """
        return "".join([str(s) for s in content_list])

    html_blocks_joiner = "\n"

    def render_join_blocks(self, content_list):
        r"""
        Join together a collection of pieces of content that have already been
        rendered.  Each piece is itself a block of content, which can assumed to
        be at least paragraph-level or even semantic blocks.  Usually you'd want
        to simply join the strings together with no joiner, which is what the
        default implementation does.
        """
        return self.html_blocks_joiner.join(content_list)


    # ------------------

    def render_value(self, value):
        return self.htmlescape(value)

    def render_empty_error_placeholder(self):
        return "<span class=\"empty-error-placeholder\">(?)</span>"

    def render_nothing(self, annotations=None):
        if not annotations:
            annotations = []
        annotations = [a.replace('--', '- - ') for a in annotations]
        return '<!-- {} -->'.format(" ".join(annotations))

    def render_verbatim(self, value, *, annotations, target_id=None):
        attrs = {}
        if target_id is not None:
            attrs['id'] = target_id
        return self.wrap_in_tag(
            'span',
            self.htmlescape(value),
            class_names=(annotations if annotations else ['verbatim']),
            attrs=attrs,
        )

    def render_math_content(self,
                            delimiters,
                            nodelist,
                            render_context,
                            displaytype,
                            environmentname=None,
                            target_id=None):
        class_names = [ f"{displaytype}-math" ]
        if environmentname is not None:
            class_names.append(f"env-{environmentname.replace('*','-star')}")

        content_html = (
            self.htmlescape( delimiters[0] + nodelist.latex_verbatim() + delimiters[1] )
        )

        attrs = {}
        if target_id is not None:
            attrs['id'] = target_id

        if displaytype == 'display':
            # BlockLevelContent( # -- don't use blockcontent as display
            # equations might or might not be in their separate paragraph.
            return (
                self.wrap_in_tag(
                    'span',
                    content_html,
                    class_names=class_names,
                    attrs=attrs
                )
            )
        return self.wrap_in_tag(
            'span',
            content_html,
            class_names=class_names,
            attrs=attrs
        )

    def render_text_format(self, text_formats, nodelist, render_context):
        r"""
        """

        content = self.render_nodelist(
            nodelist,
            render_context,
            is_block_level=False
        )

        return self.wrap_in_tag(
            'span',
            content,
            class_names=text_formats
        )

    def render_semantic_block(self, content, role, *, annotations=None, target_id=None):
        if role in ('section', 'main', 'article', ): # todo, add
            return self.wrap_in_tag(
                role,
                content,
                class_names=annotations,
            )
        return self.wrap_in_tag(
            'div',
            content,
            class_names=[role]+(annotations if annotations else []),
        )
            
 
    def render_enumeration(self, iter_items_nodelists, counter_formatter, render_context,
                           *, target_id_generator=None, annotations=None):

        r"""
        ... remember, counter_formatter is given a number starting at 1.

        ... target_id_generator is a callable, takes one argument (item #
        starting at 1, like counter_formatter), and returns the anchor name to
        use for the enumeration item (in HTML, the value of the
        id=... attribute)
        """

        s_items = []

        for j, item_content_nodelist in enumerate(iter_items_nodelists):

            use_block_level = True
            if item_content_nodelist.parsing_state.is_block_level is False:
                # if the content is explicitly not in block mode, don't use
                # block mode.
                use_block_level = False

            logger.debug("render_enumeration: got %d-th item content nodelist = %r",
                         j, item_content_nodelist)
            logger.debug("will use_block_level = %r", use_block_level)

            item_content = self.render_nodelist(
                item_content_nodelist,
                render_context=render_context,
                is_block_level=use_block_level,
            )

            enumno = 1+j

            tag_nodelist = counter_formatter(enumno)
            if isinstance(tag_nodelist, str):
                tag_content = self.render_value(tag_nodelist)
            else:
                tag_content = self.render_nodelist(
                    tag_nodelist,
                    render_context=render_context,
                    is_block_level=False,
                )

            dtattrs = {}
            if target_id_generator is not None:
                dtattrs['id'] = target_id_generator(enumno)

            s_items.append(
                self.render_join([
                    self.wrap_in_tag(
                        'dt',
                        tag_content,
                        attrs=dtattrs,
                    ),
                    self.wrap_in_tag(
                        'dd',
                        item_content
                    ),
                ])
            )

        return self.wrap_in_tag(
            'dl',
            self.render_join(s_items),
            class_names=['enumeration'] + (annotations if annotations else []),
        )


    def render_heading(self, heading_nodelist, render_context, *,
                       heading_level=1, target_id=None, annotations=None):
        if heading_level not in self.heading_tags_by_level:
            raise ValueError(f"Bad {heading_level=}, expected one of "
                             f"{list(self.heading_tags_by_level.keys())}")
        return self.wrap_in_tag(
            self.heading_tags_by_level[heading_level],
            self.render_inline_content(heading_nodelist, render_context),
            class_names=(annotations if annotations else []),
        )

    def render_link(self, ref_type, href, display_nodelist, render_context, annotations=None):
        display_content = self.render_nodelist(
            display_nodelist,
            render_context=render_context,
            is_block_level=False,
        )
        return self.wrap_in_link(
            display_content,
            href,
            class_names=[ f"href-{ref_type}" ] + (annotations if annotations else [])
        )

    
    def render_delayed_marker(self, node, delayed_key, render_context):
        return f"<LLM:DLYD:{delayed_key}/>"

    def render_delayed_dummy_placeholder(self, node, delayed_key, render_context):
        return '<!-- delayed:{delayed_key} -->'

    def replace_delayed_markers_with_final_values(self, content, delayed_values):
        return _rx_delayed_markers.sub(
            lambda m: delayed_values[int(m.group('key'))],
            content
        )




_rx_delayed_markers = re.compile(r'\<LLM:DLYD:(?P<key>\d+)\s*\/\>')
