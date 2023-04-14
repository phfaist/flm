import html
import re

import logging
logger = logging.getLogger(__name__)

from ._base import FragmentRenderer



_rx_html_entity = re.compile(r'[&]([a-zA-Z]+|[#][0-9]+|[#]x[0-9a-fA-F]+);')


class HtmlFragmentRenderer(FragmentRenderer):

    supports_delayed_render_markers = True
    """
    We use the marker ``<FLM:DLYD:delayed_key/>`` for delayed content, which
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
    

    html_blocks_joiner = "\n"
    """
    Raw HTML string to insert between different blocks.  By default, we use a
    simple newline to avoid having very long lines in the HTML code.  For
    slightly smaller HTML code and if you don't mind long lines, use an empty
    string here.
    """

    
    #fix_punctuation_line_wrapping = True  # TODO!
    """
    Enable a fix that prevents punctuation marks (e.g., period, comma, etc.)
    from appearing on a new line after content wrapped in a tag, such as a
    citation or a footnote mark.

    FIXME: NOT SURE HOW TO DO THIS!
    """


    heading_tags_by_level = {
        1: "h1",
        2: "h2",
        3: "h3",
        # we use <span> instead of <h4> because these paragraph headings might
        # be rendered inline within the <p> element, and <h4> isn't allowed
        # within <p>...</p>
        4: "span",
        5: "span",
        6: "span",

        # special level for theorems:
        'theorem': "span",
    }

    inline_heading_add_space = True
    r"""
    Whether or not to include a space after an inline (run-in) heading, e.g.,
    for ``\paragraph``.  Visually, the space should be there, but removing it
    makes it much easier to control the space using CSS.
    """

    aggressively_escape_html_attributes = False
    r"""
    If True, then values of HTML attributes, e.g., the URL in ``<a
    href="....">``, are escaped as normal HTML with HTML entities like '&amp;'.
    The default setting, `aggressively_escape_html_attributes=False`, only
    escapes '"' characters, and will escape an '&' character only if it looks
    like part of an entity.  I.e., '/page?a=1&b=2' is not modified but
    '/page?a=1&b;3' will become '/page?a=1&amp;b;3'.

    Applying the general HTML escape mechanism to attribute values (setting
    `aggressively_escape_html_attributes=True`) can make them appear more
    obscure (e.g., the link to ``/page?a=1&b=2`` becomes ``/page?a=1&amp;b=2``).
    While this is correct HTML and works fine in probably all browsers of
    course, there might be some HTML parsers/skimmers that can choke on that
    syntax.  So the default setting is
    `aggressively_escape_html_attributes=False`.
    """

    render_nothing_as_comment_with_annotations = True


    use_mathjax = True

    # ------------------

    

    # ------------------

    def htmlescape(self, value):
        esc = html.escape(value)
        esc = (
            esc.replace(' ', '&nbsp;') # NON-BREAKING SPACE
            .replace(' ', '&hairsp;') # HAIR SPACE
            .replace(' ', '&thinsp;') # THIN SPACE
            .replace(' ', '&puncsp;') # PUNCTUATION SPACE
            .replace(' ', '&ensp;') # EN SPACE
            .replace(' ', '&emsp;') # EM SPACE
            .replace(' ', '&numsp;') # FIGURE SPACE
        )
        return esc

    def htmlescape_double_quoted_attribute_value(self, value):

        if self.aggressively_escape_html_attributes:
            return self.htmlescape(value)

        # try to be as gentle as possible on escaping values; we'd like to avoid
        # expanding '&' characters in query strings for instance in case
        # imperfect parsers or scanners find URLs and don't properly un-escape
        # all characters.  (E.g., I had some issues with parceljs.org when
        # generating links of the form "image.png?width=100&amp;height=100"
        # etc.)

        # escape the '&' in patterns that happen to look like HTML entities.
        value = _rx_html_entity.sub(lambda m: '&amp;'+m.group(1)+';', value)
        # also escape double quote characters !
        value = value.replace('"', '&quot;')
        return value

    def generate_open_tag(self, tagname, *, attrs=None, class_names=None, self_close_tag=False):
        s = f'<{tagname}'
        if not attrs:
            attrs = {}
        attrs = dict(attrs) # this way attrs can be either dict or list of 2-tuples
        if 'class' in attrs:
            raise ValueError(
                "generate_open_tag(): set HTML 'class' attribute with "
                "class_names=, not with attrs="
            )
        if class_names:
            attrs['class'] = ' '.join(class_names)
        if attrs:
            for aname, aval in attrs.items():
                s += f''' {aname}="{self.htmlescape_double_quoted_attribute_value(aval)}"'''
        if self_close_tag:
            s += '/>'
        else:
            s += '>'
        return s

    def wrap_in_tag(self, tagname, content_html, *,
                    attrs=None, class_names=None):
        s = self.generate_open_tag(tagname, attrs=attrs, class_names=class_names)
        s += str(content_html)
        s += f'</{tagname}>'
        return s

    def wrap_in_link(self, display_html, target_href, *, class_names=None):
        if not target_href: # e.g., None
            target_href = '#'
        attrs = {
            'href': target_href,
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
            [ self.render_node(n, render_context) for n in nodelist ],
            render_context
        )

    def render_join(self, content_list, render_context):
        r"""
        Join together a collection of pieces of content that have already been
        rendered.  Usually you'd want to simply join the strings together with
        no joiner, which is what the default implementation does.
        """
        return "".join([str(s) for s in content_list])

    def render_join_blocks(self, content_list, render_context):
        r"""
        Join together a collection of pieces of content that have already been
        rendered.  Each piece is itself a block of content, which can assumed to
        be at least paragraph-level or even semantic blocks.  Usually you'd want
        to simply join the strings together with no joiner, which is what the
        default implementation does.
        """
        return self.html_blocks_joiner.join(
            [c for c in content_list if c is not None and len(c)]
        )


    # ------------------

    def render_value(self, value, render_context):
        return self.htmlescape(value)

    def render_empty_error_placeholder(self, debug_str, render_context):
        debug_str_safe = debug_str.replace('--', '- - ')
        return f"<span class=\"empty-error-placeholder\"><!-- {debug_str_safe} -->(?)</span>"

    def render_nothing(self, render_context, annotations=None):
        if not self.render_nothing_as_comment_with_annotations:
            return ''
        if not annotations:
            annotations = []
        annotations = [a.replace('--', '- - ') for a in annotations]
        return '<!-- {} -->'.format(" ".join(annotations))

    verbatim_highlight_spaces = False
    verbatim_protect_backslashes = True

    def render_verbatim(self, value, render_context, *,
                        is_block_level=False, annotations=None, target_id=None):
        attrs = {}
        if target_id is not None:
            attrs['id'] = target_id
        escaped = self.htmlescape(value)
        if self.verbatim_protect_backslashes:
            # so that MathJax doesn't automatically kick in in verbatim content
            escaped = escaped.replace('\\', '<span>\\</span>')
        if self.verbatim_highlight_spaces:
            escaped = escaped.replace(
                ' ', '<span class="verbatimspace">&nbsp;</span>'
            )
        tag = 'span'
        for annotation in annotations:
            if annotation in ('verbatimcode-environment', ):
                # indicates a verbatim block, use <div> instead
                tag = 'div'
        return self.wrap_in_tag(
            tag,
            escaped,
            class_names=(annotations if annotations else ['verbatimtext']),
            attrs=attrs,
        )

    def render_math_content(self,
                            delimiters,
                            nodelist,
                            render_context,
                            displaytype,
                            environmentname=None,
                            target_id=None):

        if not self.use_mathjax:
            logger.warning(
                "called HtmlFragmentRenderer.render_math_content() but "
                "self.use_mathjax is not set. Your math "
                "will probably not render correctly."
            )

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

    def render_semantic_span(self, content, role, render_context, *,
                             annotations=None, target_id=None):
        attrs = {}
        if target_id is not None:
            attrs['id'] = target_id

        annotations = list(annotations if annotations is not None else [])
        if role in annotations:
            annotations.remove(role)

        return self.wrap_in_tag(
            'span',
            content,
            attrs=attrs,
            class_names=[role]+annotations,
        )
        

    def render_semantic_block(self, content, role, render_context, *,
                              annotations=None, target_id=None):
        attrs = {}
        if target_id is not None:
            attrs['id'] = target_id

        annotations = list(annotations if annotations is not None else [])
        if role in annotations:
            annotations.remove(role)

        if role in ('section', 'main', 'article', ): # todo, add
            return self.wrap_in_tag(
                role,
                content,
                attrs=attrs,
                class_names=annotations,
            )
        return self.wrap_in_tag(
            'div',
            content,
            attrs=attrs,
            class_names=[role] + annotations
        )
            
 
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
                ], render_context)
            )

        return self.wrap_in_tag(
            'dl',
            self.render_join(s_items, render_context),
            class_names=['enumeration'] + (annotations if annotations else []),
        )


    def render_heading(self, heading_nodelist, render_context, *,
                       heading_level=1, target_id=None, inline_heading=False,
                       annotations=None):

        if heading_level not in self.heading_tags_by_level:
            raise ValueError(f"Bad {heading_level=}, expected one of "
                             f"{list(self.heading_tags_by_level.keys())}")

        annot = list(annotations) if annotations else []
        annot.append(f"heading-level-{heading_level}")
        if inline_heading:
            annot.append('heading-inline')

        attrs = {}
        if target_id is not None:
            attrs['id'] = target_id

        content = self.wrap_in_tag(
            self.heading_tags_by_level[heading_level],
            self.render_inline_content(heading_nodelist, render_context),
            class_names=annot,
            attrs=attrs,
        )
        if inline_heading and self.inline_heading_add_space:
            content += ' '
        logger.debug("Rendered heading: content=%r; inline_heading=%r; "
                     "add_space=%r", content, inline_heading, self.inline_heading_add_space)
        return content

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
        # see flm.features.floats for FloatInstance
        
        figattrs = {}

        if float_instance.target_id is not None:
            figattrs['id'] = float_instance.target_id

        full_figcaption_rendered_list = []
        if float_instance.number is not None:
            # numbered float -- generate the "Figure X" part
            full_figcaption_rendered_list.append(
                self.wrap_in_tag(
                    'span',
                    self.render_join([
                        self.render_value(
                            float_instance.float_type_info.float_caption_name,
                            render_context
                        ),
                        '&nbsp;',
                        self.render_nodelist(
                            float_instance.formatted_counter_value_flm.nodes,
                            render_context=render_context
                        ),
                    ], render_context),
                    class_names=['float-number'],
                )
            )
        elif float_instance.caption_nodelist:
            # not a numbered float, but there's a caption, so typeset "Figure: "
            # before the caption text
            full_figcaption_rendered_list.append(
                self.wrap_in_tag(
                    'span',
                    self.render_join([
                        self.render_value(
                            float_instance.float_type_info.float_caption_name,
                            render_context,
                        ),
                    ], render_context),
                    class_names=['float-no-number'],
                )
            )
        else:
            # not a numbered float, and no caption.
            pass

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
            rendered_float_caption = self.wrap_in_tag(
                'figcaption',
                self.wrap_in_tag(
                    'span',
                    self.render_join(full_figcaption_rendered_list, render_context),
                ),
                class_names=['float-caption-content'],
            )
        
        float_content_block_content = self.render_nodelist(
            float_instance.content_nodelist,
            render_context=render_context,
            is_block_level=True,
        )
        float_content_block = self.render_semantic_block(
            float_content_block_content,
            'float-contents',
            render_context=render_context,
        )

        if rendered_float_caption is not None:
            float_content_with_caption = self.render_join_blocks([
                float_content_block,
                rendered_float_caption,
            ], render_context)
        else:
            float_content_with_caption = float_content_block

        full_figure = self.wrap_in_tag(
            'figure',
            float_content_with_caption,
            attrs=figattrs,
            class_names=['float', f"float-{float_instance.float_type}",]
        )

        return full_figure


    graphics_raster_magnification = 1
    graphics_vector_magnification = 1

    def render_graphics_block(self, graphics_resource, render_context):

        imgattrs = {}

        styparts = []
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

            if width_pt is not None:
                styparts.append(f"width:{width_pt:.6f}pt")
            if height_pt is not None:
                styparts.append(f"height:{height_pt:.6f}pt")

        if styparts:
            imgattrs['style'] = ";".join(styparts)
        
        src_url = graphics_resource.src_url
        imgattrs['src'] = src_url

        if graphics_resource.srcset is not None and len(graphics_resource.srcset):
            imgattrs['srcset'] = graphics_resource.srcset

        # HTML does not require any closing tag
        return self.generate_open_tag('img', attrs=imgattrs)


    def render_cells(self, cells_model, render_context, target_id=None):

        tabheight, tabwidth = len(cells_model.grid_data), len(cells_model.grid_data[0])

        data_items = []
        row_j = 0
        while row_j < len(cells_model.grid_data):
            row_items = []
            col_j = 0
            while col_j < len(cells_model.grid_data[row_j]):

                grid_cell_data = cells_model.grid_data[row_j][col_j]

                if grid_cell_data is None or grid_cell_data['cell'] is None:
                    # no contents here, still need to render an empty cell for
                    # the HTML layout
                    row_items.append(self.wrap_in_tag(
                        'td',
                        '',
                        class_names=['cell-empty']
                    ))
                    col_j += 1
                    continue

                if grid_cell_data['is_topleft']:

                    cell = grid_cell_data['cell']
                    rendered_cell_contents = self.render_nodelist(
                        cell.content_nodes,
                        render_context=render_context,
                    )
                    clsnames = ['cell'] + [ f"cellstyle-{sty}" for sty in cell.styles ]
                    if row_j == 0:
                        clsnames.append('celltbledge-top')
                    if col_j == 0:
                        clsnames.append('celltbledge-left')
                    if cell.placement.row_range.end == tabheight:
                        clsnames.append('celltbledge-bottom')
                    if cell.placement.col_range.end == tabwidth:
                        clsnames.append('celltbledge-right')
                    tagname = 'td'
                    if 'H' in cell.styles or 'rH' in cell.styles:
                        tagname = 'th'
                    attrs = {}
                    cplc = cell.placement
                    if cplc.col_range.end != cplc.col_range.start + 1:
                        # nontrivial column span
                        attrs['colspan'] = \
                            str(cplc.col_range.end - cplc.col_range.start)
                    if cplc.row_range.end != cplc.row_range.start + 1:
                        # nontrivial row span
                        attrs['rowspan'] = str(cplc.row_range.end - cplc.row_range.start)
                    row_items.append(
                        self.wrap_in_tag(
                            tagname,
                            rendered_cell_contents,
                            attrs=attrs,
                            class_names=clsnames,
                        )
                    )
                    col_j = cplc.col_range.end
                    continue

                # no need to render a <td> item because the spot is occupied by
                # a cell with a nontrivial rowspan or colspan.
                col_j += 1

            data_items.append( row_items )
            row_j += 1

        table_attrs = {}
        if target_id is not None:
            table_attrs['id'] = target_id

        s = self.wrap_in_tag(
            'table',
            ''.join([
                '<tr>' + ''.join(row_items) + '</tr>'
                for row_items in data_items
            ]),
            attrs=table_attrs,
            class_names=['cells'],
        )
        return s


# ------------------


_rx_delayed_markers = re.compile(r'<FLM:DLYD:(?P<key>\d+)\s*/>')




# ------------------------------------------------------------------------------

_html_css_global = r"""
p, ul, ol {
  margin: 1em 0px;
}
p:first-child, ul:first-child, ol:first-child {
  margin-top: 0px;
}
p:last-child, ul:last-child, ol:last-child {
  margin-bottom: 0px;
}
dd > p, dd > p:first-child, dd > p:last-child {
  margin: 0.33em 0px;
}

