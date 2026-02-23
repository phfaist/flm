import logging
logger = logging.getLogger(__name__)

from pylatexenc.latexnodes import (
    LatexWalkerLocatedError, ParsedArgumentsInfo
)
from pylatexenc.latexnodes import nodes as latexnodes_nodes

from pylatexenc.macrospec import (
    LatexEnvironmentBodyContentsParser,
    MacroSpec,
    ParsingStateDeltaExtendLatexContextDb,
)

from ..flmenvironment import (
    FLMArgumentSpec
)

from ..flmspecinfo import (
    FLMEnvironmentSpecBase,
    text_arg
)

from ._base import Feature






_quote_section_lines_environment = {
    'macros': [
        MacroSpec('\\'),
        MacroSpec('indent'), # manually indent some lines by one "tab width"
    ],
    'specials': [
    ]
}



_quote_section_macros = {

    # the main argument for each of these quote section macros should always be
    # named 'text'.

    'text': MacroSpec('text', arguments_spec_list=[
        text_arg,
    ],),

    'lines': MacroSpec('lines', arguments_spec_list=[
        FLMArgumentSpec(
            parser='{',
            argname='text',
            flm_doc='The text or FLM content to process',
            parsing_state_delta=ParsingStateDeltaExtendLatexContextDb(
                extend_latex_context=_quote_section_lines_environment,
                set_attributes={'is_block_level': False },
            )
        )
    ],),

    'attributed': MacroSpec('attributed', arguments_spec_list=[
        text_arg,
    ],),

    'block': MacroSpec('block', arguments_spec_list=[
        FLMArgumentSpec(
            parser='{',
            argname='text',
            flm_doc='The text or FLM content to process',
            #is_block_level=True,
        )
    ],),
}


class LineInfo:
    def __init__(self,
                 *,
                 nodelist=None,
                 align=None,
                 indent_left=None,
                 indent_right=None,
                 ):
        super().__init__()
        self.nodelist = nodelist
        self.align = align
        self.indent_left = indent_left
        self.indent_right = indent_right
        self._fields = ('nodelist', 'align',
                        'indent_left', 'indent_right',)

    def asdict(self):
        return {k: getattr(self, k) for k in self._fields}

    def __repr__(self):
        return "{}({})".format(
            self.__class__.__name__,
            ", ".join([ f"{k}={getattr(self,k)!r}" for k in self._fields ])
        )




