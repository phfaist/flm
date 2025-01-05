
import logging
logger = logging.getLogger(__name__)


from pylatexenc import macrospec
from pylatexenc.latexnodes import parsers as latexnodes_parsers
from pylatexenc.latexnodes import (
    LatexWalkerParseError, LatexWalkerLocatedError,
    ParsedArgumentsInfo, ParsingStateDeltaChained
)

from .flmenvironment import (
    FLMArgumentSpec,
    FLMParsingStateDeltaSetBlockLevel
)


# ------------------------------------------------------------------------------


class FLMSpecInfo(macrospec.CallableSpec):
    r"""
    Class that specifies how to finalize a given parsed node and how to process
    it to render into primitives understood by
    :py:class:`~flm.fragmentrenderer.FragmentRenderer` objects
    """

    delayed_render = False
    r"""
    Whether this node needs to be rendered at the delayed rendering stage, i.e.,
    after a first pass through the document.  This is the case, for instance,
    for ``\ref`` commands etc. for which the entire document needs to have been
    traversed at least once beforehand.  See the delayed render mechanism in the
    documentation for the :py:class:`flmdocument.FLMDocument` class.
    """

    is_block_level = False
    r"""
    If this flag is set to `True`, then elements of this type are always parsed
    as separate block-level elements (e.g., a section heading, an enumeration
    list, etc.)
    """

    is_block_heading = False
    r"""
    If `is_block_level=True` and this flag is also set to `True`, then this
    element *introduces* a new paragraph.  I.e., a block-level/paragraph break
    is introduced immediately before this item.  The present item is itself
    included along with the non-block-level content that follows to form a new
    paragraph.
    """

    is_paragraph_break_marker = False
    r"""
    True if this node's sole purpose is to split paragraphs.  Use this for
    ``\n\n`` breaks or maybe if the user would like to introduce support for
    ``\par`` macros.
    """
    
    allowed_in_standalone_mode = False
    r"""
    Whether or not this node is allowed in *standalone mode*, i.e., whether or
    not this node can be rendered independently of any document object.
    """

    # TODO: --- try something like this ---
    #
    # manages_preceding_whitespace = False
    # manages_succeeding_whitespace = False
    # r"""
    # If either of these are `True`, then whitespace will be trimmed on the
    # corresponding side of this callable invocation, and it is the render
    # function of this object that is responsible for ensuring there is the right
    # amount of whitespace on that side of this macro/env/specials call.
    # """

    body_contents_is_block_level = None
    r"""
    Applicable only to environment specifications.  Specifies whether or
    not the body contents of the body should be parsed as block-level code or
    not.  By default, environment contents are parsed in the same mode as the
    surrounding content.
    """


    # ---------------

    def __init__(self, *, spec_node_parser_type, arguments_spec_list=None, **kwargs):

        # enforce keyword-only arguments at this point to avoid bugs because
        # it's likely the wrong arguments get assigned to the positional
        # arguments to pylatexenc.macrospec.CallableSpec().

        super().__init__(arguments_spec_list=arguments_spec_list,
                         spec_node_parser_type=spec_node_parser_type,
                         **kwargs)

    # ---------------

    def postprocess_parsed_node(self, node):
        r"""
        Can be overridden to add additional information to node objects.

        You shouldn't change the standard node structure (arguments/body
        nodelist/etc), rather, you should add custom properties to store any
        additional information that is relevant to this node.

        You don't have to return the updated node.  The default implementation
        does nothing.
        """
        pass

    def prepare_delayed_render(self, node, render_context):
        r"""
        For items with `delayed_render=True`, this method is called instead of
        render() on the first pass, so that this document item has the
        opportunity to register itself in document feature managers, etc.

        This method is never called if `delayed_render=False`.
        """
        raise RuntimeError("Reimplement me!")

    def render(self, node, render_context):
        r"""
        Produce a final representation of the node, using the given
        `render_context`.
        """
        raise RuntimeError(
            f"Element ‘{node}’ cannot be placed here, render() not reimplemented."
        )


    # ---------------

    # the following method(s) are not meant to be overridden

    def finalize_node(self, node):
        r"""
        Override this method only if you know what you're doing!
        """

        fragment_is_standalone_mode = node.latex_walker.standalone_mode
        if fragment_is_standalone_mode and not self.allowed_in_standalone_mode:
            raise LatexWalkerParseError(
                f"‘{node.latex_verbatim()}’ is not allowed here (standalone mode).",
                pos=node.pos,
            )

        node.flm_specinfo = self
        try:
            self.postprocess_parsed_node(node)

        except LatexWalkerLocatedError as e:
            e.set_pos_or_add_open_context_from_node(node)
            raise e

        except ValueError as e:
            raise LatexWalkerParseError(str(e), pos=node.pos)

        except Exception as e:
            logger.error(
                f"Internal Parse Error! {e}", exc_info=True)
            logger.error(
                f"Happened @{repr(node.latex_walker.pos_to_lineno_colno(node.pos))}, "
                f" node: ‘{node.latex_verbatim()}’"
            )
            raise 

        node.flm_is_block_level = self.is_block_level
        node.flm_is_block_heading = self.is_block_heading
        node.flm_is_paragraph_break_marker = self.is_paragraph_break_marker

        logger.debug("finalize_node(): Finalized node %r.  substitute? %r",
                     node, getattr(node, 'flm_SUBSTITUTE_NODE', None))

        # feature: postprocess_parsed_node() can request the resulting node be
        # substituted by a different node (or node list).  Only use sparingly or
        # it'll cause headaches!  This is used for custom macros, for instance.
        # If that's the case, then return the substituted node and set
        # attributes to keep track of the substitution.
        #
        # In any case, make sure the substitute node was created with
        # latex_walker.make_node() or the like so it has been properly finalized
        # with the relevant FLM-specific attributes.
        if hasattr(node, 'flm_SUBSTITUTE_NODE') and node.flm_SUBSTITUTE_NODE is not None:
            substitute_node = node.flm_SUBSTITUTE_NODE
            substitute_node.flm_SUBSTITUTE_FOR_NODE = node
            logger.debug("finalize_node(): Substituting node %r for %r !",
                         node, substitute_node)
            return substitute_node

        return node
    

    def make_body_parsing_state_delta(self,
                                      token,
                                      nodeargd,
                                      arg_parsing_state_delta,
                                      latex_walker):

        delta_base = super().make_body_parsing_state_delta(
            token,
            nodeargd,
            arg_parsing_state_delta,
            latex_walker
        )

        if self.body_contents_is_block_level is None:
            return delta_base

        delta_block_level = FLMParsingStateDeltaSetBlockLevel(
            is_block_level=self.body_contents_is_block_level
        )

        return ParsingStateDeltaChained([delta_base, delta_block_level])