a, a:link, a:hover, a:active, a:visited {
  color: #3232c8;
  text-decoration: none;
}
a:hover {
  color: #22228a;
}
"""

_html_css_content = r"""
.emph, .textit {
  font-style: italic;
}
.textbf {
  font-weight: bold;
}

h1 {
  font-size: 1.6rem;
  line-height: 1.3em;
  font-weight: bold;
  margin: 1em 0px;
}
h2 {
  font-size: 1.3rem;
  line-height: 1.3em;
  font-weight: bold;
  margin: 1em 0px;
}
h3 {
  font-size: 1rem;
  font-weight: bold;
  margin: 1em 0px;
}

.heading-level-4 {
  font-style: italic;
  display: inline;
}
.heading-level-4::after {
  display: inline-block;
  margin: 0px .12em;
  content: '—';
}

.heading-level-5 {
  font-style: italic;
  font-size: .9em;
  display: inline;
}
.heading-level-5::after {
  display: inline-block;
  margin-right: .12em;
  content: '';
}

.heading-level-6 {
  font-style: italic;
  font-size: .8em;
  display: inline;
}
.heading-level-6::after {
  display: inline-block;
  margin-right: .06em;
  content: '';
}


.heading-level-theorem {
  font-weight: bold;
  display: inline-block;
}
.heading-level-theorem::after {
  font-weight: bold;
  display: inline-block;
  margin: 0px .12em 0px 0px;
  content: '.';
}

