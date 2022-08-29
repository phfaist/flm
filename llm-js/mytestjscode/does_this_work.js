// some custom JS patches are necessary ... comment out these lines to run with python
import * as llmstd from 'llm-js/llm.llmstd.js';
import * as fmthelpers from 'llm-js/llm.fmthelpers.js';

import * as fragmentrenderer_html from 'llm-js/llm.fragmentrenderer.html.js';
const { HtmlFragmentRenderer } = fragmentrenderer_html;
import * as fragmentrenderer_text from 'llm-js/llm.fragmentrenderer.text.js';
const { TextFragmentRenderer } = fragmentrenderer_text;

// some setup code

import {__kwargtrans__, repr} from 'llm-js/org.transcrypt.__runtime__.js';
const $$kw = __kwargtrans__;


const llmtext = `
Here is some text that can contain some simple LaTeX macros, to produce
for instance~\\textbf{bold text} and \\emph{italic text}.

Two line breaks start a new paragraph. You can use inline math like
\\(\\alpha=\\sum_j\\beta_j\\) and display equations like
\\begin{align}
    S_1 &= I\\,X\\,Z\\,Z\\,X\\ ;  \\nonumber\\\\
    S_2, \\ldots, S_4 &= \\text{cyclical permutations of \\(S_1\\)}\\ .
    \\label{eq:stabilizers}
\\end{align}

Refer to equations with~\\eqref{eq:stabilizers}, etc. ...

We can also \\begin{verbatimtext}/insert\ #some *VERBATIM?\\\ntext Right here
    , which
might  span multiple
 lines


and paragraphs. Does it wrk?

\\end{verbatimtext}


Hello, world. \\emph{Here I am.} What would you like
to    see?

    \\begin{itemize}
  \\item cool things?
  \\item more stuff?
  \\item[!!!] Fun stuff??
\\item Do
   \\begin{itemize}
   \\item Nested
   \\item enumeration
     \\begin{enumerate}
     \\item lists
     \\item work?
     \\end{enumerate}
   \\end{itemize}
\\end{itemize}

\\section{First section}

From there to here, funny things are everywhere.

\\begin{defterm}{Pauli matrices}
  The \\emph{Pauli matrices} are defined as
  \\begin{align}
    \\sigma_X = \\begin{pmatrix} 0 & 1\\\\1 & 0\\end{pmatrix}\\ ;
    \\sigma_Y = \\begin{pmatrix} 0 & -i\\\\i & 0\\end{pmatrix}\\ ;
    \\sigma_Z = \\begin{pmatrix} 1 & 0\\\\0 & -1\\end{pmatrix}\\ .
  \\end{align}
\\end{defterm}

\\begin{defterm}{qu\\(d\\)it}
  A \\emph{qu\\(d\\)it} is a \\(d\\)-dimensional quantum system.
\\end{defterm}

Here's a display equation:
\\begin{align}
  a + b = c\\ .
  \\label{eq:my-equation}
\\end{align}

\\subsection{A subsection with an enumeration list}

\\begin{enumerate}[{(a.)}]
\\item First thing to see could be something nice.  This line might also be
  pretty long, with lots of unnecessary text that I could have imagined cutting
  off at some point.

  \\begin{enumerate}
  \\item Nested enumerations
  \\item also
  \\item work?
  \\end{enumerate}

\\item Second nice thing!  Check also \\eqref{eq:my-equation} and \\ref{figure:my-figure}.
\\end{enumerate}

\\paragraph{A paragraph-level sectioning command}
Paragraph content goes here. How does this work?  I can add a
figure, too; let's see how it works.
\\begin{figure}
  \\includegraphics{https://media.giphy.com/media/8Iv5lqKwKsZ2g/giphy.gif}
  \\caption{This figure has a caption.}
  \\label{figure:my-figure}
\\end{figure}

Don't forget how the \\term{Pauli matrices} are defined.  Also, \\(\\sigma_X\\) is a
\\hyperref[defterm:{Pauli matrices}]{Pauli matrix} (or a \\term[Pauli
matrices]{Pauli matrix}).  Recall the definition of a \\term{qu\\(d\\)it}, noting
that a qubit is also a \\term[qu\\(d\\)it]{qu\\(d=2\\)it}.



\\section{Another section}

We can try to add some footnotes, too.\\footnote{Like this! Does it work?}  Does it work?


\\paragraph{Yoo-hoo. Here's a paragraph!}
Here is the content of the paragraph.
\\begin{itemize}
\\item This should NOT generate an error: \\[ x\\ . \\]
\\item There\\footnote{Do} are\\footnote{Re} so\\footnote{Mi} so\\footnote{Fa}
  so\\footnote{Sol} so\\footnote{La} many\\footnote{Si} footnotes!\\footnote{Do}
  And\\footnote{Re} here\\footnote{Mi} are\\footnote{Fa} more!\\footnote{Sol}
\\end{itemize}

`;


const config = {
    footnote_counter_formatter: (n) => '['+fmthelpers.unicodesuperscriptcounter(n)+']'
};

const llmenviron = new llmstd.LLMStandardEnvironment($$kw(config));

//console.log(llmenviron.parsing_state.latex_context);

//console.log(llmenviron.parsing_state.latex_context.get_macro_spec('textbf'));

let llmfragment = null;

try {

    llmfragment = llmenviron.make_fragment(llmtext,
                                           $$kw({ what: 'my test fragment',
                                                  //is_block_level: true
                                                }));

} catch (err) {
    console.error(err.msg);
    console.log(err.stack);
    process.exit();
}

//console.log(llmfragment.nodes);

const doc = llmenviron.make_document( llmfragment.render );

const [result, render_context] = doc.render( HtmlFragmentRenderer() );
//const [result, render_context] = doc.render( TextFragmentRenderer() );

console.log(result);

console.log("\n" + render_context.feature_render_manager('endnotes').render_endnotes(
    $$kw({ include_headings_at_level: 1 })
))