# ------------------------------------------------------------------------------


class FLMMacroSpecBase(FLMSpecInfo):
    r"""
    Convenience base class for a FLM LaTeX macro specification.
    """
    def __init__(self, macroname, arguments_spec_list=None, **kwargs):
        super().__init__(
            arguments_spec_list=arguments_spec_list,
            spec_node_parser_type=macrospec.LatexMacroCallParser, # or simply 'macro'
            macroname=macroname,
            **kwargs
        )

class FLMEnvironmentSpecBase(FLMSpecInfo):
    r"""
    Convenience base class for a FLM LaTeX environment specification.
    """
    def __init__(self, environmentname, arguments_spec_list=None, **kwargs):
        super().__init__(
            arguments_spec_list=arguments_spec_list,
            spec_node_parser_type=macrospec.LatexEnvironmentCallParser, # or simply 'environment'
            environmentname=environmentname,
            **kwargs
        )

class FLMSpecialsSpecBase(FLMSpecInfo):
    r"""
    Convenience base class for a FLM LaTeX specials specification.
    """
    def __init__(self, specials_chars, arguments_spec_list=None, **kwargs):
        super().__init__(
            arguments_spec_list=arguments_spec_list,
            spec_node_parser_type=macrospec.LatexSpecialsCallParser, # or simply 'specials'
            specials_chars=specials_chars,
            **kwargs
        )




# ------------------------------------------------------------------------------