div.theoremlike, div.definitionlike, div.prooflike {
  margin: 1em 0px;
}

div.prooflike > p > .heading-level-theorem,
div.prooflike > p > .heading-level-theorem::after {
  font-weight: normal;
  font-style: italic;
}


dl.enumeration {
  display: block;
  margin-left: 2.5em;
}
dl.enumeration > dt {
  float: left;
  clear: left;
  display: inline-block;
  /*margin: 0px; */
  margin-left: -2.5em;
  width: 2.0em;
  min-width: 2.0em;
  max-width: 2.0em;
  margin-right: 0.5em;
  text-align: right;
}
dl.enumeration > dd {
  /*display: inline-block;
  width: 100%;*/
  border: 0px;
  padding: 0px;
  margin: 0px;
}

figure.float {
  width: 100%;
  border-width: 1px 0px 1px 0px;
  border-style: solid none solid none;
  border-color: rgba(120, 120, 140, 0.15);
  margin: 0.5rem 0px;
  padding: 0.5rem 0px;
}

figure.float .float-contents {
  width: 100%;
  max-width: 100%;
  overflow-x: auto;
}

figure.float .float-contents img {
  display: block;
  margin: 0pt auto;
  padding: 0pt;
  border: 0pt;
  margin: 0px auto;
}

