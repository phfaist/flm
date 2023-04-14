import re

import logging
logger = logging.getLogger(__name__)


from flm.fragmentrenderer import FragmentRenderer
from flm.fragmentrenderer.html import HtmlFragmentRenderer


rx_mdspecials = re.compile(r'[\\`*_~{}\[\]<>()#+.!|-]')



class MarkdownFragmentRenderer(FragmentRenderer):

    supports_delayed_render_markers = True
    """
    We use the marker ``<FLM:DLYD:delayed_key/>`` for delayed content, which
    cannot be confused with the rest of the HTML code that can be generated from
    this code generator.
    """
   
    use_target_ids = 'anchor'
    """
    Determine how target_id's are set (if they are set).  One of 'anchor'
    (``<a name="TARGET_ID"></a>``), 'pandoc' (``[]{#TARGET_ID}``), 'github'
    (``[](#TARGET_ID)``) or `None` (don't set any target ids).
    """


    # ------------------

    def render_build_paragraph(self, nodelist, render_context):

        content = self.render_inline_content(nodelist, render_context)

        md_indent = render_context.get_logical_state('flm_markdown').get('md_indent', '')
        if md_indent != '':
            content = md_indent + content.replace('\n', '\n'+md_indent)
        return '\n\n' + content

    def render_inline_content(self, nodelist, render_context):
        result = self.render_join(
            [ self.render_node(n, render_context) for n in nodelist ],
            render_context
        )
        logger.debug('render_inline_content -> %r', result)
        return result

    def render_join(self, content_list, render_context):
        r"""
        Join together a collection of pieces of content that have already been
        rendered.  Usually you'd want to simply join the strings together with
        no joiner, which is what the default implementation does.
        """
        result = "".join([str(s) for s in content_list])
        logger.debug('***** JOIN %r -> %r', content_list, result)
        return result

    def render_join_blocks(self, content_list, render_context):
        r"""
        Join together a collection of pieces of content that have already been
        rendered.  Each piece is itself a block of content, which can assumed to
        be at least paragraph-level or even semantic blocks.  Usually you'd want
        to simply join the strings together with no joiner, which is what the
        default implementation does.
        """
        content = "\n\n".join(
            [c for c in content_list if c is not None and len(c)]
        )
        logger.debug('***** JOIN BLOCKS %r -> %r', content_list, content)
        return re.sub(r'\n{2,}', '\n\n', content).strip()


    # ------------------

    def render_value(self, value, render_context):
        return rx_mdspecials.sub(lambda m: '\\'+m.group(), value)

    def render_empty_error_placeholder(self, debug_str, render_context):
        return ""

    def render_nothing(self, render_context, annotations=None):
        return ""

    def render_verbatim(self, value, render_context, *,
                        is_block_level=False, annotations=None, target_id=None):
        value = value.replace('``', '` ` ')
        return (
            self._get_target_id_md_code(target_id) + 
            '`` ' + self.render_value(value, render_context) + ' ``'
        )

    def render_math_content(self,
                            delimiters,
                            nodelist,
                            render_context,
                            displaytype,
                            environmentname=None,
                            target_id=None):

        content = delimiters[0] + nodelist.latex_verbatim() + delimiters[1]
        content = self.render_value( content, render_context )

        content = self._get_target_id_md_code(target_id) + content

        # Don't escape '\begin{align} ...' into '\\begin\{align\} ... ' etc.
        return content

    def render_text_format(self, text_formats, nodelist, render_context):

        content = self.render_nodelist(
            nodelist,
            render_context,
            is_block_level=False
        )

        mdtext = content
        if 'textbf' in text_formats:
            mdtext = '**' + content + '**'
        if 'textit' in text_formats or 'emph' in text_formats:
            mdtext = '*' + content + '*'

        return mdtext

    def render_semantic_block(self, content, role, render_context, *,
                              annotations=None, target_id=None):
        # add newline because content might need to be at the beginning of the
        # line (e.g., # Section Heading)
        return self._get_target_id_md_code(target_id).rstrip() + '\n' + content
    
    def render_enumeration(self, iter_items_nodelists, counter_formatter, render_context,
                           *, target_id_generator=None, annotations=None, nested_depth=None):

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
                tag_content = self.render_value(tag_nodelist, render_context)
            else:
                tag_content = self.render_nodelist(
                    tag_nodelist,
                    render_context=render_context,
                    is_block_level=False,
                )

            tgtid_md_code = ''
            if target_id_generator is not None:
                target_id = target_id_generator(enumno)
                tgtid_md_code = self._get_target_id_md_code(target_id)

            # It doesn't seem that Markdown has good support for customizing the
            # list itemize/enumeration tag, so add it to the list item content...
            s_items.append(tag_content + ' ' + tgtid_md_code + item_content)
        
        logger.debug('rendering list: s_items = %r', s_items)

        mdindent_cur = render_context.get_logical_state('flm_markdown').get('md_indent', '')
        mdindent = mdindent_cur + '  '
        with render_context.push_logical_state('flm_markdown', 'indent', mdindent):

            mdtexts = []
            
            for s_item in s_items:
                mdtexts.append(
                    mdindent_cur + '- ' + s_item.replace('\n', '\n'+mdindent)
                )

        content = self.render_join_blocks(mdtexts, render_context)

        return content


    def render_heading(self, heading_nodelist, render_context, *,
                       heading_level=1, target_id=None, inline_heading=False,
                       annotations=None):

        title_content = self.render_inline_content(heading_nodelist, render_context)

        target_id_md_code = self._get_target_id_md_code(target_id)

        return (
            '#'*heading_level + ' ' + target_id_md_code + title_content.replace('\n', ' ')
            + '\n'
        )

    def render_link(self, ref_type, href, display_nodelist, render_context, annotations=None):
        display_content = self.render_nodelist(
            display_nodelist,
            render_context=render_context,
            is_block_level=False,
        )
        return '[' + display_content + '](' + href + ')'
    
    def render_delayed_marker(self, node, delayed_key, render_context):
        return f"<FLM:DLYD:{delayed_key}/>"

    def render_delayed_dummy_placeholder(self, node, delayed_key, render_context):
        return f'<!-- delayed:{delayed_key} -->'

    def replace_delayed_markers_with_final_values(self, content, delayed_values):
        return _rx_delayed_markers.sub(
            lambda m: delayed_values[int(m.group('key'))],
            content
        )


    # --

    def render_float(self, float_instance, render_context):
        # see flm.feature.floats for FloatInstance

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

        target_id_md_code = self._get_target_id_md_code(float_instance.target_id)

        mdindent = render_context.get_logical_state('flm_markdown').get('md_indent', '')
        return mdindent + (
            '---\n\n' +
            target_id_md_code + 
            float_content_with_caption.strip() + '\n\n'
            '---\n'
        ).replace('\n', '\n'+mdindent)


    graphics_raster_magnification = 1
    graphics_vector_magnification = 1

    def render_graphics_block(self, graphics_resource, render_context):

        src_url = graphics_resource.src_url

        return '![](' + src_url + ')'


    def render_cells(self, cells_model, render_context, target_id=None):

        # gosh, just use HTML ... :/
        return HtmlFragmentRenderer().render_cells(
            cells_model, render_context, target_id=target_id
        )


    def _get_target_id_md_code(self, target_id):
        if target_id is None:
            return ''
        if self.use_target_ids == 'pandoc':
            return '[]{#' + target_id + '} '
        if self.use_target_ids == 'github':
            return '[](#' + target_id + ') '
        if self.use_target_ids == 'anchor':
            return '<a name="' + target_id + '"></a> '
        return ''




# ------------------


_rx_delayed_markers = re.compile(r'<FLM:DLYD:(?P<key>\d+)\s*/>')



# ------------------------------------------------------------------------------

class FragmentRendererInformation:
    FragmentRendererClass = MarkdownFragmentRenderer
    format_name = 'markdown'
