import re

import logging
logger = logging.getLogger(__name__)

from pylatexenc.latexnodes.nodes import (
    LatexGroupNode,
    LatexCharsNode
)

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

    def make_safe_label(self, ref_domain, ref_type, ref_label, resource_info):
        # ref_domain is like 'ref' or 'cite'

        use_raw = False
        ref_to_global_key = lambda ref_domain, ref_type, ref_label, resource_info: \
            f"{ref_type}:{ref_label}"

        if ref_domain in self.safe_ref_types:
            d = self.safe_ref_types[ref_domain].get(ref_type)
            if not d:
                pass
            elif d is True or d is False:
                use_raw = d
            else:
                # config dictionary
                if 'use_raw' in d:
                    use_raw = d['use_raw']
                if 'ref_to_global_key' in d and d['ref_to_global_key']:
                    ref_to_global_key = d['ref_to_global_key']

        if use_raw:
            # ref is known to already be safe, return it as is
            return {'safe_label': f"{ref_type}:{ref_label}"}

        ref_global_key = ref_to_global_key(
            ref_domain, ref_type, ref_label, resource_info
        )

        if ref_domain not in self.label_to_safe:
            self.label_to_safe[ref_domain] = _Dict()
            self.safe_to_label[ref_domain] = _Dict()

        label_to_safe_map = self.label_to_safe[ref_domain]
        value = label_to_safe_map.get(ref_global_key, None)
        if value is not None:
            # we already have a safe version of this label
            return value

        safe = f"{ref_domain}{str(self.safe_label_counter)}"
        self.safe_label_counter += 1

        sinfo = { 'safe_label': safe }

        self.label_to_safe[ref_domain][ref_global_key] = sinfo
        self.safe_to_label[ref_domain][safe] = {
            "ref_global_key": ref_global_key,
            "ref_type": ref_type,
            "ref_label": ref_label,
            "resource_info": resource_info,
        }

        return sinfo


    # --

    # make safer optional argument values:
    def recompose_delimited_nodelist(self, delimiters, nodelist, n):
        need_protective_braces = False
        if delimiters[0] == '[' and delimiters[1] == ']':
            if len(n.nodelist) == 1 and n.nodelist[0].isNodeType(LatexGroupNode) \
               and n.nodelist[0].delimiters[0] == '{':
                # all ok, we already have inner protective braces
                need_protective_braces = False
            elif len(n.nodelist) == 1 and n.nodelist[0].isNodeType(LatexCharsNode) \
                 and _rx_safe_chars_optarg.match(n.nodelist[0].chars) is not None:
                # all ok, we only have safe chars
                need_protective_braces = False
            else:
                need_protective_braces = True

        if need_protective_braces:
            delimiters = ('[{', '}]')

        return super().recompose_delimited_nodelist(delimiters, nodelist, n)


    # --

    recompose_specinfo_method = 'recompose_pure_latex'




_rx_safe_chars_optarg = re.compile(r'''[-a-zA-Z0-9_+ !@#$&*()<>,./:;"'|]*''')