def make_verb_argument(value):
    delim0 = None
    for delim in ('+', '|', '=', '.', '-', '!', '~', ',', ';', ':'):
        if delim not in value:
            delim0 = delim
            break
    else:
        raise ValueError("Couldn't form literal verbatim command for value %r", value)

    return (delim0 + value + delim0)


class FLMSpecInfoConstantValue(FLMSpecInfo):
    r"""
    Render a constant, literal character string.
    """

    allowed_in_standalone_mode = True

    def get_flm_doc(self):
        s = r'The literal character(s) \verbcode' + make_verb_argument(self.value)
        if len(self.value) == 1:
            s += f' (U+{ord(self.value):04x})'
        return s

    def __init__(self, value, **kwargs):
        super().__init__(**kwargs)
        self.value = value

    def render(self, node, render_context):
        return render_context.fragment_renderer.render_value(self.value, render_context)


class ConstantValueMacro(FLMSpecInfoConstantValue):
    r"""
    LaTeX macro specification for a constant/literal value.
    """
    def __init__(self, macroname, **kwargs):
        super().__init__(macroname=macroname, spec_node_parser_type='macro', **kwargs)

    _fields = ('macroname', 'value',)

class ConstantValueSpecials(FLMSpecInfoConstantValue):
    r"""
    LaTeX speicals specification for the `FLMSpecInfoConstantValue` specinfo.
    """
    def __init__(self, specials_chars, **kwargs):
        super().__init__(specials_chars=specials_chars,
                         spec_node_parser_type='specials', **kwargs)

    _fields = ('specials_chars', 'value',)



text_arg = FLMArgumentSpec(
    parser='{',
    argname='text',
    flm_doc='The text or FLM content to process',
)
r"""
A convenience :py:class:`~flm.flmenvironment.FLMArgumentSpec` instance to
capture an argument to a macro that is meant to contain FLM text content.  The
argument itself is parsed as a single LaTeX expression, i.e., a standard
mandatory argument given in curly braces or a single LaTeX token.
"""

label_arg = FLMArgumentSpec(
    parser=latexnodes_parsers.LatexTackOnInformationFieldMacrosParser(
        ['label'],
        allow_multiple=True
    ),
    argname='label',
    flm_doc=(r'A following \verbcode+\label{…}+ macro attaches a label to '
             r'this macro call')
)
r"""
A convenience :py:class:`~flm.flmenvironment.FLMArgumentSpec` instance to
capture any appended ``\label{}`` command(s).  This can be used, e.g., as the
last "argument" of a sectioning command (``\section{}``), so that the labels are
part of the macro call.  The argument parser is a
:py:class:`pylatexenc.latexnodes.parsers.LatexTackOnInformationFieldMacrosParser`
instance.
"""

def helper_collect_labels(node_arg_label, allowed_prefixes, allow_unknown_macros=False):
    r"""
    Helper function to collect all labels associated with an argument with
    specification :py:data:`label_arg`.

    Doc........................ 
    """

    if not node_arg_label.was_provided():
        return None

    the_labels = []
    argnodes = node_arg_label.get_content_nodelist()
    for argnode in argnodes:
        if argnode.delimiters[0] == r'\label':
            #logger.debug(f"{argnode=}")
            the_label = argnode.nodelist.get_content_as_chars()
            if ':' in the_label:
                ref_type, ref_label = the_label.split(':', 1)
            else:
                ref_type, ref_label = None, the_label

            if ref_type not in allowed_prefixes:
                raise LatexWalkerParseError(
                    f"Argument label ‘{the_label}’ has incorrect prefix "
                    f"‘{ref_type}:’; expected one of {allowed_prefixes}",
                    pos=argnode.pos,
                )

            the_labels.append( (ref_type, ref_label) )
            continue

        if allow_unknown_macros:
            continue

        raise LatexWalkerParseError(
            f"Bad information field macro {argnode.delimiters[0]}",
            pos=argnode.pos
        )
    
    return the_labels





