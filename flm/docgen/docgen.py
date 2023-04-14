"""
Generate documentation for a specific set of features and parsing options.
"""

from pylatexenc.latexnodes import ParsedArgumentsInfo
from pylatexenc.latexnodes import nodes as latexnodes_nodes
from pylatexenc.latexnodes import parsers as latexnodes_parsers
from pylatexenc import macrospec

from flm.flmenvironment import FLMArgumentSpec
from flm.flmspecinfo import (
    FLMMacroSpecBase, FLMEnvironmentSpecBase, text_arg, make_verb_argument,
    TextFormatMacro
)
from flm.feature import SimpleLatexDefinitionsFeature
#from flm.feature.verbatim import VerbatimMacro

# ------------------------------------------------------------------------------


class MacroDocArg(FLMMacroSpecBase):

    def get_flm_doc(self):
        return ("Produce documentation of a specific argument of a FLM callable "
                "(macro, environment, specials).  Only use in {flmDocArguments} "
                "environment.")

    def __init__(self, macroname='flmDocArg',):
        super().__init__(
            macroname=macroname,
            arguments_spec_list=[
                FLMArgumentSpec(
                    parser=latexnodes_parsers.LatexDelimitedVerbatimParser(),
                    argname='parser_name',
                    flm_doc=('The name or shorthand notation for how this '
                             'argument is parsed (e.g., mandatory \{...\} '
                             'argument, optional [...] argument, etc.'),
                ),
                FLMArgumentSpec(
                    parser='{',
                    argname='argument_name',
                    flm_doc=('The argument name (argname property of the argument spec).'),
                ),
                FLMArgumentSpec(
                    parser='{',
                    argname='argument_doc',
                    flm_doc=("The documentation of this argument's function and use."),
                ),
            ],
        )
        

    def postprocess_parsed_node(self, node):

        node_args = ParsedArgumentsInfo(node=node).get_all_arguments_info(
            ('parser_name', 'argument_name', 'argument_doc',) ,
        )

        node.flm_doc_parser_name = node_args['parser_name'].get_content_as_chars()
        node.flm_doc_argument_name = node_args['argument_name'].get_content_as_chars()
        node.flm_doc_argument_doc_nodelist = node_args['argument_doc'].get_content_nodelist()

        return node


class EnvironmentDocArguments(FLMEnvironmentSpecBase):

    def get_flm_doc(self):
        return ("Produce documentation of the arguments of a FLM callable "
                "(macro, environment, specials)")

    def postprocess_parsed_node(self, node):
        # find and register child nodes
        node.flm_doc_arguments_nodes = []

        for n in node.nodelist:
            if n.isNodeType(latexnodes_nodes.LatexCommentNode):
                # skip comments
                continue
            if n.isNodeType(latexnodes_nodes.LatexCharsNode) and n.chars.strip() == '':
                # skip whitespace
                continue
            if n.isNodeType(latexnodes_nodes.LatexMacroNode) and n.macroname == 'flmDocArg':
                node.flm_doc_arguments_nodes.append(n)
                continue

            raise ValueError("Invalid contents in {flmDocArguments} environment: " + repr(n))

        return node

    def render(self, node, render_context):

        # items_tags = []
        items_nodelists = []

        for nargdoc in node.flm_doc_arguments_nodes:

            # node_args = ParsedArgumentsInfo(node=nargdoc).get_all_arguments_info(
            #     ('parser_name', 'argument_name', 'argument_doc',) ,
            # )

            # items_tags.append(
            #     node_args['argument_name'].get_content_nodelist(),
            # )

            content = render_context.doc.environment.make_fragment(
                (
                    nargdoc.flm_doc_parser_name
                    + ': '
                    + r'\flmDocArgumentName{'
                    + nargdoc.flm_doc_argument_name
                    + '} — '
                ),
                #
                is_block_level=False,
                what=f"Argument doc leading ...",
            )

            items_nodelists.append(
                node.latex_walker.make_nodelist(
                    [*content.nodes,
                     *nargdoc.flm_doc_argument_doc_nodelist],
                    parsing_state=node.parsing_state
                )
            )
            
        heading_fragment = render_context.doc.environment.make_fragment(
            '\\textbf{Arguments:} ',
            is_block_level=False,
        )

        return render_context.fragment_renderer.render_semantic_block(
            render_context.fragment_renderer.render_join_blocks([
                render_context.fragment_renderer.render_nodelist(
                    heading_fragment.nodes, render_context
                ),
                render_context.fragment_renderer.render_enumeration(
                    iter_items_nodelists=items_nodelists,
                    counter_formatter=lambda n: '•', #lambda n: items_tags[n-1],
                    render_context=render_context,
                )
            ], render_context),
            role='flm_doc_arguments',
            render_context=render_context
        )


