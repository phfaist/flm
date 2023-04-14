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
    r"""
    We use the marker ``\FLMDLYD{delayed_key}`` for delayed content, which
    cannot be confused with the rest of the LaTeX code that can be generated
    from this code generator.
    """

    heading_commands_by_level = {
        1: "section",
        2: "subsection",
        3: "subsubsection",
        4: "paragraph",
        5: "subparagraph",
        6: "subsubparagraph",
        
        # special heading type for theorems
        'theorem': "flmTheoremHeading",
    }

    text_format_cmds = {
        'textit': 'textit',
        'textbf': 'textbf',
        'defterm-term': 'displayterm'
    }

    latex_semantic_block_environments = {
        'defterm': 'defterm',
        
        'theoremlike': 'flmThmTheoremLike',
        'definitionlike': 'flmThmDefinitionLike',
        'prooflike': 'flmThmProofLike',
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
            [ self.render_node(n, render_context) for n in nodelist ],
            render_context
        )

    def render_join(self, content_list, render_context):
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
            #print(f"(potential) LAST LINE COMMENT -> {a=}, {last_line=}, {b=}")
            return a + '\n' + b
        if re.search(r'\\[a-zA-Z]+$', a) is not None:
            # ends with named macro, need space
            return a + ' ' + b
        return a + b

    def render_join_blocks(self, content_list, render_context):
        r"""
        Join together a collection of pieces of content that have already been
        rendered.  Each piece is itself a block of content, which can assumed to
        be at least paragraph-level or even semantic blocks.  Usually you'd want
        to simply join the strings together with no joiner, which is what the
        default implementation does.
        """
        return "\n\n".join([ c.strip() for c in content_list]) + '\n'


    # ------------------

    def render_value(self, value, render_context):
        return self.latexescape(value)

    def render_empty_error_placeholder(self, debug_str, render_context):
        #return r"\relax % " + debug_str.replace('\n', ' ') + '\n\\relax{}'
        return "% " + debug_str.replace('\n', ' ') + "\n"

    def render_nothing(self, render_context, annotations=None):
        if not annotations:
            annotations = []
        else:
            annotations = [a.replace('\n', ' ') for a in annotations]
        #return r"\relax % " + " ".join(annotations) + '\n\\relax{}'
        return f"% {' '.join(annotations)}\n"

    latex_wrap_verbatim_macro = None

    def render_verbatim(self, value, render_context, *,
                        is_block_level=False, annotations=None, target_id=None):
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


    use_endnote_latex_command = None #'textsuperscript'
    use_citation_latex_command = None #'textsuperscript'

    def render_semantic_span(self, content, role, render_context, *,
                             annotations=None, target_id=None):

        if self.use_endnote_latex_command is not None and role == 'endnotes':
            content = (
                '\\' + self.use_endnote_latex_command + '{' + content + '}'
            )
        if self.use_citation_latex_command is not None and role == 'citations':
            content = (
                '\\' + self.use_citation_latex_command + '{' + content + '}'
            )

        return content


    def render_semantic_block(self, content, role, render_context, *,
                              annotations=None, target_id=None):

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
                tag_content = self.render_value(tag_nodelist, render_context)
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
            self.render_join(s_items, render_context),
            render_context
        )


    def render_heading(self, heading_nodelist, render_context, *,
                       heading_level=1, target_id=None, inline_heading=False,
                       annotations=None):

        if heading_level not in self.heading_commands_by_level:
            raise ValueError(f"Unknown {heading_level=}, expected one of "
                             f"{list(self.heading_commands_by_level.keys())}")

        heading_command = self.heading_commands_by_level[heading_level]

        title_content = self.render_inline_content(heading_nodelist, render_context)

        labelcmd = ''
        if target_id:
            labelcmd = r'\label{'+self.latex_label_prefix+target_id+'}%\n' #+ '%\n\\relax{}'

        return (
            '\\' + heading_command + '{' + title_content + '}' + '%\n' #\\relax{}'
            + labelcmd
        )

    def render_link(self, ref_type, href, display_nodelist, render_context, annotations=None):

        display_content = self.render_nodelist(
            display_nodelist,
            render_context=render_context,
            is_block_level=False,
        )

        annotations = annotations or []

        if self.use_endnote_latex_command is not None and 'endnotes' in annotations:
            display_content = (
                '\\' + self.use_endnote_latex_command + '{' + display_content + '}'
            )
        if self.use_citation_latex_command is not None and 'citations' in annotations:
            display_content = (
                '\\' + self.use_citation_latex_command + '{' + display_content + '}'
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
        return r'\href{' + href.replace(r'%',r'\%') + r'}{' + display_content + r'}'
    
    def render_delayed_marker(self, node, delayed_key, render_context):
        return r"\FLMDLYD{" + str(delayed_key) + "}"

    def render_delayed_dummy_placeholder(self, node, delayed_key, render_context):
        return f'% delayed:{delayed_key}\n' #+ r'\relax{}'

    def replace_delayed_markers_with_final_values(self, content, delayed_values):
        return _rx_delayed_markers.sub(
            lambda m: delayed_values[int(m.group('key'))],
            content
        )


    # --

    def render_float(self, float_instance, render_context):
        # see flm.features.floats for FloatInstance
        
        full_figcaption_rendered_list = []
        float_designator = None
        if float_instance.number is not None:
            # numbered float -- generate the "Figure X" part
            float_designator = (
                self.render_value(
                    float_instance.float_type_info.float_caption_name,
                    render_context,
                )
                + '~'
                + self.render_nodelist(
                    float_instance.formatted_counter_value_flm.nodes,
                    render_context=render_context
                )
            )
        elif float_instance.caption_nodelist:
            # not a numbered float, but there's a caption, so typeset "Figure: "
            # before the caption text
            float_designator = (
                self.render_value(
                    float_instance.float_type_info.float_caption_name,
                    render_context,
                )
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
                    + self.render_join(full_figcaption_rendered_list, render_context),
                    role='figure_caption',
                    render_context=render_context,
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
            ], render_context)
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

    def render_graphics_block(self, graphics_resource, render_context):

        src_url, incloptions = self.collect_graphics_resource(graphics_resource, render_context)

        opts = ''
        if incloptions is not None:
            opts = '['+incloptions+']'

        return r'\includegraphics' + opts + '{' + src_url + '}'

    def collect_graphics_resource(self, graphics_resource, render_context):
        # can be reimplemented to collect the given graphics resource somewhere
        # relevant etc.

        whoptc = None
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

        return graphics_resource.src_url, whoptc

    def render_cells(self, cells_model, render_context, target_id=None):

        # no support for styles yet ...
        # logger.warning("LaTeX output only has very rudimentary support for tables !")

        stab_contents = ''

        cell_spans_styles = ''
        cell_hlines = []
        cell_vlines = []

        tabheight, tabwidth = len(cells_model.grid_data), len(cells_model.grid_data[0])

        for row in cells_model.grid_data:
            stab_rowitems = []
            # row_has_any_non_header_element = False
            for cellinfo in row:
                if (cellinfo is not None and cellinfo['cell'] is not None
                    and cellinfo['is_topleft']):
                    #
                    cell = cellinfo['cell']
                    cell_content =  self.render_nodelist(
                        cell.content_nodes,
                        render_context=render_context,
                    )

                    # if we're spanning multiple rows/columns, we need a
                    # cell={...} specifier...
                    thiscellspanopts = []
                    rowj = cell.placement.row_range.start
                    rowjend = cell.placement.row_range.end
                    numrows = rowjend - rowj
                    if numrows > 1:
                        thiscellspanopts.append(f'r={numrows}')

                    colj = cell.placement.col_range.start
                    coljend = cell.placement.col_range.end
                    numcols = coljend - colj
                    if numcols > 1:
                        thiscellspanopts.append(f'c={numcols}')

                    thiscellstyles = 'm'
                    if 'l' in cell.styles:
                        thiscellstyles = 'l'
                    elif 'c' in cell.styles:
                        thiscellstyles = 'c'
                    elif 'r' in cell.styles:
                        thiscellstyles = 'r'

                    bgcol = None
                    if 'green' in cell.styles:
                        bgcol = 'flmTabCellColorGreen'
                    elif 'red' in cell.styles:
                        bgcol = 'flmTabCellColorRed'
                    elif 'blue' in cell.styles:
                        bgcol = 'flmTabCellColorBlue'
                    elif 'yellow' in cell.styles:
                        bgcol = 'flmTabCellColorYellow'

                    if bgcol:
                        thiscellstyles += f', bg={bgcol}'

                    if 'H' in cell.styles or 'rH' in cell.styles:
                        thiscellstyles += r', font={\flmCellsHeaderFont}'

                    if 'H' in cell.styles:
                        if coljend == colj+1:
                            colnstr = f'{1+colj}'
                        else:
                            colnstr = f'{1+colj}-{coljend}'
                        # only add hline if it is not already at the bottom of
                        # the table (where the \bottomline is already enforced)
                        if rowjend < tabheight:
                            cell_hlines.append( (str(1+rowjend), colnstr, '.4pt,solid') )

                    if 'lvert' in cell.styles or 'rvert' in cell.styles:
                        if rowjend == rowj+1:
                            rownstr = f'{1+rowj}'
                        else:
                            rownstr = f'{1+rowj}-{rowjend}'

                        if 'lvert' in cell.styles:
                            cell_vlines.append( (rownstr, str(1+colj), '.4pt,solid') )
                        if 'rvert' in cell.styles:
                            cell_vlines.append( (rownstr, str(2+colj), '.4pt,solid') )

                    if len(thiscellspanopts) > 0 or thiscellstyles != 'm':
                        cell_spans_styles += (
                            ',\n  cell{' + str(1+rowj) + r'}{' + str(1+colj) + r'}='
                            + r'{' + ','.join(thiscellspanopts) + r'}{'
                            + thiscellstyles
                            + r'}'
                        )
                else:
                    cell_content = '' # part of multicell

                stab_rowitems.append(cell_content)

            stab_contents += '&'.join(stab_rowitems) + '\\\\' + '\n'

        s = (
            r'\begin{center}' + '\n'
            # Hack for automatic width detection -- typeset table once with 'c'
            # column types; if the width exceeds a maximum set width
            # (0.96\linewidth), then re-typeset the table with 'X[-1]' column
            # types.
            r'\long\def\flmTempTypesetThisTable#1{%' + '\n'
            r'\begin{tblr}{#1,' + '\n' + r'  hspan=minimal'
            + cell_spans_styles
            + "".join([ ",\n  hline{"+rownrng+"}={"+colnrng+"}{"+lsty+"}"
                         for (rownrng, colnrng, lsty) in cell_hlines ])
            + "".join([ ",\n  vline{"+colnrng+"}={"+rownrng+"}{"+lsty+"}"
                         for (rownrng, colnrng, lsty) in cell_vlines ])
            + r'}' + '\n'
            + r'\toprule'
            + '\n'
        )
        s += stab_contents
        s += r'\bottomrule' + '\n'
        s += r'\end{tblr}%' + '\n'
        s += r'}%' + '\n'
        # now, the code to automatically detect the correct width
        s += (
            r'\def\flmTmpMaxW{\dimexpr ' + self.max_table_width_latexdim + r'\relax}%' + '\n'
            + r'\setbox0=\hbox{\flmTempTypesetThisTable{colspec={'
                + ('c' * tabwidth) + r'}}}%' + '\n'
            + r'\ifdim\wd0<\flmTmpMaxW\relax' + '\n'
            + r'  \leavevmode\box0 ' + '\n'
            + r'\else' + '\n'
            + r'  \flmTempTypesetThisTable{width=\flmTmpMaxW,colspec={'
                + ('X[-1]' * tabwidth) + r'}}' + '\n'
            + r'\fi' + '\n'
        )
        s += r'\end{center}'

        return s

    max_table_width_latexdim = r'0.96\linewidth'

