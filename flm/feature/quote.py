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
                extend_latex_context={'macros': [ MacroSpec('\\') ]},
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
            extend_latex_context['macros'].append( MacroSpec('\\') )

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
                node=None,
                text_arg_nodelist=node.latex_walker.make_nodelist(
                    list(auto_collecting_qs_nodelist),
                    parsing_state=node.nodelist.parsing_state,
                ),
            )
            auto_collecting_qs_nodelist.clear()

        def finalize_and_push_qsn_info(name, node, text_arg_nodelist):
            qsn_info = {
                'name': name,
                'node': None,
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
                lines_iter_nodelists = []
                for line_nodelist in text_arg_lines_nodelists:
                    # normalize whitespace on this line --- 
                    lines_iter_nodelists.append(
                        nodelist_strip_surrounding_whitespace( line_nodelist )
                    )
                qsn_info['lines_iter_nodelists'] = lines_iter_nodelists

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
                        node=n,
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
                + f"); found content {str(n)}"
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
                        qsn['lines_iter_nodelists'],
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

        flm_lines_setup_macro = recopt_quote.get('setup_macro', 'flmLinesSetup') # or None
        flm_text_macro = recopt_quote.get('text_macro', None)
        flm_lines_macro = recopt_quote.get('lines_macro', None)
        flm_attributed_macro = recopt_quote.get('attributed_macro', 'flmLinesAttributed')
        flm_block_macro = recopt_quote.get('block_macro', 'flmLinesBlock')

        s = r'\begin{' + node.environmentname + r'}%' + '\n'
        if flm_lines_setup_macro is not None and len(flm_lines_setup_macro):
            s += '\\' + flm_lines_setup_macro + r'{' + node.environmentname + r'}%' + '\n'

        for qsn in node.flm_quote_section_nodes:
            # render a corresponding block
            name = qsn['name']
            qsn_node = qsn['node']
            text_arg_nodelist = qsn['text_arg_nodelist']
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

            if flm_attributed_macro is not None and len(flm_attributed_macro):
                s += '\\'+flm_attributed_macro+'{'+content+'}%'+'\n'
            else:
                s += content

        s += r'\end{' + node.environmentname + r'}'
        
        return s





def nodelist_strip_surrounding_whitespace(nodelist):
    if not nodelist or not len(nodelist):
        return nodelist

    jfirst = 0
    for n in nodelist:
        if n.isNodeType(latexnodes_nodes.LatexCommentNode):
            jfirst += 1
            continue # no need to fix leading comments
        break

    if jfirst == len(nodelist):
        return nodelist

    new_node_list = [
        node for node in nodelist
    ]
    lw = nodelist.latex_walker

    first_node = new_node_list[jfirst]
    if first_node.isNodeType(latexnodes_nodes.LatexCharsNode):
        if first_node.chars[0].isspace():
            # need to "fix" the first node.
            fixed_first_node = lw.make_node(
                latexnodes_nodes.LatexCharsNode,
                chars=first_node.chars.lstrip(),
                pos=first_node.pos,
                pos_end=first_node.pos_end,
                parsing_state=first_node.parsing_state,
            )
            # replace node object in-place in our new list
            new_node_list[jfirst] = fixed_first_node
    elif first_node.pre_space is not None and len(first_node.pre_space):
        d = {}
        for fld in first_node._fields:
            d[fld] = getattr(first_node, fld)
        d['pre_space'] = ''
        fixed_first_node = lw.make_node(
            first_node.__class__,
            **d
        )
        # replace node object in-place in our new list
        new_node_list[jfirst] = fixed_first_node

    # fix last node, if applicable:
    jlast = len(nodelist)-1
    last_node = new_node_list[jlast]
    if ( last_node.isNodeType(latexnodes_nodes.LatexCharsNode)
         and last_node.chars[len(last_node.chars)-1].isspace() ):
        fixed_last_node = lw.make_node(
            latexnodes_nodes.LatexCharsNode,
            chars=last_node.chars.rstrip(),
            pos=last_node.pos,
            pos_end=last_node.pos_end,
            parsing_state=last_node.parsing_state,
        )
        # replace node object in-place in our new list
        new_node_list[jlast] = fixed_last_node

    return lw.make_nodelist(new_node_list, parsing_state=nodelist.parsing_state)
                    


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
