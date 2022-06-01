
class Feature:

    feature_name = None

    class DocumentManager:
        def __init__(self, feature, doc, **kwargs):
            super().__init__(**kwargs)
            self.feature = feature
            self.feature_name = self.feature.feature_name
            self.doc = doc
            self.RenderManager = self.feature.RenderManager

        def initialize(self):
            pass

    class RenderManager:
        def __init__(self, feature_document_manager, render_context, **kwargs):
            super().__init__(**kwargs)
            self.feature_document_manager = feature_document_manager
            self.feature = self.feature_document_manager.feature
            self.feature_name = self.feature.feature_name
            self.render_context = render_context

        def initialize(self):
            pass

        def process(self, first_pass_value):
            pass

        def postprocess(self, final_value):
            pass


    def add_latex_context_definitions(self):
        return {}
        
