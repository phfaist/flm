
from ._base import FragmentRenderer


class TextFragmentRenderer(FragmentRenderer):

    display_href_urls = True

    float_separator_top = '·'*80
    float_separator_bottom = '·'*80


    #supports_delayed_render_markers = False # -- inherited already

    def render_value(self, value, render_context):
        return value

    def render_delayed_marker(self, node, delayed_key, render_context):
        return ''

    def render_delayed_dummy_placeholder(self, node, delayed_key, render_context):
        return '#DELAYED#'

    def render_nothing(self, render_context, annotations=None):
        return ''

    def render_empty_error_placeholder(self, debug_str, render_context):
        return ''

    def render_text_format(self, text_formats, nodelist, render_context):
        content = self.render_nodelist(
            nodelist,
            render_context,
            is_block_level=False,
        )
        return content
    
    def render_enumeration(self, iter_items_nodelists, counter_formatter, render_context,
                           *, target_id_generator=None, annotations=None, nested_depth=0):

        all_items = []
        for j, item_content_nodelist in enumerate(iter_items_nodelists):

            item_content = self.render_nodelist(
                item_content_nodelist,
                render_context=render_context,
                is_block_level=True,
            )

            tag_nodelist = counter_formatter(1+j)
            if tag_nodelist is None:
                tag_content = '?'
            elif isinstance(tag_nodelist, str):
                tag_content = self.render_value(tag_nodelist, render_context)
            else:
                tag_content = self.render_nodelist(
                    tag_nodelist,
                    render_context=render_context,
                    is_block_level=False,
                )
                
            if nested_depth > 0:
                tag_content = " "*(4*nested_depth) + tag_content

            all_items.append(
                (tag_content, item_content),
            )

        if not all_items:
            return self.render_semantic_block('', 'enumeration',
                                              render_context=render_context,
                                              annotations=annotations)

        max_item_width = max([ len(fmtcnt) for fmtcnt, item_content in all_items ])

        return self.render_join_blocks([
            self.render_semantic_block(
                self.render_join([
                    self.render_value(
                        fmtcnt.rjust(max_item_width+2, ' ') + ' ',
                        render_context,
                    ),
                    item_content,
                ], render_context),
                'enumeration',
                render_context=render_context,
                annotations=annotations,
            )
            for fmtcnt, item_content in all_items
        ], render_context)


    heading_level_formatter = {
        1: lambda s: f"{s}\n{'='*len(s)}",
        2: lambda s: f"{s}\n{'-'*len(s)}",
        3: lambda s: f"{s}\n{'~'*len(s)}",
        4: lambda s: f"{_add_punct(s, ':')}  ",
        5: lambda s: f"    {_add_punct(s, ':')}  ",
        6: lambda s: f"        {_add_punct(s, ':')}  ",

        # special 'theorem' level
        'theorem': lambda s: f"{s}.  ",
    }

    def render_heading(self, heading_nodelist, render_context, *,
                       heading_level=1, target_id=None, inline_heading=False,
                       annotations=None):

        rendered_heading = self.render_inline_content(heading_nodelist, render_context)

        if heading_level in self.heading_level_formatter:
            formatter = self.heading_level_formatter[heading_level]
            return formatter(rendered_heading)

        raise ValueError(f"Bad {heading_level=}, expected 1..6")


    def render_verbatim(self, value, render_context, *,
                        is_block_level=False, annotations=None, target_id=None):
        return value

    def render_link(self, ref_type, href, display_nodelist, render_context,
                    annotations=None):
        r"""
        .....

        `href` can be:

        - a URL (external link)
        
        - an anchor fragment only (`#fragment-name`), for links within the
          document; note that we use #fragment-name universally, even if the
          output format is not HTML.  It's up to the output format's render
          context features / fragment renderer subclass implementations to
          translate the linking scheme correctly.
        """

        display_content = self.render_nodelist(
            display_nodelist,
            render_context=render_context,
            is_block_level=False,
        )

        # never display local links (e.g. #footnote-X)
        if self.display_href_urls and not href.startswith("#"):
            return f"{display_content} <{href}>"
        return display_content


    def render_float(self, float_instance, render_context):

        full_figcaption_rendered_list = []
        if float_instance.number is not None:
            full_figcaption_rendered_list.append(
                self.render_join([
                    float_instance.float_type_info.float_caption_name,
                    ' ',
                    self.render_nodelist(float_instance.formatted_counter_value_flm.nodes,
                                         render_context=render_context),
                ], render_context)
            )
        elif float_instance.caption_nodelist:
            full_figcaption_rendered_list.append(
                float_instance.float_type_info.float_caption_name
            )
        else:
            pass
        
        if float_instance.caption_nodelist:
            full_figcaption_rendered_list.append(
                ": " # filler between the "Figure X" and the rest of the caption text.
            )
            full_figcaption_rendered_list.append(
                self.render_nodelist(
                    float_instance.caption_nodelist,
                    render_context=render_context
                )
            )

        rendered_float_caption = None
        if full_figcaption_rendered_list:
            rendered_float_caption = self.render_join(full_figcaption_rendered_list,
                                                      render_context)

        float_content_block = self.render_nodelist(
            float_instance.content_nodelist,
            render_context=render_context,
            is_block_level=True,
        )

        if rendered_float_caption is not None:
            float_content_with_caption = self.render_join_blocks([
                float_content_block,
                rendered_float_caption,
            ], render_context)
        else:
            float_content_with_caption = float_content_block

        return (
            self.float_separator_top
            + '\n' + float_content_with_caption + '\n'
            + self.float_separator_bottom
        )


    def render_graphics_block(self, graphics_resource, render_context):

        return f"{'['+graphics_resource.src_url+']':^80}"


    cells_column_sep = '   '

    def render_cells(self, cells_model, render_context, target_id=None):

        # render columns
        rendered_cells = []
        for cell in cells_model.cells_data:

            rendered_cell_contents = self.render_nodelist(
                cell.content_nodes,
                render_context=render_context,
            )

            rendered_cell_contents_lines = rendered_cell_contents.split('\n')

            is_header = False
            if 'H' in cell.styles:
                is_header = True

            rendered_cells.append( {
                'cell': cell,
                'rendered_contents_lines': rendered_cell_contents_lines,
                'width': max([
                    len(line) for line in rendered_cell_contents_lines
                ]),
                'is_header': is_header,
            } )

        # # compute column widths
        # col_widths = [ 0 for _ in range(len(cells_model.cells_size[1])) ]
        # for rcell in rendered_cells
        #     # requirement is that sum(col_widths(...cell...)) >= (content-width cell....)
        #     cell = rcell['cell']
        #     existing_total_column_widths = sum([
        #         col_widths[col_idx]
        #         for col_idx in range(cell.placement.col_range.start,
        #                              cell.placement.col_range.end)
        #     ])
        #     missing = rcell['width'] - existing_total_column_widths
        #     if missing < 0:
        #         # all good
        #         continue
        #     # otherwise, we need to increase the width (say of the last column
        #     # in the span).
        #     col_widths[cell.placement.col_range.end-1] += missing

        # # we can now render the table by preparing a text data model
        # text_data_model = [
        #     [
        #         # the lines in this cell
        #         []
        #         for _ in range(cells_model.cells_size[1])
        #     ]
        #     for _ in range(cells_model.cells_size[0])
        # ]
        # for rcell in rendered_cells:
        #     row, col = cell.placement.row_range.start, cell.placement.col_range.start
        #     text_data_model[row][col] += rcell['rendered_contents_lines']

        s_items = []

        # Very rudimentary text tables support, sorry ...
        for rcell in rendered_cells:
            s_items.append(
                '\n'.join([ f'    {line}' for line in rcell['rendered_contents_lines'] ])
            )

        return '\n'.join(s_items)




def _add_punct(x, c):
    x = str(x)
    if x.rstrip()[-1:] in '.,:;?!':
        return x
    return x + c



# ------------------------------------------------------------------------------

class FragmentRendererInformation:
    FragmentRendererClass = TextFragmentRenderer
    format_name = 'text'
