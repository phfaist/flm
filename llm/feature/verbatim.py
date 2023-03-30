
from ..llmspecinfo import VerbatimEnvironment

from ._base import SimpleLatexDefinitionsFeature



class FeatureVerbatim(SimpleLatexDefinitionsFeature):

    feature_name = 'verbatim'

    latex_definitions = {
        'environments': [
            VerbatimEnvironment(environmentname='verbatimtext'),
        ]
    }


FeatureClass = FeatureVerbatim