figure.float figcaption {
  display: block;
  margin-top: 0.5em;
  margin: 0.75em 2em 0px;
  text-align: center;
}

figure.float figcaption > span {
  display: inline-block;
  font-style: italic;
  text-align: left;
}

table {
  margin: 1em 0px 1em 0px;
  border-collapse: separate;
  border-spacing: 0px;
  /*border-top: solid 1pt;
  border-bottom: solid 1pt;*/
}
figure.float .float-contents table {
  margin: 0px auto;
}
td {
  padding: 0.3em 0.5em;
  border: none 0px;
}
th {
  padding: 0.3em 0.5em;
  border: none 0px;
}
.cellstyle-H {
  border-bottom: solid .5pt;
}
.cellstyle-rH {
}
.cellstyle-l {
  text-align: left;
}
.cellstyle-c {
  text-align: center;
}
.cellstyle-r {
  text-align: right;
}
.cellstyle-green {
  background-color: rgba(69, 255, 69, 0.31); /*rgb(200,255,200);*/
}
.cellstyle-blue {
  background-color: rgba(79, 142, 255, 0.27); /*rgb(200,220,255);*/
}
.cellstyle-yellow {
  background-color: rgba(255, 255, 49, 0.33); /*rgb(255,255,200);*/
}
.cellstyle-red {
  background-color: rgba(255,120,120,0.30);  /*rgb(255,200,200);*/
}
.cellstyle-lvert {
  border-left: solid .5pt;
}
.cellstyle-rvert {
  border-right: solid .5pt;
}

