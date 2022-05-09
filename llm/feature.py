

class FeatureDocumentManager:

    def __init__(self, feature, doc, **kwargs):
        super().__init__(**kwargs)
        self.feature_name = feature.feature_name
        self.feature = feature
        self.doc = doc

    def initialize(self):
        pass

    def process(self, fragment_renderer, first_pass_value):
        pass

    def postprocess(self, fragment_renderer, final_value):
        pass



class Feature:

    feature_name = None
    feature_manager_class = FeatureDocumentManager

    def spawn_document_manager(self, doc):
        return self.feature_manager_class(self, doc)

    def add_latex_context_definitions(self):
        return {}
        
