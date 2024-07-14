"""
Generate documentation for a specific set of features and parsing options.
"""

from pylatexenc.latexnodes import ParsedArgumentsInfo
from pylatexenc.latexnodes import nodes as latexnodes_nodes
from pylatexenc.latexnodes import parsers as latexnodes_parsers
from pylatexenc import macrospec

from flm.flmenvironment import FLMArgumentSpec
from flm.flmspecinfo import (
    FLMMacroSpecBase, FLMEnvironmentSpecBase,
    #text_arg,
    make_verb_argument,
    TextFormatMacro, SemanticBlockEnvironment
)
from flm.feature import SimpleLatexDefinitionsFeature
#from flm.feature.verbatim import VerbatimMacro

# ------------------------------------------------------------------------------


class MacroDocArg(FLMMacroSpecBase):

    def get_flm_doc(self):
        return (r"Produce documentation of a specific argument of a FLM callable "
                r"(macro, environment, specials).  Only use in "
                r"\verbcode+{flmDocArguments}+ environment.")

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

    is_block_level = True

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


        #
        # Prepare the node list that we'll have to render here.  Remember, don't
        # create nodes in render(), because we need stable node_ids for delayed
        # renders (e.g. references to defterms).
        #

        flm_environment = node.latex_walker.flm_environment
        
        # items_tags = []
        items_nodelists = []

        for nargdoc in node.flm_doc_arguments_nodes:

            content = flm_environment.make_fragment(
                (
                    r'\begin{flmDocArgumentListItem}'
                    + r'\flmDocArgumentListItemLabel{'
                    + nargdoc.flm_doc_parser_name
                    + '}'
                    + ': '
                    + r'\flmDocArgumentName{'
                    + nargdoc.flm_doc_argument_name
                    + '}'
                    + r'\end{flmDocArgumentListItem}'
                ),
                #
                is_block_level=True,
                what=f"Argument doc leading ...",
            )

            items_nodelists.append(
                node.latex_walker.make_nodelist(
                    [*content.nodes,
                     *nargdoc.flm_doc_argument_doc_nodelist],
                    parsing_state=node.parsing_state
                )
            )
            
        heading_fragment = flm_environment.make_fragment(
            '\\flmFormatArgumentsCaption{Arguments:} ',
            is_block_level=False,
        )
        
        node.flm_compiled_items_nodelists = items_nodelists
        node.flm_compiled_heading_fragment = heading_fragment

        return node

    def render(self, node, render_context):

        return render_context.fragment_renderer.render_semantic_block(
            render_context.fragment_renderer.render_join_blocks([
                render_context.fragment_renderer.render_nodelist(
                    node.flm_compiled_heading_fragment.nodes, render_context
                ),
                render_context.fragment_renderer.render_enumeration(
                    iter_items_nodelists=node.flm_compiled_items_nodelists,
                    counter_formatter=lambda n: '•', #lambda n: items_tags[n-1],
                    render_context=render_context,
                ),
            ], render_context),
            role='flm_doc_arguments',
            render_context=render_context
        )


class EnvironmentDocText(FLMEnvironmentSpecBase):

    is_block_level = True

    def __init__(self, environmentname='flmDocText'):
        super().__init__(
            environmentname=environmentname,
        )

    def render(self, node, render_context):

        nodelist = node.nodelist

        content = render_context.fragment_renderer.render_nodelist(
            nodelist,
            render_context,
        )
        return render_context.fragment_renderer.render_semantic_block(
            content,
            role='flm_doc_text',
            render_context=render_context,
            annotations=[],
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

        #
        # Prepare the node list that we'll have to render here.  Remember, don't
        # create nodes in render(), because we need stable node_ids for delayed
        # renders (e.g. references to defterms).
        #
        flm_environment = node.latex_walker.flm_environment

        node_args = ParsedArgumentsInfo(node=node).get_all_arguments_info(
            ('thing_name',) ,
        )
        thing_name = node_args['thing_name'].get_content_as_chars()

        thing_fmt_flm = ''
        if thing_name:
            thing_fmt_flm = (
                r"\flmFormatFullThingCall{"
                + self.thing_format_fn(thing_name, node.flm_doc_arguments_environment)
                + r"}"
            )

        heading_nodelist = flm_environment.make_fragment(
            thing_fmt_flm,
            #
            is_block_level=True,
            what=f"heading for thing ‘{thing_name}’ ...",
        ).nodes

        node.flmdoc_compiled_heading_nodelist = heading_nodelist

        return node

    def render(self, node, render_context):

        s_items = []

        heading_nodelist = node.flmdoc_compiled_heading_nodelist

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
        r"\flmFormatThingMacro{"
        + r"\verbcode" + make_verb_argument(macroname)
        + "}"
        + _render_thing_args_prototype(flm_doc_arguments_environment)
        #+ '\n\n'
        #+ " —"
    )