.celltbledge-top {
  border-top: solid 1pt;
}
.celltbledge-bottom {
  border-bottom: solid 1pt;
}

.verbatimcode {
  font-family: monospace;
  font-size: 0.9em;
  background-color: rgba(127,127,127,0.25);
  border-radius: 2px;
  padding: 1px 2px;
  display: inline-block;
  white-space: pre-wrap;
}
.verbatima {
  font-style: italic;
}
.verbatimcode-environment {
  display: block;
  margin: 0.75em 0px 1em;
  white-space: pre;
}

.defterm {
  font-style: italic;
}

.defterm .defterm-term {
  font-style: italic;
  font-weight: bold;
}

.display-math {
  width: 100%;
  max-width: 100%;
  display: block;
  overflow-x: auto;
}

.endnotes, .citations {
  font-size: 0.8em;
  display: inline-block;
  vertical-align: 0.3em;
  margin-top: -0.3em;
}
.citation {
}
.footnote {
}
dl.citation-list > dt, dl.footnote-list > dt {
  font-size: 0.8em;
  display: inline-block;
  vertical-align: 0.3em;
  margin-top: -0.3em;
}
"""


_html_js_mathjax = r"""
MathJax = {
    tex: {
        inlineMath: [['\\(', '\\)']],
        displayMath: [['\\[', '\\]']],
        processEnvironments: true,
        processRefs: true,

        // equation numbering on
        tags: 'ams'
    },
    options: {
        // all MathJax content is marked with CSS classes
        // skipHtmlTags: 'body',
        // processHtmlClass: 'display-math|inline-math',
    },
    startup: {
        pageReady: function() {
            // override the default "typeset everything on the page" behavior to
            // only typeset whatever we have explicitly marked as math
            return typesetPageMathPromise();
        }
    }
};
function typesetPageMathPromise()
{
    var elements = document.querySelectorAll('.display-math, .inline-math');
    return MathJax.typesetPromise(elements);
}
"""

_html_body_end_js_scripts_mathjax = r"""
<script src="https://polyfill.io/v3/polyfill.min.js?features=es6"></script>
<script id="MathJax-script" async src="https://cdn.jsdelivr.net/npm/mathjax@3/es5/tex-mml-chtml.js"></script>
"""


# ------------------------------------------------------------------------------

def get_html_css_global(html_fragment_renderer):
    return _html_css_global

def get_html_css_content(html_fragment_renderer):
    return _html_css_content

def get_html_js(html_fragment_renderer):
    if html_fragment_renderer.use_mathjax:
        return _html_js_mathjax
    return ''

def get_html_body_end_js_scripts(html_fragment_renderer):
    if html_fragment_renderer.use_mathjax:
        return _html_body_end_js_scripts_mathjax
    return ''



# ------------------------------------------------------------------------------

class FragmentRendererInformation:
    FragmentRendererClass = HtmlFragmentRenderer

    @staticmethod
    def get_style_information(fragment_renderer):
        return {
            'css_global': get_html_css_global(fragment_renderer),
            'css_content': get_html_css_content(fragment_renderer),
            'js': get_html_js(fragment_renderer),
            'body_end_js_scripts': get_html_body_end_js_scripts(fragment_renderer),
        }

    format_name = 'html'
