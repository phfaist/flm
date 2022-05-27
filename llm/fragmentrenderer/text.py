
from ._base import FragmentRenderer


class TextFragmentRenderer(FragmentRenderer):

    display_href_urls = True

    #supports_delayed_render_markers = False # -- inherited already

    def render_value(self, value):
        return value

    def render_delayed_marker(self, node, delayed_key, render_context):
        return ''

    def render_delayed_dummy_placeholder(self, node, delayed_key, render_context):
        return '#DELAYED#'

    def render_nothing(self, annotations=None):
        return ''

    def render_empty_error_placeholder(self, debug_str):
        return ''

    def render_text_format(self, text_formats, nodelist, render_context):
        content = self.render_nodelist(
            nodelist,
            render_context,
            is_block_level=False,
        )
        return content
    
    def render_enumeration(self, iter_items_nodelists, counter_formatter, render_context,
                           annotations=None):

        all_items = []
        for j, item_content_nodelist in enumerate(iter_items_nodelists):

            item_content = self.render_nodelist(
                item_content_nodelist,
                render_context=render_context,
                is_block_level=True,
            )

            tag_nodelist = counter_formatter(1+j)
            if isinstance(tag_nodelist, str):
                tag_content = self.render_value(tag_nodelist)
            else:
                tag_content = self.render_nodelist(
                    tag_nodelist,
                    render_context=render_context,
                    is_block_level=False,
                )

            all_items.append(
                (tag_content, item_content),
            )

        if not all_items:
            return self.render_semantic_block('', 'enumeration', annotations=annotations)

        max_item_width = max([ len(fmtcnt) for fmtcnt, item_content in all_items ])

        return self.render_join_blocks([
            self.render_semantic_block(
                self.render_join([
                    self.render_value(fmtcnt.rjust(max_item_width+2, ' ') + ' '),
                    item_content,
                ]),
                'enumeration',
                annotations=annotations,
            )
            for fmtcnt, item_content in all_items
        ])

    def render_verbatim(self, value, annotations=None):
        return value

    def render_link(self, ref_type, href, display_nodelist, render_context,
                    annotations=None):
        r"""
        .....

        `href` can be:

        - a URL (external link)
        
        - an anchor fragment only (`#fragment-name`), for links within the
          document; note that we use #fragment-name universally, even if the
          output format is not HTML.  It's up to the output format's
          DocumentContext implementation to translate the linking scheme
          correctly.
        """

        display_content = self.render_nodelist(
            display_nodelist,
            render_context=render_context,
            is_block_level=False,
        )

        if self.display_href_urls:
            return f"{display_content} <{href}>"
        return display_content