def _render_environment_doc_heading(environmentname, flm_doc_arguments_environment):
    return (
        r"\flmFormatThingEnvironmentBegin{"
        + r"\verbcode+\begin{"+environmentname+r"}+"
        + "}"
        + _render_thing_args_prototype(flm_doc_arguments_environment)
        + '…'
        + r"\flmFormatThingEnvironmentEnd{"
        + r"\verbcode+\end{"+environmentname+r"}+"
        + "}"
        #+ '\n\n'
        #+ " —"
    )

def _render_specials_doc_heading(specials_chars, flm_doc_arguments_environment):
    return (
        r"\flmFormatThingSpecialsChars{"
        + f"\\verbcode" + make_verb_argument(specials_chars)
        + "}"
        + _render_thing_args_prototype(flm_doc_arguments_environment)
        #+ '\n\n'
        #+ " —"
    )

def _render_thing_args_prototype(flm_doc_arguments_environment):
    if flm_doc_arguments_environment is None:
        return ''
    s_items = []
    for nargdoc in flm_doc_arguments_environment.flm_doc_arguments_nodes:
        s_items.append(
            r"\textbf{" + nargdoc.flm_doc_parser_name + "}"
        )
    return r"\flmFormatArgsSignature{" + "".join(s_items) + "}"


# ----------------------------

class FeatureFLMDocumentation(SimpleLatexDefinitionsFeature):

    feature_name = 'flm_doc'

    latex_definitions = {
        'macros': [
            MacroDocArg(),
            #MacroDocText(),
            TextFormatMacro('flmDocArgumentName', text_formats=['textbf', 'flm_doc_arg_name']),
            TextFormatMacro('flmFormatArgsSignature', text_formats=['flm_doc_arg_signature']),
            TextFormatMacro('flmFormatThingMacro',
                            text_formats=['flm_doc_thing', 'flm_doc_thing_macro']),
            TextFormatMacro('flmFormatThingEnvironmentBegin',
                            text_formats=['flm_doc_thing', 'flm_doc_thing_environment_begin']),
            TextFormatMacro('flmFormatThingEnvironmentEnd',
                            text_formats=['flm_doc_thing', 'flm_doc_thing_environment_end']),
            TextFormatMacro('flmFormatThingSpecialsChars',
                            text_formats=['flm_doc_thing', 'flm_doc_thing_specials_chars']),
            TextFormatMacro('flmFormatArgumentsCaption',
                            text_formats=['textbf', 'flm_doc_arguments_caption']),
            TextFormatMacro('flmFormatFullThingCall',
                            text_formats=['flm_doc_full_thing_call'],),
            TextFormatMacro('flmDocArgumentListItemLabel',
                            text_formats=['flm_doc_arg_list_item_label'],),
        ],
        'environments': [
            EnvironmentDocText(),
            EnvironmentDocArguments('flmDocArguments'),
            EnvironmentDocBlock('flmDocMacro', thing_format_fn=_render_macro_doc_heading),
            EnvironmentDocBlock('flmDocEnvironment',
                                thing_format_fn=_render_environment_doc_heading),
            EnvironmentDocBlock('flmDocSpecials',
                                thing_format_fn=_render_specials_doc_heading),
            # SemanticBlockEnvironment('flmFormatFullThingCall',
            #                          role='flm_doc_full_thing_call',)
            SemanticBlockEnvironment('flmDocArgumentListItem',
                                     role='flm_doc_arg_list_item'),
        ]
    }
    


# ------------------------------------------------------------------------------