class QuoteEnvironment(FLMEnvironmentSpecBase):
    r"""
    A {quote} type environment.  "Quote Sections" are commands that appear
    in the {quote} environment. They can be "quote" (a literary quote, formatted
    as a paragraph; can contain ``\\`` newlines), "attributed" (quote
    attribution, e.g. author and year), "block" (typeset text normally but
    formatted as a block quote; think quoting a reply in an email).
    """
    
    is_block_level = True

    body_contents_is_block_level = True

    def __init__(self, environmentname,
                 enabled_quote_sections=None,
                 content_is_block_level=True,
                 auto_quote_section_bare_content=False):
        super().__init__(
            environmentname,
            arguments_spec_list=[],
        )
        if enabled_quote_sections and len(enabled_quote_sections):
            self.enabled_quote_sections = list(enabled_quote_sections)
        else:
            self.enabled_quote_sections = []
        self.auto_quote_section_bare_content = auto_quote_section_bare_content
        self.content_is_block_level = content_is_block_level

        if (self.auto_quote_section_bare_content == 'lines' 
            and self.content_is_block_level):
            logger.warning(
                f"Configuration of quote-type environment {'{'}{environmentname}{'}'} "
                f"might be flawed: using '\\lines' for bare environment content "
                f"(auto_quote_section_bare_content='lines') but content_is_block_level "
                f"is set to True; consider setting it to False instead."
            )

    _fields = ('enabled_quote_sections', 'content_is_block_level', )

    def make_body_parser(self, token, nodeargd, arg_parsing_state_delta):

        extend_latex_context = dict(
            macros=[],
            environments=[],
            specials=[]
        )
        for quote_section in self.enabled_quote_sections:
            extend_latex_context['macros'].append( _quote_section_macros[quote_section] )

        # Special case to handle \\ in body content if the auto-quote-section is
        # for a \lines{} section:
        if self.auto_quote_section_bare_content == 'lines':
            for part in ('macros', 'environments', 'specials'):
                if part in _quote_section_lines_environment:
                    extend_latex_context[part].extend(
                        _quote_section_lines_environment[part]
                    )

        return LatexEnvironmentBodyContentsParser(
            environmentname=token.arg,
            contents_parsing_state_delta=ParsingStateDeltaExtendLatexContextDb(
                extend_latex_context=extend_latex_context,
                set_attributes=dict(is_block_level=self.content_is_block_level),
            )
        )

    def postprocess_parsed_node(self, node):
        
        # do not allow any other content than quote section commands in the
        # environment body.

        quote_section_nodes = []

        auto_collecting_qs_nodelist = []

        def flush_auto_collecting_qs():
            if not self.auto_quote_section_bare_content:
                return
            # check that we have a non-empty node list that does not consist of
            # whitespace-only chars:
            has_content = False
            for n in auto_collecting_qs_nodelist:
                if n.isNodeType(latexnodes_nodes.LatexCommentNode):
                    continue
                if n.isNodeType(latexnodes_nodes.LatexCharsNode) and len(n.chars.strip()) == 0:
                    continue
                has_content = True
                break
            if not has_content:
                return
            finalize_and_push_qsn_info(
                self.auto_quote_section_bare_content,
                qsn_node=None,
                text_arg_nodelist=node.latex_walker.make_nodelist(
                    list(auto_collecting_qs_nodelist),
                    parsing_state=node.nodelist.parsing_state,
                ),
            )
            auto_collecting_qs_nodelist.clear()

        def finalize_and_push_qsn_info(name, qsn_node, text_arg_nodelist):
            qsn_info = {
                'name': name,
                'node': qsn_node,
                'text_arg_nodelist': text_arg_nodelist,
            }
            # Special processing for certain section types/names:
            if name == 'lines':
                # split the argument into lines
                text_arg_lines_nodelists = text_arg_nodelist.split_at_node(
                    lambda n: (
                        n.isNodeType(latexnodes_nodes.LatexMacroNode)
                        and n.macroname == '\\'
                    )
                )
                latex_walker = node.latex_walker
                lines_info_list = []
                for line_nodelist in text_arg_lines_nodelists:
                    # process this line (normalize whitespace, find \indent's,
                    # anything else in the future...)
                    lines_info_list.append(
                        quote_lines_process_line_nodelist_to_lineinfo(
                            line_nodelist,
                            latex_walker=latex_walker,
                            parsing_state=line_nodelist.parsing_state,
                        )
                    )

                qsn_info['lines_info_list'] = lines_info_list

            quote_section_nodes.append(qsn_info)

        for n in node.nodelist:
            if n.isNodeType(latexnodes_nodes.LatexMacroNode):
                if n.macroname in self.enabled_quote_sections:
                    # A quote section delimited by an appropriate macro.

                    # in case we're auto-collecting bare content into a
                    # quote-section, flush it:
                    flush_auto_collecting_qs()

                    # Extract the text argument:
                    text_arg_info = ParsedArgumentsInfo(node=n).get_argument_info('text')
                    text_arg_nodelist = text_arg_info.get_content_nodelist()
                    finalize_and_push_qsn_info(
                        n.macroname,
                        qsn_node=n,
                        text_arg_nodelist=text_arg_nodelist,
                    )
                    continue

            if n.isNodeType(latexnodes_nodes.LatexCommentNode):
                # comments are fine
                continue

            if self.auto_quote_section_bare_content:
                # okay, we're collecting any other content into an
                # auto-generated quote-section of the given type
                auto_collecting_qs_nodelist.append(n)
                continue

            if n.isNodeType(latexnodes_nodes.LatexCharsNode) and n.chars.strip() == "":
                # all good, only whitespace
                continue

            raise LatexWalkerLocatedError(
                f"All content in {'{'}{self.environmentname}{'}'} environments must "
                f"be wrapped in an appropriate quote-section command ("
                + ", ".join(["\\"+str(c)+"{}" for c in self.enabled_quote_sections])
                + f"); found content {str(n)}",
                pos=n.pos
            )

        flush_auto_collecting_qs()

        node.flm_quote_section_nodes = quote_section_nodes

        return node

    def render(self, node, render_context):

        fragment_renderer = render_context.fragment_renderer
        
        # ---

        pieces = []

        for qsn in node.flm_quote_section_nodes:
            # render a corresponding block
            name = qsn['name']
            qsn_node = qsn['node']
            text_arg_nodelist = qsn['text_arg_nodelist']
            if name == 'text':
                pieces.append(
                    fragment_renderer.render_semantic_block(
                        fragment_renderer.render_nodelist(text_arg_nodelist, render_context),
                        role='quote-text',
                        render_context=render_context,
                    )
                )
            elif name == 'lines':
                pieces.append(
                    fragment_renderer.render_lines(
                        qsn['lines_info_list'],
                        role='quote-lines',
                        render_context=render_context,
                    )
                )
            elif name == 'attributed':
                pieces.append(
                    fragment_renderer.render_semantic_block(
                        fragment_renderer.render_nodelist(text_arg_nodelist, render_context),
                        role='quote-attributed',
                        render_context=render_context,
                    )
                )
            elif name == 'block':
                pieces.append(
                    fragment_renderer.render_semantic_block(
                        fragment_renderer.render_nodelist(text_arg_nodelist, render_context),
                        role='quote-block',
                        render_context=render_context,
                    )
                )
            else:
                raise ValueError(
                    f"No idea how to render quote-section of type ‘{name}’"
                )


        rendered_contents = fragment_renderer.render_join_blocks(
            pieces, render_context
        )

        return fragment_renderer.render_semantic_block(
            rendered_contents,
            role=self.environmentname,
            render_context=render_context,
            #annotations=[],
        )

    def recompose_pure_latex(self, node, recomposer):

        recopt_quote = recomposer.get_options('quote')
        if recopt_quote.get('keep_as_is', False):
            return False # use default recomposer.

        flm_quote_setup_macro = recopt_quote.get('setup_macro', 'flmQuoteSetup') # or None
        flm_text_macro = recopt_quote.get('text_macro', None)
        flm_lines_macro = recopt_quote.get('lines_macro', None)
        flm_attributed_macro = recopt_quote.get('attributed_macro', 'flmQuoteAttributed')
        flm_block_macro = recopt_quote.get('block_macro', 'flmQuoteBlock')

        s = r'\begin{' + node.environmentname + r'}' + '\n'
        if flm_quote_setup_macro is not None and len(flm_quote_setup_macro):
            s += '\\' + flm_quote_setup_macro + r'{' + node.environmentname + r'}%' + '\n'

        for qsn in node.flm_quote_section_nodes:
            # render a corresponding block
            name = qsn['name']
            qsn_node = qsn['node']
            text_arg_nodelist = qsn['text_arg_nodelist']

            if name == 'lines':
                # special handling of content, use split lines
                content = "\\\\\n".join([
                    recompose_qs_line(lineinfo, node, recomposer)
                    for lineinfo in qsn['lines_info_list']
                ])
            else:
                content = recomposer.recompose_nodelist(text_arg_nodelist, node).strip()

            wrap_macro = None
            if name == 'text':
                if flm_text_macro is not None and len(flm_text_macro):
                    wrap_macro = flm_text_macro
            elif name == 'lines':
                if flm_lines_macro is not None and len(flm_lines_macro):
                    wrap_macro = flm_lines_macro
            elif name == 'attributed':
                if flm_attributed_macro is not None and len(flm_attributed_macro):
                    wrap_macro = flm_attributed_macro
            elif name == 'block':
                if flm_block_macro is not None and len(flm_block_macro):
                    wrap_macro = flm_block_macro
            else:
                raise ValueError(
                    f"No idea how to render quote-section of type ‘{name}’"
                )

            if wrap_macro is not None and len(wrap_macro):
                s += '\\'+wrap_macro+'{'+content+'}'+'\n'
            else:
                s += content + '\n'

        s += r'\end{' + node.environmentname + r'}'
        
        return s