class MacroDocText(FLMMacroSpecBase):
    def __init__(self, macroname='flmDocText'):
        super().__init__(
            macroname=macroname,
            arguments_spec_list=[ text_arg ],
        )

    def render(self, node, render_context):

        node_args = ParsedArgumentsInfo(node=node).get_all_arguments_info(
            ('text',) ,
        )

        return render_context.fragment_renderer.render_text_format(
            [ 'flm_doc_text' ],
            node_args['text'].get_content_nodelist(),
            render_context,
        )



class EnvironmentDocBlock(FLMEnvironmentSpecBase):

    def __init__(self, environmentname, thing_format_fn):
        super().__init__(
            environmentname=environmentname,
            arguments_spec_list=[
                FLMArgumentSpec(
                    parser=latexnodes_parsers.LatexDelimitedVerbatimParser(),
                    argname='thing_name',
                    flm_doc=('The name of the thing (feature, macro, environment, specials) '
                             'being documented.'),
                ),
            ]
        )
        self.thing_format_fn = thing_format_fn

    def get_flm_doc(self):
        return ("Produce a block of documentation for a given FLM thing "
                "(feature, macro, environment, specials)")


    def postprocess_parsed_node(self, node):
        # find and register child nodes
        node.flm_doc_arguments_environment = None

        for n in node.nodelist:
            if (n.isNodeType(latexnodes_nodes.LatexEnvironmentNode)
                and n.environmentname == 'flmDocArguments'):
                #
                node.flm_doc_arguments_environment = n

        return node

    def render(self, node, render_context):

        s_items = []

        node_args = ParsedArgumentsInfo(node=node).get_all_arguments_info(
            ('thing_name',) ,
        )
        thing_name = node_args['thing_name'].get_content_as_chars()

        heading_nodelist = render_context.doc.environment.make_fragment(
            (self.thing_format_fn(thing_name, node.flm_doc_arguments_environment)
             if thing_name else ''),
            #
            is_block_level=False,
            what=f"heading for thing ‘{thing_name}’ ...",
        ).nodes

        # s_items.append(render_context.fragment_renderer.render_heading(
        #     heading_nodelist, render_context,
        #     heading_level=self.heading_level
        # ))

        s_items.append(render_context.fragment_renderer.render_nodelist(
            node.latex_walker.make_nodelist([ *heading_nodelist, *node.nodelist ],
                                            parsing_state=node.parsing_state),
            render_context,
            is_block_level=True
        ))

        return render_context.fragment_renderer.render_join_blocks(s_items, render_context)




# ----------------------------


def _render_macro_doc_heading(macroname, flm_doc_arguments_environment):
    return (
        r"\verbcode" + make_verb_argument(macroname)
        + _render_thing_args_prototype(flm_doc_arguments_environment)
        + " —"
    )

def _render_environment_doc_heading(environmentname, flm_doc_arguments_environment):
    return (
        r"\verbcode+\begin{"+environmentname+r"}+"
        + _render_thing_args_prototype(flm_doc_arguments_environment)
        + r"\verbcode+…\end{"+environmentname+r"}+ —"
    )

def _render_specials_doc_heading(specials_chars, flm_doc_arguments_environment):
    return (
        f"\\verbcode" + make_verb_argument(specials_chars)
        + _render_thing_args_prototype(flm_doc_arguments_environment)
        + " —"
    )

