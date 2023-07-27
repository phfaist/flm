import flm.flmspecinfo
import flm.feature.cells
import flm.feature.cite
import flm.feature.defterm
import flm.feature.endnotes
import flm.feature.enumeration
import flm.feature.floats
import flm.feature.graphics
import flm.feature.headings
import flm.feature.href
import flm.feature.math
import flm.feature.refs
import flm.feature.theorems
import flm.feature.verbatim

#import flm.docgen.docgen

resources = {
    'FLMSpecInfo': {
        # flm.flmspecinfo
        'flm.flmspecinfo:ConstantValueMacro': flm.flmspecinfo.ConstantValueMacro,
        'flm.flmspecinfo:ConstantValueSpecials': flm.flmspecinfo.ConstantValueSpecials,
        'flm.flmspecinfo:TextFormatMacro': flm.flmspecinfo.TextFormatMacro,
        'flm.flmspecinfo:SemanticBlockEnvironment': flm.flmspecinfo.SemanticBlockEnvironment,
        'flm.flmspecinfo:ParagraphBreakSpecials': flm.flmspecinfo.ParagraphBreakSpecials,
        'flm.flmspecinfo:ParagraphBreakMacro': flm.flmspecinfo.ParagraphBreakMacro,
        'flm.flmspecinfo:FLMMacroSpecError': flm.flmspecinfo.FLMMacroSpecError,
        'flm.flmspecinfo:FLMEnvironmentSpecError': flm.flmspecinfo.FLMEnvironmentSpecError,
        'flm.flmspecinfo:FLMSpecialsSpecError': flm.flmspecinfo.FLMSpecialsSpecError,

        # flm.feature.cells
        'flm.feature.cells:LatexTabularRowSeparatorSpec':
            flm.feature.cells.LatexTabularRowSeparatorSpec,
        'flm.feature.cells:LatexTabularColumnSeparatorSpec':
            flm.feature.cells.LatexTabularColumnSeparatorSpec,
        'flm.feature.cells:MergeMacroSpec': flm.feature.cells.MergeMacroSpec,
        'flm.feature.cells:CellMacro': flm.feature.cells.CellMacro,
        'flm.feature.cells:CelldataMacroSpec': flm.feature.cells.CelldataMacroSpec,
        'flm.feature.cells:CellsEnvironment': flm.feature.cells.CellsEnvironment,

        # flm.feature.cite
        'flm.feature.cells:CiteMacro': flm.feature.cells.CiteMacro,

        # flm.feature.defterm
        'flm.feature.defterm:DefineTermEnvironment': flm.feature.defterm.DefineTermEnvironment,
        'flm.feature.defterm:RefTermMacro': flm.feature.defterm.RefTermMacro,

        # flm.feature.endnotes
        'flm.feature.endnotes:EndnoteMacro': flm.feature.endnotes.EndnoteMacro,

        # flm.feature.enumeration
        'flm.feature.enumeration:Enumeration': flm.feature.enumeration.Enumeration,

        # flm.feature.floats
        'flm.feature.floats:FloatEnvironment': flm.feature.floats.FloatEnvironment,

        # flm.feature.graphics
        'flm.feature.graphics:SimpleIncludeGraphicsMacro':
            flm.feature.graphics.SimpleIncludeGraphicsMacro,

        # flm.feature.headings
        'flm.feature.headings:HeadingMacro': flm.feature.headings.HeadingMacro,

        # flm.feature.href
        'flm.feature.href:HrefHyperlinkMacro': flm.feature.href.HrefHyperlinkMacro,

        # flm.feature.math
        'flm.feature.math:MathEnvironment': flm.feature.math.MathEnvironment,
        'flm.feature.math:MathEqrefMacro': flm.feature.math.MathEqrefMacro,

        # flm.feature.refs
        'flm.feature.refs:RefMacro': flm.feature.refs.RefMacro,

        # flm.feature.theorems
        'flm.feature.theorems:TheoremEnvironment': flm.feature.theorems.TheoremEnvironment,

        # flm.feature.verbatim
        'flm.feature.verbatim:VerbatimSpecInfo': flm.feature.verbatim.VerbatimSpecInfo,
        'flm.feature.verbatim:VerbatimMacro': flm.feature.verbatim.VerbatimMacro,
        'flm.feature.verbatim:VerbatimEnvironment': flm.feature.verbatim.VerbatimEnvironment,


        # docgen-related
        # 'flm.docgen.docgen:MacroDocArg': flm.docgen.docgen.MacroDocArg,
        # 'flm.docgen.docgen:EnvironmentDocArguments':
        #     flm.docgen.docgen.EnvironmentDocArguments,
        # 'flm.docgen.docgen:EnvironmentDocText': flm.docgen.docgen.EnvironmentDocText,
        # 'flm.docgen.docgen:EnvironmentDocBlock': flm.docgen.docgen.EnvironmentDocBlock,
    },

    
}
