import re

import logging
logger = logging.getLogger(__name__)

from pylatexenc.latexnodes.nodes import LatexGroupNode

from ._recomposer import FLMNodesFlmRecomposer


# escaping \$ in regex is required for Transcrypt! Otherwise we match newlines ... :/
_default_rx_escape_chars_text = re.compile(r'[\$&#\^_%]')


# We'll need to specialize code for Transcrypt/JS, as we need to use JS
# Map() objects instead of dict() objects because we have keys with
# weird characters.
#
#__pragma__('ecom')
        
_Dict = dict

# only for Transcrypt:
#__pragma__('js', '{}', 'function _JsMapDict_createMap() { return new Map(); }; function _JsMapDict_get(map, k, dflt) { if (map.has(k)) { return map.get(k) } return dflt; };')
"""?
class JsMapDict:
    def __init__(self):
        self.map = _JsMapDict_createMap()
    def get(self, k, dflt=None):
        return _JsMapDict_get(self.map, k, dflt)
    def __setattr__(self, k, v):
        self.map.set(k, v)

_Dict = JsMapDict
?"""



class FLMPureLatexRecomposer(FLMNodesFlmRecomposer):
    r"""
    Doc ................
    """
    
    def __init__(self, options):
        super().__init__()

        if options is None:
            options = {}

        self.options = dict(options)
        self.options_recomposer = dict( options.get('recomposer', {}) )
        self.render_context = options.get('render_context', None)

        self.packages = {}
        self.safe_to_label = {}
        self.label_to_safe = {}
        self.safe_label_counter = 1

        # self.safe_ref_types['ref']['code'] = True to treat 'ref'-domain labels
        # of the form 'code:kxnfjdoisan' as automatically safe
        safe_ref_types = dict( self.options_recomposer.get('safe_label_ref_types', {}) )
        # make sure all objects are turned into dict's (especially, ensure that
        # they are not native JS objects in Transcrypt)
        for ref_domain in list(safe_ref_types.keys()):
            dic = dict(safe_ref_types[ref_domain])
            safe_ref_types[ref_domain] = dic
        self.safe_ref_types = safe_ref_types


    def recompose_pure_latex(self, node):
        latex = self.start(node)
        return {
            "latex": latex,
            "packages": self.packages
        }

    # --

    rx_escape_chars_text = _default_rx_escape_chars_text

    def get_options(self, key):
        return dict(self.options.get(key, {}))

    def ensure_latex_package(self, packagename, options=None):
        if packagename not in self.packages:
            self.packages[packagename] = {
                'options': options,
            }
            return
        if options is None:
            # all good, package is already loaded
            return
        if self.packages[packagename]['options'] is None:
            # all good, simply need to set options
            self.packages[packagename]['options'] = options
        if self.packages[packagename]['options'] == options:
            # all ok, options are the same
            return
        # not good, conflicting options
        raise ValueError(
            f"Conflicting pure latex package options requested for package {packagename} in "
            f"pure latex FLM export: ‘{self.packages[packagename]['options']}’ ≠ ‘{options}’"
        )

    def make_safe_label(self, ref_domain, ref_type, ref_label):
        # ref_domain is like 'ref' or 'cite'

        ref_full_label = f"{ref_type}:{ref_label}"

        if ref_domain in self.safe_ref_types and self.safe_ref_types[ref_domain].get(ref_type):
            # ref is automatically safe, return it as is
            return {'safe_label': ref_full_label}

        if ref_domain not in self.label_to_safe:
            self.label_to_safe[ref_domain] = _Dict()
            self.safe_to_label[ref_domain] = _Dict()

        label_to_safe_map = self.label_to_safe[ref_domain]
        value = label_to_safe_map.get(ref_full_label, None)
        if value is not None:
            # we already have a safe version of this label
            return value

        safe = f"{ref_domain}{str(self.safe_label_counter)}"
        self.safe_label_counter += 1

        sinfo = {'safe_label': safe}

        self.label_to_safe[ref_domain][ref_full_label] = sinfo
        self.safe_to_label[ref_domain][safe] = ref_full_label

        return sinfo


    # --

    # make safer optional argument values:
    def recompose_delimited_nodelist(self, delimiters, recomposed_list, n):
        need_protective_braces = False
        if delimiters[0] == '[' and delimiters[1] == ']':
            if len(n.nodelist) == 1 and n.nodelist[0].isNodeType(LatexGroupNode) \
               and n.nodelist[0].delimiters[0] == '{':
                # all ok, we already have inner protective braces
                need_protective_braces = False
            else:
                need_protective_braces = True
        if need_protective_braces:
            delimiters = ('[{', '}]')
        return super().recompose_delimited_nodelist(delimiters, recomposed_list, n)


    # --

    recompose_specinfo_method = 'recompose_pure_latex'

