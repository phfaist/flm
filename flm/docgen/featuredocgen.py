"""
Feature and environment definitions for docgen-generated FLM content.
"""

from pylatexenc.latexnodes import ParsedArgumentsInfo
from pylatexenc.latexnodes import nodes as latexnodes_nodes
from pylatexenc.latexnodes import parsers as latexnodes_parsers

from flm.flmenvironment import FLMArgumentSpec
from flm.flmspecinfo import (
    FLMMacroSpecBase, FLMEnvironmentSpecBase,
    make_verb_argument,
    TextFormatMacro, SemanticBlockEnvironment
)
from flm.feature import SimpleLatexDefinitionsFeature


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
                    flm_doc=(r'The name or shorthand notation for how this '
                             r'argument is parsed (e.g., mandatory \{...\} '
                             r'argument, optional [...] argument, etc.'),
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

    is_block_level = True

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
            what=f"heading for thing '{thing_name}' ...",
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





FeatureClass = FeatureFLMDocumentation