class FLMEnvironmentDocumentationGenerator:

    include_feature_name = False
    include_feature_dependencies = False

    def document_environment(self, environment):

        s_items = []

        s_items.append( self.document_environment_introduction(environment) )

        for feature in environment.features:
            s_items.append( self.document_feature(feature) )

        return '\n\n'.join(s_items)

    def document_environment_introduction(self, environment):
        env_features = r"""\begin{enumerate}""" + "\n" + "\n".join([
            r"\item " + r"\ref{" + self._get_feature_label_id(feature) + r"}" + "\n"
            #self._get_feature_title(feature) + "\n"
            for feature in environment.features ]) + r"""\end{enumerate}""" + "\n"
        return (r"""
\section{Environment documentation}

""" + env_features + "\n\n")

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

    def _get_feature_title(self, feature):
        feature_title = getattr(feature, 'feature_title', None)
        if feature_title is None:
            feature_title = feature.feature_name.capitalize()
        return feature_title

    def _get_feature_label_id(self, feature):
        return "sec:feature-" + feature.feature_name + '-' + str(id(feature))

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
            defs = feature.add_flm_doc_latex_context_definitions(feature_defs) or {}
            for k, vlist in defs.items():
                definitions[k].extend( vlist )

        feature_title = self._get_feature_title(feature)
        s = r"\section{" + feature_title + "}\n"
        s += r"\label{" + self._get_feature_label_id(feature) + "}\n"
        
        if self.include_feature_name:
            s += (r"""\textit{Documentation of the ‘\verbcode+"""
                  + feature.feature_name
                  + r"""+’ feature.}""" + "\n\n")

        if hasattr(feature, 'feature_flm_doc'):
            try:
                feature_flm_doc = feature.feature_flm_doc()
            except TypeError:
                feature_flm_doc = feature.feature_flm_doc
            s += feature_flm_doc + "\n"

        if self.include_feature_dependencies:
            if feature.feature_dependencies is not None \
               and len(feature.feature_dependencies):
                s += (
                    r"""\textit{This feature requires the following feature(s) to be loaded:}"""
                    + ", ".join([ r"‘\verbcode+" + f + "+’"
                                  for f in feature.feature_dependencies ])
                    + "\n\n"
                )

            if feature.feature_optional_dependencies is not None \
               and len(feature.feature_optional_dependencies):
                s += (
                    r"""\textit{This feature suggests loading the following feature(s) """
                    + r"""for enhanced functionality:}  """
                    + ", ".join([ r"‘\verbcode+" + f + "+’"
                                  for f in feature.feature_optional_dependencies ])
                    + "\n\n"
                )

        if definitions['macros']:
            s += (
                # macros
                r"\subsection{Available Macros}" + "\n"
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
                r"\subsection{Available Environments}" + "\n"
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

        callable_type = _get_callable_type(specinfo)

        if callable_type == 'macro':
        
            macroname = specinfo.macroname
            
            flm_doc_text = (
                getattr(specinfo, 'get_flm_doc')()
                if hasattr(specinfo, 'get_flm_doc')
                else ""
            )

            verbargument = make_verb_argument('\\'+macroname)

            return (
                r"\begin{flmDocMacro}" + verbargument + "\n"
                + r"\begin{flmDocText}" + flm_doc_text + r"\end{flmDocText}" + "\n"
                + arguments_doc
                + r"\end{flmDocMacro}"
            )

        if callable_type == 'environment':
        
            environmentname = specinfo.environmentname
            
            flm_doc_text = (
                getattr(specinfo, 'get_flm_doc')()
                if hasattr(specinfo, 'get_flm_doc')
                else ""
            )

            return (
                r"\begin{flmDocEnvironment}{" + environmentname + "}\n"
                + r"\begin{flmDocText}" + flm_doc_text + r"\end{flmDocText}" + "\n"
                + arguments_doc
                + r"\end{flmDocEnvironment}"
            )

        if callable_type == 'specials':
        
            specials_chars = specinfo.specials_chars
            
            flm_doc_text = (
                getattr(specinfo, 'get_flm_doc')()
                if hasattr(specinfo, 'get_flm_doc')
                else ""
            )

            show_specials_chars = specials_chars
            show_specials_chars = show_specials_chars.replace('\n', '\\n') # for \par (\n\n)

            verbargument = make_verb_argument(show_specials_chars)

            return (
                r"\begin{flmDocSpecials}" + verbargument + "\n"
                + r"\begin{flmDocText}" + flm_doc_text + r"\end{flmDocText}" + "\n"
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
            
            



def _get_callable_type(spec):
    r"""
    Return one of 'macro', 'environment' or 'specials'.
    """
    if spec.spec_node_parser_type is macrospec.LatexMacroCallParser:
        return 'macro'
    if spec.spec_node_parser_type is macrospec.LatexEnvironmentCallParser:
        return 'environment'
    if spec.spec_node_parser_type is macrospec.LatexSpecialsCallParser:
        return 'specials'

    raise ValueError("Unknown spec_node_parser_type: " + repr(self.spec_node_parser_type))