default_purelatex_defs_makeatletter = r"""

\providecommand\flmRequirePackage[2][]{\usepackage[#1]{#2}}

\flmRequirePackage{verbatim}


\providecommand\flmFinalPreambleSetup{}

% For references -- pin down custom labels wherever we want for \cref
\NewDocumentCommand{\flmL@crefCustomLabel}{O{flmLCustomLabel}m+m}{%
  \begingroup
    \cref@constructprefix{#1}{\cref@result}%
    \protected@edef\@currentlabel{%
      #3}%
    \protected@edef\@currentlabelname{#3}%
    \protected@edef\cref@currentlabel{%
      [#1][][\cref@result]%
      #3%
    }%
    \flmL@cref@label[{#1}]{#2}%
  \endgroup
}
\g@addto@macro\flmFinalPreambleSetup{%
  \ifcsname crefname\noexpand\endcsname
    \crefname{flmLCustomLabel}{}{}%
    \Crefname{flmLCustomLabel}{}{}%
  \fi}
\newcommand\flmLDefLabelText[2]{%
  \begingroup
    \let\flmL@cref@label\label
    \def\label##1{%
      \flmL@crefCustomLabel{##1}{#1}%
    }%
    #2%
  \endgroup
}

% For defterms -- 

\newif\ifdeftermShowTerm
\deftermShowTermfalse
\def\flmL@defterm#1\label#2{%
  \begingroup
  \par\vspace{\abovedisplayskip}%
  \flmDeftermFormat
  \phantomsection
  \label{#2}%
  \edef\flmL@cur@defterm@label{\flmL@cur@defterm@label,#2,}%
  \ifdeftermShowTerm \flmDisplayTerm{#1: }\fi
}
\def\flmL@cur@defterm@label{}
\def\endflmL@defterm{%
  \par
  \vspace{\belowdisplayskip}%
  \endgroup
}
\ifcsname defterm\endcsname \else % there's no \provideenvironment
  \def\defterm#1{\flmL@defterm{#1}}
  \def\enddefterm{\endflmL@defterm}
\fi
\providecommand\flmTerm[4]{%
  \edef\x{\noexpand\in@{,#2,}{\flmL@cur@defterm@label}}%
  \x\ifin@
    \flmTermDisplayInDefterm{#4}%
  \else
    \flmTermDisplayTermRef{#2}{#4}%
  \fi
}
\robustify\flmTerm
\providecommand\flmTermDisplayTermRef[2]{%
  \hyperref[#1]{\flmTermFormat{#2}}%
}
\providecommand\flmTermDisplayInDefterm[1]{%
  \textbf{\textit{#1}}%
}
\providecommand\flmTermFormat[1]{%
  #1%
}
\ifcsname flmFloat\endcsname \else % no \provideenvironment :/
  \def\flmFloat#1#2{%
    \edef\flmFloat@curfloatenv{#1}%
    \edef\flmFloat@usefloatenv{#1}%
    \edef\flmFloat@useenvargs{\csname flmFloatPlacementArgs#2\endcsname}%
    \ifcsname flmFloatSetUseEnv#1\endcsname
      \csname flmFloatSetUseEnv#1\endcsname
    \fi
    \ifcsname flmFloatSetUseConfig#2\endcsname
      \csname flmFloatSetUseConfig#2\endcsname
    \fi
    \edef\x{%
      \noexpand\begin{\flmFloat@usefloatenv}\expandonce\flmFloat@useenvargs}%
    \x
    \centering
  }
  \def\endflmFloat{%
    \expandafter\end\expandafter{\flmFloat@usefloatenv}%
  }
\fi
\providecommand\flmFloatPlacementArgsNumCap{[tbph]}
\providecommand\flmFloatPlacementArgsNumOnly{[tbph]}
\providecommand\flmFloatPlacementArgsCapOnly{[h]}
\providecommand\flmFloatPlacementArgsBare{[h]}
\providecommand\flmFloatSetUseConfigBare{%
  \edef\flmFloat@usefloatenv{center}%
  \def\flmFloat@useenvargs{%
    \edef\@captype{\flmFloat@curfloatenv}%
  }%
}
\providecommand\flmFloatSetUseConfigCapOnly{\flmFloatSetUseConfigBare}



\def\flmLInlineVerbatimEnv#1{%
  \begingroup
  \catcode`\$=12\relax%
  \catcode`\&=12\relax%
  \catcode`\#=12\relax%
  \catcode`\^=12\relax%
  \catcode`\_=12\relax%
  \catcode`\%=12\relax%
  \catcode`\{=12\relax%
  \catcode`\}=12\relax%
  \catcode`\\=12\relax%
  \flmLInlineVerbatimEnv@{#1}%
}
\begingroup
  \catcode`|=0\relax%
  \catcode`[=1\relax%
  \catcode`]=2\relax%
  \catcode`\{=12\relax%
  \catcode`\}=12\relax%
  \catcode`\\=12\relax%
  |long|gdef|flmLInlineVerbatimEnv@#1[%
      |def|flmL@tmp@ReadThisVerbatimEnvContent##1\end{#1}[%
          |endgroup|end[#1]|flmLinlineverbatimenv[#1][##1]%
      ]%
      |flmL@tmp@ReadThisVerbatimEnvContent%
  ]%
|endgroup

\long\def\flmLinlineverbatimenv#1#2{%
  \begingroup
  \obeyspaces
  \obeylines
  \ifcsname #1Format\endcsname
     \csname #1Format\endcsname{\ignorespaces #2\unskip}%
  \else
     \flmInlineVerbatimFormatDefault{\ignorespaces #2\unskip}%
  \fi
  \endgroup
}


\def\flmInlineVerb#1#2{% cf. \verb definition in LaTeX sources latex.ltx
  \relax\ifmmode\hbox\else\leavevmode\null\fi
  \bgroup
    \expandafter\def\expandafter\verb@egroup\expandafter{\verb@egroup #2}%
    \let\do\@makeother \dospecials
    \language\l@nohyphenation
    #1%
    \flmInlineVerb@
}
\def\flmInlineVerb@{%
  \catcode`\{=12\relax \catcode`\}=12\relax
  \expandafter\@ifnextchar\flm@literalBgroup{%
    \flmInlineVerb@WithBgroup
  }{%
    \flmInlineVerb@@
  }%
}
\def\flmInlineVerb@WithBgroup#1{% #1 == '{'
  \@vobeyspaces\frenchspacing
    \expandafter\@sverb\expandafter{\flm@literalEgroup}%
}
\begingroup
  \catcode`\:=12\relax
  \lccode`\:=`\{\relax
  \catcode`\;=12\relax
  \lccode`\;=`\}\relax
  \lowercase{\endgroup\xdef\flm@literalBgroup{:}\xdef\flm@literalEgroup{;}}
\def\flmInlineVerb@ClosingChar#1{%
  \ifcsname flmInlineVerb@ClosingChar@#1\endcsname
    \csname flmInlineVerb@ClosingChar@#1\endcsname
  \else
    #1%
  \fi
}
\def\flmInlineVerb@DefineClosingChar#1#2{%
  \expandafter\edef\csname flmInlineVerb@ClosingChar@#1\endcsname{#2}%
}
\flmInlineVerb@DefineClosingChar{[}{]}
\flmInlineVerb@DefineClosingChar{(}{)}
\flmInlineVerb@DefineClosingChar{<}{>}
\flmInlineVerb@DefineClosingChar{|}{|}
\def\flmInlineVerb@@#1{%
  \edef\x{\flmInlineVerb@ClosingChar#1}%
  \expandafter\@sverb\expandafter{\x}%
}



% block environments (verbatimcode) are defined using the {verbatim} package.
% inline environments (verbatimtext) use our custom flmLInlineVerbatimEnv above.
%
% Note that we cannot use \verb from latex directly because \verb doesn't understand
% braced arguments like \verb{abc}.


\ifcsname verbatimcode\endcsname\else
  \newenvironment{verbatimcode}{%
    \begingroup
    \parskip=\z@\relax
    \ttfamily
    \verbatim
  }{%
    \endverbatim
    \endgroup
  }
\fi

\ifcsname verbatimtext\endcsname\else
  \newenvironment{verbatimtext}{\flmLInlineVerbatimEnv{verbatimtext}}{}
\fi

\providecommand\flmInlineVerbatimFormatDefault[1]{#1}


\def\verba{\flmInlineVerb{\itshape}{}}
\def\verbtext{\flmInlineVerb{}{}}
\def\verbcode{\flmInlineVerb{\ttfamily}{}}


"""