class TextFormatMacro(FLMMacroSpecBase):
    r"""
    The argument `text_formats` is a list of strings, each string is a format
    name to apply.  Format names are inspired from the corresponding canonical
    LaTeX macros that apply them.  They are:

    - `textit`

    - `textbf`

    - (possibly more in the future)
    """

    allowed_in_standalone_mode = True

    # internal, used when truncating fragments to a certain number of characters
    # (see fragment.truncate_to())
    _flm_main_text_argument = 'text'

    def __init__(self, macroname, *, text_formats):
        super().__init__(
            macroname=macroname,
            arguments_spec_list=[text_arg],
        )
        self.text_formats = text_formats

    _fields = ('macroname', 'text_formats',)

    def get_flm_doc(self):
        return (
            r"Formats its argument using the text format(s) "
            + " ".join(f"‘{text_format}’" for text_format in self.text_formats)
        )

    def render(self, node, render_context):

        node_args = ParsedArgumentsInfo(node=node).get_all_arguments_info(
            ('text',) ,
        )

        return render_context.fragment_renderer.render_text_format(
            self.text_formats,
            node_args['text'].get_content_nodelist(),
            render_context,
        )


class SemanticBlockEnvironment(FLMEnvironmentSpecBase):
    r"""
    The argument `role` and `annotations` are passed on to the fragment
    renderer.
    """

    allowed_in_standalone_mode = True

    is_block_level = True

    def __init__(self, environmentname, *, role, annotations=None):
        super().__init__(
            environmentname=environmentname,
        )
        self.role = role
        self.annotations = annotations

    _fields = ('environmentname', 'role', 'annotations',)

    def get_flm_doc(self):
        with_annotations_str = ""
        if self.annotations is not None and len(self.annotations):
            with_annotations_str = (
                " and annotations ["
                + ",".join(["‘"+a+"’" for a in self.annotations]) + "]"
            )
        return (
            r"Formats its contents using a semantic block with role ‘"
            + self.role + r"’" + with_annotations_str
        )

    def render(self, node, render_context):

        content = render_context.fragment_renderer.render_nodelist(
            node.nodelist,
            render_context
        )

        return render_context.fragment_renderer.render_semantic_block(
            content,
            role=self.role,
            render_context=render_context,
            annotations=self.annotations
        )




class FLMSpecInfoParagraphBreak(FLMSpecInfo):

    is_block_level = True

    is_paragraph_break_marker = True

    allowed_in_standalone_mode = True
    
    def render(self, node, render_context):
        raise LatexWalkerParseError('Paragraph break is not allowed here', pos=node.pos)

    def get_flm_doc(self):
        return "Produce a paragraph break to begin a new paragraph"


class ParagraphBreakSpecials(FLMSpecInfoParagraphBreak):
    def __init__(self, specials_chars, **kwargs):
        super().__init__(specials_chars=specials_chars,
                         spec_node_parser_type='specials', **kwargs)

    _fields = ('specials_chars',)

class ParagraphBreakMacro(FLMSpecInfoParagraphBreak):
    def __init__(self, macroname, **kwargs):
        super().__init__(macroname=macroname, spec_node_parser_type='macro', **kwargs)

    _fields = ('macroname',)




class FLMSpecInfoError(FLMSpecInfo):

    allowed_in_standalone_mode = True

    def __init__(self, error_msg=None, **kwargs):
        super().__init__(**kwargs)
        self.error_msg = error_msg
    
    def render(self, node, render_context):
        if self.error_msg:
            msg = self.error_msg
        else:
            msg = f"The node ‘{node.latex_verbatim().strip()}’ cannot be placed here."
            
        logger.error(f"Misplaced node: {repr(node)}")

        raise LatexWalkerParseError(msg, pos=node.pos)


class FLMMacroSpecError(FLMSpecInfoError):
    def __init__(self, macroname, **kwargs):
        super().__init__(macroname=macroname, spec_node_parser_type='macro', **kwargs)

    _fields = ('macroname', 'error_msg', )

class FLMEnvironmentSpecError(FLMSpecInfoError):
    def __init__(self, environmentname, **kwargs):
        super().__init__(environmentname=environmentname,
                         spec_node_parser_type='environment',
                         **kwargs)

    _fields = ('environmentname', 'error_msg', )

class FLMSpecialsSpecError(FLMSpecInfoError):
    def __init__(self, specials_chars, **kwargs):
        super().__init__(specials_chars=specials_chars,
                         spec_node_parser_type='specials',
                         **kwargs)

    _fields = ('specials_chars', 'error_msg', )