# ------------------


_rx_delayed_markers = re.compile(r'\\FLMDLYD\{(?P<key>\d+)\}')



# ------------------------------------------------------------------------------
#
# some style defaults
#

_latex_preamble_suggested_defs = r"""

\usepackage{amsmath}
\usepackage{amssymb}

\usepackage{graphicx}
\usepackage{xcolor}

\providecommand\phantomsection{}

\ifdefined\defterm\else
\newenvironment{defterm}{%
  \par\vspace{0.5ex plus 0.5ex}\noindent
  \begingroup\itshape
}{%
  \endgroup\par\vspace{0.5ex plus 0.5ex}%
}
\fi

\providecommand\displayterm[1]{\textbf{#1}}

\providecommand\flmThmHeadingTheoremLike[1]{\textbf{#1}.\hspace{.8em}\ignorespaces}
\providecommand\flmThmHeadingDefinitionLike[1]{\textbf{#1}.\hspace{.8em}\ignorespaces}
\providecommand\flmThmHeadingProofLike[1]{\textit{#1}.\hspace{.8em}\ignorespaces}
\providecommand\flmTheoremHeading{\flmThmHeadingTheoremLike}
\ifdefined\flmThmTheoremLike\else
\newenvironment{flmThmTheoremLike}{%
  \par\vspace{0.5ex plus 0.5ex}\noindent
  \let\flmTheoremHeading\flmThmHeadingTheoremLike
}{%
  \par\vspace{0.5ex plus 0.5ex}%
}
\fi
\ifdefined\flmThmDefinitionLike\else
\newenvironment{flmThmDefinitionLike}{%
  \par\vspace{0.5ex plus 0.5ex}\noindent
  \let\flmTheoremHeading\flmThmHeadingDefinitionLike
}{%
  \par\vspace{0.5ex plus 0.5ex}%
}
\fi
\ifdefined\flmThmProofLike\else
\newenvironment{flmThmProofLike}{%
  \par\vspace{0.5ex plus 0.5ex}\noindent
  \let\flmTheoremHeading\flmThmHeadingProofLike
}{%
  \par\vspace{0.5ex plus 0.5ex}%
}
\fi

% for cells/tables
\usepackage{tabularray}
\UseTblrLibrary{booktabs}
\definecolor{flmTabCellColorGreen}{RGB}{200,255,200}
\definecolor{flmTabCellColorBlue}{RGB}{200,220,255}
\definecolor{flmTabCellColorYellow}{RGB}{255,255,200}
\definecolor{flmTabCellColorRed}{RGB}{255,200,200}
\providecommand\flmCellsHeaderFont{\bfseries}

"""


# ------------------------------------------------------------------------------

class FragmentRendererInformation:
    FragmentRendererClass = LatexFragmentRenderer

    @staticmethod
    def get_style_information(fragment_renderer):
        return {
            'preamble_suggested_defs': _latex_preamble_suggested_defs
        }

    format_name = 'latex'