def recompose_qs_line(lineinfo, node, recomposer):
    s = ''
    if lineinfo.indent_left is not None:
        s += r'\indent ' * lineinfo.indent_left
    s += recomposer.recompose_nodelist(lineinfo.nodelist, node)
    return s


def quote_lines_process_line_nodelist_to_lineinfo(nodelist, latex_walker, parsing_state):
    # normalize whitespace on this line --- 

    def mk_line_info(new_node_list, indent_left):
        return LineInfo(
            nodelist=latex_walker.make_nodelist(new_node_list, parsing_state=parsing_state),
            indent_left=indent_left,
        )

    if not nodelist or not len(nodelist):
        return mk_line_info([], None)

    # find any initial \indent macros and we'll set indent_left
    indent_left = None

    jfirst = 0
    for n in nodelist:
        if n.isNodeType(latexnodes_nodes.LatexCommentNode):
            # skip leading comments
            jfirst += 1
            continue

        if n.isNodeType(latexnodes_nodes.LatexCharsNode) and n.chars.strip() == '':
            # skip leading whitespace
            jfirst += 1
            continue

        if n.isNodeType(latexnodes_nodes.LatexMacroNode) and n.macroname == 'indent':
            # add left indent ...
            if indent_left is None:
                indent_left = 0
            indent_left += 1

            # ... and skip macro
            jfirst += 1
            continue

        break

    if jfirst == len(nodelist):
        return mk_line_info([], indent_left)

    linfo_macronames = [ m.macroname for m in _quote_section_lines_environment['macros'] ]
    linfo_specials = [ s.specials_chars for s in _quote_section_lines_environment['specials'] ]

    # check that no node past the first content node (index jfirst) contains
    # line-specific formatting. (\indent or \\) [[FOR LATER: if we add other
    # line-info commands that can appear in the middle of the line, change this
    # here to parse these commands and make sure we process their removal from
    # the final node list.]]
    for n in nodelist[jfirst:]:
        if (n.isNodeType(latexnodes_nodes.LatexMacroNode)
            and n.macroname in linfo_macronames):
            raise LatexWalkerLocatedError(
                f"Macro \\{n.macroname} cannot appear mid-line in \\lines",
                pos=n.pos
            )
        if (n.isNodeType(latexnodes_nodes.LatexSpecialsNode)
            and n.specials_chars in linfo_specials):
            raise LatexWalkerLocatedError(
                f"Specials ‘\\{n.specials_chars}’ cannot appear mid-line in \\lines",
                pos=n.pos
            )

    new_node_list = [
        node for node in nodelist
    ]

    first_node = new_node_list[jfirst]
    if first_node.isNodeType(latexnodes_nodes.LatexCharsNode):
        if first_node.chars[0].isspace():
            # need to "fix" the first node.
            fixed_first_node = latex_walker.make_node(
                latexnodes_nodes.LatexCharsNode,
                chars=first_node.chars.lstrip(),
                pos=first_node.pos,
                pos_end=first_node.pos_end,
                parsing_state=first_node.parsing_state,
            )
            # replace node object in-place in our new list
            new_node_list[jfirst] = fixed_first_node

    # fix last node, if applicable:
    jlast = len(nodelist)-1
    last_node = new_node_list[jlast]
    if ( last_node.isNodeType(latexnodes_nodes.LatexCharsNode)
         and last_node.chars[len(last_node.chars)-1].isspace() ):
        fixed_last_node = latex_walker.make_node(
            latexnodes_nodes.LatexCharsNode,
            chars=last_node.chars.rstrip(),
            pos=last_node.pos,
            pos_end=last_node.pos_end,
            parsing_state=last_node.parsing_state,
        )
        # replace node object in-place in our new list
        new_node_list[jlast] = fixed_last_node

    return mk_line_info(new_node_list[jfirst:], indent_left)
                    


