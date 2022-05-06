
class DocumentFeature:

    feature_name = None

    def initialize(self, doc):
        pass

    def process(self, doc, fragment_renderer):
        raise RuntimeError("reimplement me")
