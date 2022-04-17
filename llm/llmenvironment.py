
from pylatexenc import macrospec


class LLMRenderDispatcher:

    def render(self, node, doc):
        raise RuntimeError("Reimplement me!")






class LLMMacroSpec(macrospec.MacroSpec):
    def __init__(self, macroname, arguments_spec_list=None, rd=None):
        super().__init__(
            macroname,
            arguments_spec_list=arguments_spec_list,
            finalize_node=getattr(rd, 'finalize_parsed_node', None),
        )
        self.rd = rd

class LLMEnvironmentpec(macrospec.EnvironmentSpec):
    def __init__(self, environmentname, arguments_spec_list=None, body_parser=None,
                 rd=None):
        super().__init__(
            environmentname,
            arguments_spec_list=arguments_spec_list,
            body_parser=body_parser,
            finalize_node=getattr(rd, 'finalize_parsed_node', None),
        )
        self.rd = rd

class LLMSpecialsSpec(macrospec.SpecialsSpec):
    def __init__(self, specials_chars, arguments_spec_list=None, rd=None):
        super().__init__(
            environmentname,
            arguments_spec_list=arguments_spec_list,
            body_parser=body_parser,
            finalize_node=getattr(rd, 'finalize_parsed_node', None),
        )
        self.rd = rd




# ------------------------------------------------------------------------------


class Verbatim(LLMRenderDispatcher):
    def __init__(self, annotation=None, include_environment_begin_end=False):
        super().__init__()
        self.annotation = annotation
        self.include_environment_begin_end = include_environment_begin_end

    def render(self, node, doc):

        if node.isNodeType(nodes.LatexEnvironmentNode):
            if self.include_environment_begin_end:
                verbatim_contents = node.latex_verbatim()
            else:
                # it's an environment node, and we only want to render the contents of
                # the environment.
                verbatim_contents = node.nodelist.latex_verbatim()
        else:
            verbatim_contents = node.latex_verbatim()
        
        return doc.render_verbatim( verbatim_contents , annotation=self.annotation )


class TextFormat(LLMRenderDispatcher):
    # any additional 
    def __init__(self, text_format):
        super().__init__()
        self.text_format = text_format

    def render(self, node, doc):

        node_args = doc.get_arguments_nodelists(
            node,
            ('text',) ,
            all=True
        )

        text = doc.render_nodelist( node_args['text'] )

        return doc.render_text_format(self.text_format, text)


class Ref(LLMRenderDispatcher):
    def __init__(self, ref_type):
        super().__init__()
        self.ref_type = ref_type
        
    def render(self, node, doc):

        node_args = doc.get_arguments_nodelists(
            node,
            ('reftarget', 'displaytext'),
            all=True,
            skip_nonexistent=True,
        )

        reftarget_nodelist = node_args['reftarget']
        if not reftarget_nodelist or len(reftarget_nodelist) > 1 \
           or not reftarget_nodelist[1].isNodeType(nodes.LatexCharsNode):
            raise ValueError(
                f"Expected exactly one characters node as reftarget, got {node!r}"
            )

        reftarget = reftarget_nodelist[1].chars

        if 'displaytext' in node_args:
            display_content = doc.render_nodelist( node_args['displaytext'] )
        else:
            display_content = None

        return doc.render_ref(self.ref_type, reftarget, display_content)

class FloatGraphics(LLMRenderDispatcher):
    def __init__(self, float_type='figure', float_caption_name='Figure'):
        super().__init__()
        self.float_type = float_type
        self.float_caption_name = float_caption_name
        
    def finalize_parsed_node(self, node):
        ...

    def render(self, node, doc):
        



class Error(LLMRenderDispatcher):
    def render(self, node, doc):
        raise ValueError(
            f"The node ‘{node}’ cannot be placed here."
        )



# ------------------------------------------------------------------------------


class LLMStandardEnvironment:
    
    .............
