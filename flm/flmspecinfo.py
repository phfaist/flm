
import logging
logger = logging.getLogger(__name__)


from pylatexenc import macrospec
from pylatexenc.latexnodes import parsers as latexnodes_parsers
from pylatexenc.latexnodes import ParsedArgumentsInfo, LatexWalkerParseError

from .flmenvironment import (
    FLMArgumentSpec
)


# ------------------------------------------------------------------------------


class FLMSpecInfo:
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


    def postprocess_parsed_node(self, node):
        r"""
        Can be overridden to add additional information to node objects.

        You shouldn't change the node object.  You don't have to return it,
        either.
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


    # ---

    # the following method(s) are not meant to be overridden

    def finalize_node(self, node):
        r"""
        Override this method only if you know what you're doing!
        """

        fragment_is_standalone_mode = node.latex_walker.standalone_mode
        if fragment_is_standalone_mode and not self.allowed_in_standalone_mode:
            raise LatexWalkerParseError(
                f"‘{node.latex_verbatim()}’ is not allowed here (standalone mode)."
            )

        node.flm_specinfo = self
        try:
            self.postprocess_parsed_node(node)

        except LatexWalkerParseError as e:
            if not hasattr(e, 'pos') or e.pos is None:
                e.pos = node.pos
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

        # Don't overload the node with properties if they're not needed... 
        #node.flm_strip_preceding_whitespace = False
        
        return node
    



# ------------------------------------------------------------------------------


# transcrypt doesn't seem to like super().__init__() (or the default
# constructor) with multiple inheritance
### BEGINPATCH_MULTIPLE_BASE_CONSTRUCTORS
def _dobaseconstructors2argslast(Me, self, args, kwargs, kwargs_to_1=None):
    super(Me, self).__init__(*args, **kwargs)
### ENDPATCH_MULTIPLE_BASE_CONSTRUCTORS


class FLMMacroSpecBase(FLMSpecInfo, macrospec.MacroSpec):
    def __init__(self, *args, **kwargs):
        _dobaseconstructors2argslast(FLMMacroSpecBase, self, args, kwargs)

class FLMEnvironmentSpecBase(FLMSpecInfo, macrospec.EnvironmentSpec):
    def __init__(self, *args, **kwargs):
        _dobaseconstructors2argslast(FLMEnvironmentSpecBase, self, args, kwargs)

class FLMSpecialsSpecBase(FLMSpecInfo, macrospec.SpecialsSpec):
    def __init__(self, *args, **kwargs):
        _dobaseconstructors2argslast(FLMSpecialsSpecBase, self, args, kwargs)




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

    allowed_in_standalone_mode = True

    def get_flm_doc(self):
        return r'The literal character(s) \verbcode' + make_verb_argument(self.value)

    def __init__(self, *args, value, **kwargs):
        super().__init__(*args, **kwargs)
        self.value = value

    def render(self, node, render_context):
        return render_context.fragment_renderer.render_value(self.value, render_context)


class ConstantValueMacro(FLMSpecInfoConstantValue, macrospec.MacroSpec):
    def __init__(self, *args, **kwargs):
        _dobaseconstructors2argslast(ConstantValueMacro, self, args, kwargs, ('value',))

class ConstantValueSpecials(FLMSpecInfoConstantValue, macrospec.SpecialsSpec):
    def __init__(self, *args, **kwargs):
        _dobaseconstructors2argslast(ConstantValueSpecials, self, args, kwargs, ('value',))


text_arg = FLMArgumentSpec(
    parser='{',
    argname='text',
    flm_doc='The text or FLM content to process',
)

label_arg = FLMArgumentSpec(
    parser=latexnodes_parsers.LatexTackOnInformationFieldMacrosParser(
        ['label'],
        allow_multiple=True
    ),
    argname='label',
    flm_doc=(r'A following \verbcode+\label{…}+ macro attaches a label to '
             r'this macro call')
)

def helper_collect_labels(node_arg_label, allowed_prefixes, allow_unknown_macros=False):

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
                    f"Heading label ‘{the_label}’ has incorrect prefix "
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


class FLMSpecInfoParagraphBreak(FLMSpecInfo):

    is_block_level = True

    is_paragraph_break_marker = True

    allowed_in_standalone_mode = True
    
    def render(self, node, render_context):
        raise LatexWalkerParseError('Paragraph break is not allowed here', pos=node.pos)

    def get_flm_doc(self):
        return "Produce a paragraph break to begin a new paragraph"

class ParagraphBreakSpecials(FLMSpecInfoParagraphBreak, macrospec.SpecialsSpec):
    def __init__(self, *args, **kwargs):
        _dobaseconstructors2argslast(ParagraphBreakSpecials, self, args, kwargs)

class ParagraphBreakMacro(FLMSpecInfoParagraphBreak, macrospec.MacroSpec):
    def __init__(self, *args, **kwargs):
        _dobaseconstructors2argslast(ParagraphBreakMacro, self, args, kwargs)




class FLMSpecInfoError(FLMSpecInfo):

    allowed_in_standalone_mode = True

    def __init__(self, *args, error_msg=None, **kwargs):
        super().__init__(*args, **kwargs)
        self.error_msg = error_msg
    
    def render(self, node, render_context):
        if self.error_msg:
            msg = self.error_msg
        else:
            msg = f"The node ‘{node.latex_verbatim().strip()}’ cannot be placed here."
            
        logger.error(f"Misplaced node: {repr(node)}")

        raise LatexWalkerParseError(msg, pos=node.pos)


class FLMMacroSpecError(FLMSpecInfoError, macrospec.MacroSpec):
    def __init__(self, *args, **kwargs):
        _dobaseconstructors2argslast(FLMMacroSpecError, self, args, kwargs)

class FLMEnvironmentSpecError(FLMSpecInfoError, macrospec.EnvironmentSpec):
    def __init__(self, *args, **kwargs):
        _dobaseconstructors2argslast(FLMEnvironmentSpecError, self, args, kwargs)

class FLMSpecialsSpecError(FLMSpecInfoError, macrospec.SpecialsSpec):
    def __init__(self, *args, **kwargs):
        _dobaseconstructors2argslast(FLMSpecialsSpecError, self, args, kwargs)





