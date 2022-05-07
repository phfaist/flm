
class DocumentFeatureBase:

    feature_name = None

    def initialize(self, doc):
        pass

    def process(self, doc, fragment_renderer, first_pass_value):
        pass

    def postprocess(self, doc, fragment_renderer, final_value):
        pass
