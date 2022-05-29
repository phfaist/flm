


class FeatureFloats(LLMSpecInfo):
    def __init__(self, float_type='figure', float_caption_name='Figure'):
        super().__init__()
        self.float_type = float_type
        self.float_caption_name = float_caption_name
        
    def finalize_parsed_node(self, node):
        ...

    def render(self, node, doc, fragment_renderer):
        
