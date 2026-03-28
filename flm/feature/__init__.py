r"""
FLM feature plugin system.

Features are the primary extension mechanism in FLM.  Each feature defines
a set of macros, environments, and/or specials that can be used in FLM
documents.  Features are pluggable: you can enable, disable, and configure
them independently.

To define a custom feature, subclass :py:class:`Feature` (or
:py:class:`SimpleLatexDefinitionsFeature` for features that only need to
provide LaTeX context definitions without document/render managers).

See :py:func:`~flm.stdfeatures.standard_features` for the default set of
features shipped with FLM.
"""


from ._base import Feature, SimpleLatexDefinitionsFeature
