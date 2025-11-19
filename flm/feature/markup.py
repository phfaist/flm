
from ..flmspecinfo import (
    TextFormatMacro, SemanticBlockEnvironment
)

from ._base import Feature

    


class FeatureMarkup(Feature):

    feature_name = 'markup'
    feature_title = \
        'Mark up parts chunks of text to produce custom text formatting or custom environments'

    feature_flm_doc = r"""
    Feature that lets you define custom macros and environments that render as
    custom text formatting macros or custom semantic blocks.
    """

    # no need for "manager" instances - nothing to keep track of at document
    # processing or rendering time.
    DocumentManager = None
    RenderManager = None


    def __init__(self,
                 text_macros=None,
                 semantic_environments=None,
                 ):
        super().__init__()
        self.text_macros = text_macros or {}
        self.semantic_environments = semantic_environments or {}
    

    def add_latex_context_definitions(self):

        macro_specs = []
        for tmname, tmspec in self.text_macros.items():
            macro_specs.append(TextFormatMacro(macroname=tmname, **tmspec))

        environment_specs = []
        for sename, sespec in self.semantic_environments.items():
            environment_specs.append(
                SemanticBlockEnvironment(environmentname=sename, **sespec)
            )

        return {
            'macros': macro_specs,
            'environments': environment_specs,
        }




FeatureClass = FeatureMarkup
