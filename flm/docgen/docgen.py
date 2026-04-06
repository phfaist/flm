"""
Generate documentation for a specific set of features and parsing options.
"""

from pylatexenc.latexnodes import ParsedArgumentsInfo
from pylatexenc.latexnodes import parsers as latexnodes_parsers
from pylatexenc import macrospec

from flm.flmspecinfo import (
    make_verb_argument,
)
from flm.docgen.featuredocgen import FeatureFLMDocumentation


# ------------------------------------------------------------------------------



class FLMEnvironmentDocumentationGenerator:

    include_feature_name = True
    include_feature_dependencies = True

    argument_parser_names_by_classname = {}


    def __init__(self, document_title='FLM Environment Documentation'):
        super().__init__()
        self.document_title = document_title


    def document_environment(self, environment):

        s_items = []

        s_items.append( self.document_environment_introduction(environment) )

        for feature in environment.features:
            s_items.append( self.document_feature(feature) )

        return '\n\n'.join(s_items)

    def document_environment_introduction(self, environment):

        env_section_title = ''
        if self.document_title:
            env_section_title = r"""
\section{""" + self.document_title + r"""}

"""

        env_features = r"""\begin{enumerate}""" + "\n" + "\n".join([
            r"\item " + r"\ref{" + self._get_feature_label_id(feature) + r"}" + "\n"
            #self._get_feature_title(feature) + "\n"
            for feature in environment.features ]) + r"""\end{enumerate}""" + "\n"

        return (env_section_title + env_features + "\n\n")

    def document_epilog(self):
        return r"""
\section{Definitions}

\begin{defterm}{delimited content}
  \term[delimited content]{Delimited content}
  can be any FLM content delimited by either one of the
  characters '\{', '[', '(', or '<', up to a matching closing delimiter.
\end{defterm}

\begin{defterm}{verbatim delimited}
  \term[verbatim delimited]{Verbatim delimited content} is
  accepted by the \verbcode+\verbcode+
  family of macro in the standard FLM environment.  The first character
  read determines how the argument is delimited.  If the first character is
  one of '\{', '[', '(', or '<', then the verbatim content is read until the
  first matching closing delimiter.  If any other character is encountered,
  the verbatim content is read until the same character is seen again.
\end{defterm}

\begin{defterm}{following macro}
  A special case of an argument is a \emph{following} or \emph{ensuing macro}
  that is placed immediately after the first macro call.  For instance, in the
  standard FLM environment, the code
  \verbcode+\section{My Section Title} \label{sec:xyz}+ is actually parsed
  in a way that \verbcode+\label{...}+ is seen as an optional argument to the
  \verbcode+\section+ macro call.  In this way, the label is immediately
  associated with the section.  It also forbids the label to appear further
  down in the section, which might be confusing to the document author.
\end{defterm}

"""

    def _get_feature_title(self, feature):
        feature_title = getattr(feature, 'feature_title', None)
        if feature_title is None:
            feature_title = feature.feature_name.capitalize()
        return feature_title

    def _get_feature_label_id(self, feature):
        return "sec:feature-" + feature.feature_name + '-' + str(id(feature))

    def document_feature(self, feature):

        definitions = {
            'macros': [],
            'environments': [],
            'specials': []
        }

        feature_defs = feature.add_latex_context_definitions() or {}
        if feature_defs:
            for k, vlist in feature_defs.items():
                definitions[k].extend( vlist )
        if hasattr(feature, 'add_flm_doc_latex_context_definitions'):
            defs = feature.add_flm_doc_latex_context_definitions(feature_defs) or {}
            for k, vlist in defs.items():
                definitions[k].extend( vlist )

        feature_title = self._get_feature_title(feature)
        s = r"\section{" + feature_title + "}\n"
        s += r"\label{" + self._get_feature_label_id(feature) + "}\n"

        if self.include_feature_name:
            s += (r"""\textit{These definitions are part of the '\verba+"""
                  + feature.feature_name
                  + r"""+' FLM feature.}""" + "\n\n")

        if hasattr(feature, 'feature_flm_doc'):
            try:
                feature_flm_doc = feature.feature_flm_doc()
            except TypeError:
                feature_flm_doc = feature.feature_flm_doc
            s += feature_flm_doc + "\n"

        if self.include_feature_dependencies:
            if feature.feature_dependencies is not None \
               and len(feature.feature_dependencies):
                s += (
                    r"""\textit{The feature '\verba+""" + feature.feature_name
                    + r"""+' requires the """
                    r"""following feature(s) to be loaded:}"""
                    + ", ".join([ r"'\verba+" + f + "+'"
                                  for f in feature.feature_dependencies ])
                    + ".\n\n"
                )

            if feature.feature_optional_dependencies is not None \
               and len(feature.feature_optional_dependencies):
                s += (
                    r"""\textit{The feature '\verba+""" + feature.feature_name
                    + r"""+' suggests loading the following feature(s) """
                    + r"""for enhanced functionality:}  """
                    + ", ".join([ r"'\verba+" + f + "+'"
                                  for f in feature.feature_optional_dependencies ])
                    + ".\n\n"
                )

        if definitions['macros']:
            s += (
                # macros
                r"\subsection{Macros}" + "\n"
                + r"\begin{flmDocItemizeMacros}" + "\n"
                + "".join([
                    r"\item " + self.document_callable_specinfo(spec) + "\n"
                for spec in definitions['macros']
                ])
                + r"\end{flmDocItemizeMacros}" + "\n"
            )
        if definitions['environments']:
            s += (
                # environments
                r"\subsection{Environments}" + "\n"
                + r"\begin{flmDocItemizeEnvironments}" + "\n"
                + "".join([
                    r"\item " + self.document_callable_specinfo(spec) + "\n"
                    for spec in definitions['environments']
                ])
                + r"\end{flmDocItemizeEnvironments}" + "\n"
            )
        if definitions['specials']:
            s += (
                # specials
                r"\subsection{Specials}" + "\n"
                + r"\begin{flmDocItemizeSpecials}" + "\n"
                + "".join([
                    r"\item " + self.document_callable_specinfo(spec) + "\n"
                    for spec in definitions['specials']
                ])
                + r"\end{flmDocItemizeSpecials}" + "\n"
            )

        return s


    def document_callable_specinfo(self, specinfo):

        arguments_doc = ''
        if specinfo.arguments_spec_list:
            arguments_doc = (
                r"\begin{flmDocArguments}"
                + "".join([
                    self.document_argument(arg)
                    for arg in specinfo.arguments_spec_list
                ])
                + r"\end{flmDocArguments}"
            )

        callable_type = _get_callable_type(specinfo)

        if callable_type == 'macro':

            macroname = specinfo.macroname

            flm_doc_text = (
                getattr(specinfo, 'get_flm_doc')()
                if hasattr(specinfo, 'get_flm_doc')
                else ""
            )

            verbargument = make_verb_argument('\\'+macroname)

            return (
                r"\begin{flmDocMacro}" + verbargument + "\n"
                + r"\begin{flmDocText}" + flm_doc_text + r"\end{flmDocText}" + "\n"
                + arguments_doc
                + r"\end{flmDocMacro}"
            )

        if callable_type == 'environment':

            environmentname = specinfo.environmentname

            flm_doc_text = (
                getattr(specinfo, 'get_flm_doc')()
                if hasattr(specinfo, 'get_flm_doc')
                else ""
            )

            return (
                r"\begin{flmDocEnvironment}{" + environmentname + "}\n"
                + r"\begin{flmDocText}" + flm_doc_text + r"\end{flmDocText}" + "\n"
                + arguments_doc
                + r"\end{flmDocEnvironment}"
            )

        if callable_type == 'specials':

            specials_chars = specinfo.specials_chars

            flm_doc_text = (
                getattr(specinfo, 'get_flm_doc')()
                if hasattr(specinfo, 'get_flm_doc')
                else ""
            )

            show_specials_chars = specials_chars
            show_specials_chars = show_specials_chars.replace('\n', '\\n') # for \par (\n\n)

            verbargument = make_verb_argument(show_specials_chars)

            return (
                r"\begin{flmDocSpecials}" + verbargument + "\n"
                + r"\begin{flmDocText}" + flm_doc_text + r"\end{flmDocText}" + "\n"
                + arguments_doc
                + r"\end{flmDocSpecials}"
            )

        raise ValueError("Unknown specinfo - " + repr(specinfo))

    def _get_delimited_group_doc_name(self, delimiters):
        if isinstance(delimiters, str):
            matching_closing = { '{': '}', '[': ']', '(': ')', '<': '>' }
            delimiters = (delimiters, matching_closing.get(delimiters, delimiters))
        if delimiters is None:
            return r"⟨\term{delimited content}⟩"
        elif delimiters and isinstance(delimiters, tuple):
            return (
                r"\verbcode"+make_verb_argument(delimiters[0])
                + "…"
                + r"\verbcode"+make_verb_argument(delimiters[1])
            )
        return "⟨?⟩"

    def _get_parser_doc_name(self, parser):

        # Handle string argument specs (single-char shortcuts and multi-char
        # specs as supported by LatexStandardArgumentParser).
        if isinstance(parser, str):

            # Mandatory argument in braces: 'm' or '{'
            if parser in ('m', '{'):
                return self._get_delimited_group_doc_name(('{', '}'))

            # Optional argument in square brackets: 'o' or '['
            if parser in ('o', '['):
                return self._get_delimited_group_doc_name(('[', ']'))

            # Single-char delimiters: '(', '<'
            if parser in ('(', '<'):
                return self._get_delimited_group_doc_name(parser)

            # Optional star: 's' or '*'
            if parser in ('s', '*'):
                return r"\verbcode+*+"

            # 'r<char1><char2>' — required delimited argument
            if parser.startswith('r') and len(parser) == 3:
                return self._get_delimited_group_doc_name(
                    (parser[1], parser[2])
                )

            # 'd<char1><char2>' — optional delimited argument
            if parser.startswith('d') and len(parser) == 3:
                return self._get_delimited_group_doc_name(
                    (parser[1], parser[2])
                )

            # 't<char>' — optional token marker
            if parser.startswith('t') and len(parser) == 2:
                return r"\verbcode" + make_verb_argument(parser[1])

            # 'e{<chars>}' — optional embellishment arguments
            if parser.startswith('e'):
                arg_spec_arg = parser[1:].strip()
                if (len(arg_spec_arg) >= 3
                    and arg_spec_arg[0] == '{'
                    and arg_spec_arg[len(arg_spec_arg)-1] == '}'):
                    chars = arg_spec_arg[1:len(arg_spec_arg)-1]
                    return (
                        r"⟨\term[embellishment]{"
                        + " ".join([
                            r"\verbcode" + make_verb_argument(c)
                            for c in chars
                        ])
                        + r"}⟩"
                    )

            # 'v' or 'v<char1><char2>' — verbatim argument
            if parser == 'v' or (parser.startswith('v') and len(parser) == 3):
                return r"⟨\term{verbatim delimited}⟩"

            # 'AnyDelimited' / 'AnyDelimitedOptional'
            if parser in ('AnyDelimited', 'AnyDelimitedOptional'):
                return r"⟨\term{delimited content}⟩"

            return None

        # Handle parser instances

        if isinstance(parser, latexnodes_parsers.LatexDelimitedGroupParser):
            return self._get_delimited_group_doc_name(parser.delimiters)

        if isinstance(parser, latexnodes_parsers.LatexDelimitedVerbatimParser):
            return r"⟨\term{verbatim delimited}⟩"

        if isinstance(parser, latexnodes_parsers.LatexTackOnInformationFieldMacrosParser):
            return (
                r"⟨\term[following macro]{"
                + "|".join([
                    (r"\verbcode+" + '\\' + mname + '+')
                    for mname in parser.macronames
                ]) + r"}⟩"
            )

        if isinstance(parser, latexnodes_parsers.LatexOptionalCharsMarkerParser):
            # Check for embellishment subclass via attribute (not re-exported
            # from the parsers package, so we check the attribute instead).
            if hasattr(parser, 'embellishment_chars'):
                return (
                    r"⟨\term[embellishment]{"
                    + " ".join([
                        r"\verbcode" + make_verb_argument(c)
                        for c in parser.embellishment_chars
                    ])
                    + r"}⟩"
                )
            # General optional chars marker
            return (
                " ".join([
                    r"\verbcode" + make_verb_argument(c)
                    for c in parser.chars_list
                ])
            )

        if isinstance(parser, latexnodes_parsers.LatexDelimitedMultiDelimGroupParser):
            return r"⟨\term{delimited content}⟩"

        if isinstance(parser, latexnodes_parsers.LatexExpressionParser):
            return self._get_delimited_group_doc_name(('{', '}'))

        return None

    def document_argument(self, arg):

        parser = arg.parser

        # If the parser is a LatexStandardArgumentParser, use its arg_spec
        # string to determine the documentation.
        if isinstance(parser, latexnodes_parsers.LatexStandardArgumentParser):
            parser = parser.arg_spec

        parser_name = self._get_parser_doc_name(parser)

        if parser_name is None:
            parser_name = self.argument_parser_names_by_classname.get(
                arg.parser.__class__.__name__,
                None
            )
        if parser_name is None:
            parser_name = repr(arg.parser)

        return (
            r"\flmDocArg"
            + make_verb_argument( parser_name )
            + r"{"
            + (arg.argname or '') #r'(anonymous)')
            + r"}{"
            + (getattr(arg, '_flm_doc', None) or '')
            + r"}"
        )




def _get_callable_type(spec):
    r"""
    Return one of 'macro', 'environment' or 'specials'.
    """
    if spec.spec_node_parser_type is macrospec.LatexMacroCallParser:
        return 'macro'
    if spec.spec_node_parser_type is macrospec.LatexEnvironmentCallParser:
        return 'environment'
    if spec.spec_node_parser_type is macrospec.LatexSpecialsCallParser:
        return 'specials'

    raise ValueError("Unknown spec_node_parser_type: "
                     + repr(spec.spec_node_parser_type))
