import re

from pylatexenc.latexencode import UnicodeToLatexEncoder

import logging
logger = logging.getLogger(__name__)

from ._base import FragmentRenderer


class LatexFragmentRenderer(FragmentRenderer):

    r"""
    .............

    AN IMPORTANT ASSUMPTION MADE BY THIS RENDERER: (No stray comments
    assumption.) At no point in rendered content is there a comment without a
    corresponding newline following it.
    """


    supports_delayed_render_markers = True
    """
    We use the marker ``\LLMDLYD{delayed_key}`` for delayed content, which
    cannot be confused with the rest of the LaTeX code that can be generated
    from this code generator.
    """

    heading_commands_by_level = {
        1: "chapter",
        2: "section",
        3: "subsection",
        4: "subsubsection",
        5: "paragraph",
        6: "subparagraph",
    }

    text_format_cmds = {
        'textit': 'textit',
        'textbf': 'textbf',
        'defterm-term': 'displayterm'
    }

    latex_semantic_block_environments = {
        'defterm': 'defterm'
    }

    # ------------------

    def __init__(self, **kwargs):
        super().__init__(**kwargs)
        self.latex_encoder = UnicodeToLatexEncoder(unknown_char_policy='unihex')

    # ------------------

    def latexescape(self, value):
        return self.latex_encoder.unicode_to_latex(value)


    def wrap_in_text_format_macro(self, value, text_formats, render_context):
        
        content = value

        for txtfmt in list(text_formats)[::-1]:
            # recursively wrap in text formatting commands
            txtfmtcmd = self.text_format_cmds.get(txtfmt,None)
            if txtfmtcmd:
                content = '\\'+txtfmtcmd+'{' + content + '}'

        return content


    def wrap_in_latex_enumeration_environment(self, annotations, items_content, render_context):
        return (
            r'\begin{itemize}'
            + '% ' + ",".join([a.replace('\n',' ') for a in annotations]) + "\n" #"\\relax{}"
            + items_content.strip()
            + '%\n'
            + r'\end{itemize}'
        )

    use_phantom_section = True
    latex_label_prefix = 'x:'

    def pin_label_here(self, target_id, display_latex, insert_phantom_section=True):
        s = ''
        if insert_phantom_section and self.use_phantom_section:
            s += r'\phantomsection '
        s += r'\expandafter\def\csname @currentlabel\endcsname{' + display_latex + '}'
        s += r'\label{' + self.latex_label_prefix + target_id + '}'
        return s

    # -----------------

    def render_build_paragraph(self, nodelist, render_context):
        return (
            "\n\n"
            + self.render_inline_content(nodelist, render_context)
            + "\n\n"
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
        #return "%\n\\relax{}".join([str(s) for s in content_list]) + "%\n\\relax{}"

        # '\n' in case one of the items ends with a comment
        #return "\n".join([ str(s).strip() for s in content_list ])
        result = ''
        for s in content_list:
            result = self._latex_join(result, str(s))
        return result

    def _latex_join(self, a, b):
        if '\n' in a:
            _, last_line = a.rsplit('\n', 1)
        else:
            last_line = a
        if '%' in last_line:
            print(f"LAST LINE COMMENT -> {a=}, {last_line=}, {b=}")
            return a + '\n' + b
        if re.search(r'\\[a-zA-Z]+$', a) is not None:
            # ends with named macro, need space
            return a + ' ' + b
        return a + b

    def render_join_blocks(self, content_list):
        r"""
        Join together a collection of pieces of content that have already been
        rendered.  Each piece is itself a block of content, which can assumed to
        be at least paragraph-level or even semantic blocks.  Usually you'd want
        to simply join the strings together with no joiner, which is what the
        default implementation does.
        """
        return "\n\n".join([ c.strip() for c in content_list]) + '\n'


    # ------------------

    def render_value(self, value):
        return self.latexescape(value)

    def render_empty_error_placeholder(self, debug_str):
        #return r"\relax % " + debug_str.replace('\n', ' ') + '\n\\relax{}'
        return "% " + debug_str.replace('\n', ' ') + "\n"

    def render_nothing(self, annotations=None):
        if not annotations:
            annotations = []
        else:
            annotations = [a.replace('\n', ' ') for a in annotations]
        #return r"\relax % " + " ".join(annotations) + '\n\\relax{}'
        return f"% {' '.join(annotations)}\n"

    latex_wrap_verbatim_macro = None

    def render_verbatim(self, value, *, annotations, target_id=None):
        # what to do with annotations / target_id ??
        if self.latex_wrap_verbatim_macro:
            return "\\" + self.latex_wrap_verbatim_macro + "{" + self.latexescape(value) + "}"
        return self.latexescape(value)

    def render_math_content(self,
                            delimiters,
                            nodelist,
                            render_context,
                            displaytype,
                            environmentname=None,
                            target_id=None):

        # recycle latex content as is
        return delimiters[0] + nodelist.latex_verbatim() + delimiters[1]


    def render_text_format(self, text_formats, nodelist, render_context):
        r"""
        """

        content = self.render_nodelist(
            nodelist,
            render_context,
            is_block_level=False
        )

        return self.wrap_in_text_format_macro(content, text_formats, render_context)

    def render_semantic_block(self, content, role, *, annotations=None, target_id=None):

        if not annotations:
            annotations = []
        else:
            annotations = [a.replace('\n', ' ') for a in annotations]

        # we don't have to worry about adding a \relax{}, we know we're in block
        # level mode
        begincmd = '% --- begin ' + ",".join(annotations) + " ---\n"
        endcmd = '% --- end ' + ",".join(annotations) + " ---\n"

        if role and role in self.latex_semantic_block_environments:
            envname = self.latex_semantic_block_environments[role]
            begincmd = r'\begin{' + envname + '}'
            endcmd = r'\end{' + envname + '}' + '%\n'

        lblcmd = ''
        if target_id:
            lblcmd = self.pin_label_here(target_id, '<block>', insert_phantom_section=True)

        return (
            begincmd
            + lblcmd
            + self._latex_join(content,
                               endcmd)
        )
 
    def render_enumeration(self, iter_items_nodelists, counter_formatter, render_context,
                           *, target_id_generator=None, annotations=None, nested_depth=None):

        r"""
        ... remember, counter_formatter is given a number starting at 1.

        ... target_id_generator is a callable, takes one argument (item #
        starting at 1, like counter_formatter), and returns the anchor name to
        use for the enumeration item (in LaTeX, will be used in \label{...})
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

            itemlabel = ''
            if target_id_generator is not None:
                this_target_id = target_id_generator(enumno)
                itemlabel = self.pin_label_here(this_target_id, tag_content,
                                                insert_phantom_section=True)

            s_items.append(
                "%\n" + r'\item[{' + tag_content + '}]' #+ '%\n\\relax{}'
                + itemlabel
                + item_content
            )

        if not annotations:
            annotations = []
        else:
            annotations = [a.replace('\n', ' ') for a in annotations]

        return self.wrap_in_latex_enumeration_environment(
            ['enumeration']+annotations,
            self.render_join(s_items),
            render_context
        )


    def render_heading(self, heading_nodelist, render_context, *,
                       heading_level=1, target_id=None, inline_heading=False,
                       annotations=None):

        if heading_level not in self.heading_commands_by_level:
            raise ValueError(f"Unknown {heading_level=}, expected one of "
                             f"{list(self.heading_commands_by_level.keys())}")

        heading_command = self.heading_commands_by_level[heading_level]

        # annot = list(annotations) if annotations else []
        # annot.append(f"heading-level-{heading_level}")
        # if inline_heading:
        #     annot.append('heading-inline')

        title_content = self.render_inline_content(heading_nodelist, render_context)

        labelcmd = ''
        if target_id:
            labelcmd = r'\label{'+self.latex_label_prefix+target_id+'}%\n' #+ '%\n\\relax{}'

        return (
            '\\' + heading_command + '{' + title_content + '}' + '%\n' #\\relax{}'
            + labelcmd
        )

    def render_link(self, ref_type, href, display_nodelist, render_context, annotations=None):

        if not annotations:
            annotations = []
        else:
            annotations = [a.replace('\n', ' ') for a in annotations]

        display_content = self.render_nodelist(
            display_nodelist,
            render_context=render_context,
            is_block_level=False,
        )
        if href[0:1] == '#':
            return self.render_latex_link_hyperref(
                display_content,
                href[1:],
            )
        return self.render_latex_link_href(
            display_content,
            href,
        )

    def render_latex_link_hyperref(self, display_content, to_target_id):
        return (
            r'\hyperref[{' + self.latex_label_prefix + to_target_id + '}]{'
            + display_content + '}'
        )

    def render_latex_link_href(self, display_content, href):
        return r'\href{' + href.replace('%','\%') + '}{' + display_content + '}'
    
    def render_delayed_marker(self, node, delayed_key, render_context):
        return r"\LLMDLYD{" + str(delayed_key) + "}"

    def render_delayed_dummy_placeholder(self, node, delayed_key, render_context):
        return f'% delayed:{delayed_key}\n' #+ r'\relax{}'

    def replace_delayed_markers_with_final_values(self, content, delayed_values):
        return _rx_delayed_markers.sub(
            lambda m: delayed_values[int(m.group('key'))],
            content
        )


    # --

    def render_float(self, float_instance, render_context):
        # see llm.features.floats for FloatInstance
        
        full_figcaption_rendered_list = []
        float_designator = None
        if float_instance.number is not None:
            # numbered float -- generate the "Figure X" part
            float_designator = (
                self.render_value(
                    float_instance.float_type_info.float_caption_name
                )
                + '~'
                + self.render_nodelist(
                    float_instance.formatted_counter_value_llm.nodes,
                    render_context=render_context
                )
            )
        elif float_instance.caption_nodelist:
            # not a numbered float, but there's a caption, so typeset "Figure: "
            # before the caption text
            float_designator = (
                self.render_value(float_instance.float_type_info.float_caption_name),
            )
        else:
            # not a numbered float, and no caption.
            pass

        labelcmd = ''
        if float_designator is not None:
            full_figcaption_rendered_list.append( float_designator )

            if float_instance.target_id is not None:
                labelcmd = self.pin_label_here(float_instance.target_id,
                                               float_designator,
                                               insert_phantom_section=True)

        if float_instance.caption_nodelist:
            # we still haven't rendered the caption text itself. We only
            # rendered the "Figure X" or "Figure" so far.  So now we add the
            # caption text.
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
            rendered_float_caption = (
                r'\par' +
                self.render_semantic_block(
                    labelcmd
                    + self.render_join(full_figcaption_rendered_list),
                    role='figure_caption'
                )
            )
        
        float_content_block_content = self.render_nodelist(
            float_instance.content_nodelist,
            render_context=render_context,
            is_block_level=True,
        )

        if rendered_float_caption is not None:
            float_content_with_caption = self.render_join_blocks([
                float_content_block_content,
                rendered_float_caption,
            ])
        else:
            float_content_with_caption = float_content_block

        return (
            r"\begin{" + float_instance.float_type + "}[h!]%\n"
            + r"\centering{}"
            + float_content_with_caption
            + r"\end{" + float_instance.float_type + "}"
        )

    graphics_raster_magnification = 1
    graphics_vector_magnification = 1

    def render_graphics_block(self, graphics_resource):

        whopt = ''
        if graphics_resource.physical_dimensions is not None:

            width_pt, height_pt = graphics_resource.physical_dimensions

            if graphics_resource.graphics_type == 'raster':
                if width_pt is not None:
                    width_pt *= self.graphics_raster_magnification
                if height_pt is not None:
                    height_pt *= self.graphics_raster_magnification
            elif graphics_resource.graphics_type == 'vector':
                if width_pt is not None:
                    width_pt *= self.graphics_vector_magnification
                if height_pt is not None:
                    height_pt *= self.graphics_vector_magnification

            whoptc = ''
            if width_pt is not None:
                whoptc += f"width={width_pt:.6f}pt,"
            if height_pt is not None:
                whoptc += f"height={height_pt:.6f}pt,"
            whopt = '['+whoptc+']'

        return r'\includegraphics' + whopt + '{' + graphics_resource.src_url + '}'


# ------------------


_rx_delayed_markers = re.compile(r'\\LLMDLYD\{(?P<key>\d+)\}')