def _render_thing_args_prototype(flm_doc_arguments_environment):
    if flm_doc_arguments_environment is None:
        return ''
    s_items = []
    for nargdoc in flm_doc_arguments_environment.flm_doc_arguments_nodes:
        s_items.append(
            r"\textbf{" + nargdoc.flm_doc_parser_name + "}"
        )
    return "".join(s_items)


# ----------------------------

class FeatureFLMDocumentation(SimpleLatexDefinitionsFeature):

    feature_name = 'flm_doc'

    latex_definitions = {
        'macros': [
            MacroDocArg(),
            MacroDocText(),
            TextFormatMacro('flmDocArgumentName', text_formats=['textbf', 'flm_doc_arg_name']),
        ],
        'environments': [
            EnvironmentDocArguments('flmDocArguments'),
            EnvironmentDocBlock('flmDocMacro', thing_format_fn=_render_macro_doc_heading),
            EnvironmentDocBlock('flmDocEnvironment',
                                thing_format_fn=_render_environment_doc_heading),
            EnvironmentDocBlock('flmDocSpecials',
                                thing_format_fn=_render_specials_doc_heading),
        ]
    }
    


# ------------------------------------------------------------------------------



class FLMEnvironmentDocumentationGenerator:

    def document_environment(self, environment):

        s_items = []

        for feature in environment.features:
            s_items.append( self.document_feature(feature) )

        return '\n\n'.join(s_items)


    def document_epilog(self):
        return r"""
\section{Definitions}

\begin{defterm}{delimited content}
  Delimited content can be any FLM content delimited by either one of the
  characters '\{', '[', '(', or '<', up to a matching closing delimiter.
\end{defterm}

\begin{defterm}{verbatim delimited}
  \emph{Verbatim delimited content} is accepted by the \verbcode+\verbcode+
  family of macro in the standard FLM environment.  The first character
  read determines how the argument is delimited.  If the first character is
  one of '\{', '[', '(', or '<', then the verbatim content is read until the
  first matching closing delimiter.  If any other character is encountered,
  the verbatim content is read until the same character is seen again.
\end{defterm}

\begin{defterm}{following macro}
  A special case of an argument is a \emph{following} or \emph{ensuing macro}
  that is placed immediately after the first macro call.  For instance, in the
  standard FLM environment, the code
  \verbcode+\section{My Section Title} \label{sec:xyz}+ is actually parsed
  in a way that \verbcode+\label{...}+ is seen as an optional argument to the
  \verbcode+\section+ macro call.  In this way, the label is immediately
  associated with the section.  It also forbids the label to appear further
  down in the section, which might be confusing to the document author.
\end{defterm}

"""

    def document_feature(self, feature):

        definitions = {
            'macros': [],
            'environments': [],
            'specials': []
        }

        feature_defs = feature.add_latex_context_definitions() or {}
        if feature_defs:
            for k, vlist in feature_defs.items():
                definitions[k].extend( vlist )
        if hasattr(feature, 'add_flm_doc_latex_context_definitions'):
            defs = feature.add_flm_doc_latex_context_definitions() or {}
            for k, vlist in defs.items():
                definitions[k].extend( vlist )

        s = r"\section{" + feature.feature_title + "}\n"

        if hasattr(feature, 'feature_flm_doc'):
            try:
                feature_flm_doc = feature.feature_flm_doc()
            except TypeError:
                feature_flm_doc = feature.feature_flm_doc
            s += feature_flm_doc + "\n"

        if definitions['macros']:
            s += (
                # macros
                r"\subsection{Macros}" + "\n"
                + r"\begin{flmDocItemize}" + "\n"
                + "".join([
                    r"\item " + self.document_callable_specinfo(spec) + "\n"
                for spec in definitions['macros']
                ])
                + r"\end{flmDocItemize}" + "\n"
            )
        if definitions['environments']:
            s += (
                # environments
                r"\subsection{Environments}" + "\n"
                + r"\begin{flmDocItemize}" + "\n"
                + "".join([
                    r"\item " + self.document_callable_specinfo(spec) + "\n"
                    for spec in definitions['environments']
                ])
                + r"\end{flmDocItemize}" + "\n"
            )
        if definitions['specials']:
            s += (
                # specials
                r"\subsection{Specials}" + "\n"
                + r"\begin{flmDocItemize}" + "\n"
                + "".join([
                    r"\item " + self.document_callable_specinfo(spec) + "\n"
                    for spec in definitions['specials']
                ])
                + r"\end{flmDocItemize}" + "\n"
            )

        return s
            

    def document_callable_specinfo(self, specinfo):
        
        arguments_doc = ''
        if specinfo.arguments_spec_list:
            arguments_doc = (
                r"\begin{flmDocArguments}"
                + "".join([
                    self.document_argument(arg)
                    for arg in specinfo.arguments_spec_list
                ])
                + r"\end{flmDocArguments}"
            )

        if isinstance(specinfo, macrospec.MacroSpec):
        
            macroname = specinfo.macroname
            
            flm_doc_text = getattr(specinfo, 'get_flm_doc', lambda: "")()

            verbargument = make_verb_argument('\\'+macroname)

            return (
                r"\begin{flmDocMacro}" + verbargument + "\n"
                + r"\flmDocText{" + flm_doc_text + "}\n"
                + arguments_doc
                + r"\end{flmDocMacro}"
            )

        if isinstance(specinfo, macrospec.EnvironmentSpec):
        
            environmentname = specinfo.environmentname
            
            flm_doc_text = getattr(specinfo, 'get_flm_doc', lambda: "")()

            return (
                r"\begin{flmDocEnvironment}{" + environmentname + "}\n"
                + r"\flmDocText{" + flm_doc_text + "}\n"
                + arguments_doc
                + r"\end{flmDocEnvironment}"
            )

        if isinstance(specinfo, macrospec.SpecialsSpec):
        
            specials_chars = specinfo.specials_chars
            
            flm_doc_text = getattr(specinfo, 'get_flm_doc', lambda: "")()

            show_specials_chars = specials_chars
            show_specials_chars = show_specials_chars.replace('\n', '\\n') # for \par (\n\n)

            verbargument = make_verb_argument(show_specials_chars)

            return (
                r"\begin{flmDocSpecials}" + verbargument + "\n"
                + r"\flmDocText{" + flm_doc_text + "}\n"
                + arguments_doc
                + r"\end{flmDocSpecials}"
            )

        raise ValueError("Unknown specinfo - " + repr(specinfo))

    def document_argument(self, arg):

        parser_name = None

        is_std_delimiter = False
        if arg.parser in ('{', '[', '(', '<'):
            is_std_delimiter = True
        if (isinstance(arg.parser, latexnodes_parsers.LatexDelimitedGroupParser)
            or is_std_delimiter):
            if is_std_delimiter:
                delimiters = arg.parser
            else:
                delimiters = arg.parser.delimiters
            if isinstance(delimiters, str):
                matching_closing = { '{': '}', '[': ']', '(': ')', '<': '>' }
                delimiters = (delimiters, matching_closing.get(delimiters, delimiters))
            if delimiters is None:
                parser_name = r"⟨\term{delimited content}⟩"
            elif delimiters and isinstance(delimiters, tuple):
                parser_name = (
                    r"\verbcode"+make_verb_argument(delimiters[0])
                    + "…"
                    + r"\verbcode"+make_verb_argument(delimiters[1])
                )
            else:
                parser_name = "⟨?⟩"
        elif isinstance(arg.parser, latexnodes_parsers.LatexDelimitedVerbatimParser):
            parser_name = r"⟨\term{verbatim delimited}⟩"
        elif isinstance(arg.parser, latexnodes_parsers.LatexTackOnInformationFieldMacrosParser):
            parser_name = (
                r"⟨\term[following macro]{"
                + "|".join([
                    (r"\verbcode+" + '\\' + mname + '+') for mname in arg.parser.macronames
                ]) + r"}⟩"
            )

        if parser_name is None:
            parser_name = self.argument_parser_names_by_classname.get(
                arg.parser.__class__.__name__,
                None
            )
        if parser_name is None:
            parser_name = repr(arg.parser)

        return (
            r"\flmDocArg"
            + make_verb_argument( parser_name )
            + r"{"
            + (arg.argname or r'(anonymous)')
            + r"}{"
            + (getattr(arg, '_flm_doc', None) or '')
            + r"}"
        )
            
            