default_quote_environments = {
    'quote': {
        'enabled_quote_sections': ['text', 'lines', 'attributed', 'block'],
        #'auto_quote_section_bare_content': False,
        #'content_is_block_level': True,
    },
    'address': {
        'enabled_quote_sections': [],
        'auto_quote_section_bare_content': 'lines',
        'content_is_block_level': False,
    },
    'blockquote': {
        'enabled_quote_sections': [],
        'auto_quote_section_bare_content': 'block',
    },
}


class FeatureQuote(Feature):

    feature_name = 'quotation'
    feature_title = \
        "Quote other people's words, with attribution"

    feature_flm_doc = r"""
    Provides the \verbcode+\begin{quote} ... \end{quote}+ environment to produce
    block literary quotations.
    """

    # no need for "manager" instances - nothing to keep track of at document
    # processing or rendering time.
    DocumentManager = None
    RenderManager = None

    def __init__(self, quote_environments=None):
        super().__init__()
        if quote_environments is None:
            quote_environments = default_quote_environments
        self.quote_environments = quote_environments


    def _mk_quote_environment_spec(self, qenvname, qenvspec):
        if isinstance(qenvspec, QuoteEnvironment):
            if qenvname != qenvspec.environmentname:
                raise ValueError(
                    f"Misconfigured QuoteEnvironment instance with "
                    f"qenvname != qenvspec.environmentname : "
                    f"{repr(qenvname)} != {repr(qenvspec.environmentname)}"
                )
            return qenvspec
        return QuoteEnvironment(environmentname=qenvname, **qenvspec)

    def add_latex_context_definitions(self):

        environments = [
            self._mk_quote_environment_spec(qenvname, qenvspec)
            for qenvname, qenvspec in self.quote_environments.items()
        ]

        return {
            'environments': environments,
        }




FeatureClass = FeatureQuote
