
def flm_text():
    return (r"""
Hello, world. \emph{Here I am.}  What would you like
to    see?

    \begin{itemize}
  \item cool things?
  \item more stuff?
  \item[!!!] Fun stuff??
\end{itemize}

\section{First section}

From there to here, funny things are everywhere.

\begin{defterm}{Pauli matrices}
  The \emph{Pauli matrices} are defined as
  \begin{align}
    \sigma_X = \begin{pmatrix} 0 & 1\\1 & 0\end{pmatrix}\ ;
    \sigma_Y = \begin{pmatrix} 0 & -i\\i & 0\end{pmatrix}\ ;
    \sigma_Z = \begin{pmatrix} 1 & 0\\0 & -1\end{pmatrix}\ .
  \end{align}
\end{defterm}

\begin{defterm}{qu\(d\)it}
  \label{topic:qudit}
  A \emph{qu\(d\)it} is a \(d\)-dimensional quantum system.
\end{defterm}

Here's a display equation:
\begin{align}
  a + b = c\ .
  \label{eq:my-equation}
\end{align}

\subsection{An enumeration list}

\begin{enumerate}[{(a.)}]
\item First thing to see could be something nice.  This line might also be
  pretty long, with lots of unnecessary text that I could have imagined cutting
  off at some point.

\item Second nice thing!  Check also \eqref{eq:my-equation} and \ref{figure:my-figure}.
\end{enumerate}

\paragraph{A paragraph-level sectioning command}
Paragraph content goes here. How does this work?  I can add a
figure, too; let's see how it works.
\begin{figure}
  \includegraphics{https://media.giphy.com/media/8Iv5lqKwKsZ2g/giphy.gif}
  \caption{This figure has a caption.}
  \label{figure:my-figure}
\end{figure}

Don't forget how the \term{Pauli matrices} are defined.  Also, \(\sigma_X\) is a
\hyperref[defterm:{Pauli matrices}]{Pauli matrix} (or a \term[Pauli
matrices]{Pauli matrix}).  Recall the definition of a \term{qu\(d\)it}, noting
that a qubit is also a \term[qu\(d\)it]{qu\(d=2\)it}.

\begin{verbatimtext}   Now \!this%$%^}] <is> some text that
   will be rendered
verbatim.
as \{is.\end{verbatim\ ha no, gotcha parser!
  \end{verbatimtext}

\emph{Vertaim macros like \verbtext+}+ and \verbtext|{| should be
  parsed correctly, even as macro arguments!}

\section{Another section}
\label{sec:test-label-another-sec}

We can try to add some footnotes, too.\footnote{Like this! Does it work?}  Does it work?

See also the topic \ref{topic:qudit}.

""".lstrip()
)


def flm_rendered_html():
    return (r"""
<p>Hello, world. <span class="textit">Here I am.</span> What would you like to see?</p>
<dl class="enumeration itemize"><dt>•</dt><dd><p>cool things?</p></dd><dt>•</dt><dd><p>more stuff?</p></dd><dt>!!!</dt><dd><p>Fun stuff??</p></dd></dl>
<h1 id="sec--First-section" class="heading-level-1">First section</h1>
<p>From there to here, funny things are everywhere.</p>
<div id="defterm-Pauli_20Xmatrices" class="defterm p-block"><p><span class="defterm-term">Pauli matrices: </span>The <span class="textit">Pauli matrices</span> are defined as <span id="equation-1" class="display-math env-align">\begin{align}
    \sigma_X = \begin{pmatrix} 0 &amp; 1\\1 &amp; 0\end{pmatrix}\ ;
    \sigma_Y = \begin{pmatrix} 0 &amp; -i\\i &amp; 0\end{pmatrix}\ ;
    \sigma_Z = \begin{pmatrix} 1 &amp; 0\\0 &amp; -1\end{pmatrix}\ .
  \tag*{(1)}\end{align}</span></p></div>
<div id="defterm-qu_5cX_28Xd_5cX_29Xit" class="defterm p-block"><p><span class="defterm-term">qu<span class="inline-math">\(d\)</span>it: </span>A <span class="textit">qu<span class="inline-math">\(d\)</span>it</span> is a <span class="inline-math">\(d\)</span>-dimensional quantum system.</p></div>
<p>Here&#x27;s a display equation: <span id="equation-2" class="display-math env-align">\begin{align}
  a + b = c\ .
  \label{eq:my-equation}
\tag*{(2)}\end{align}</span></p>
<h2 id="sec--An-enumeration-list" class="heading-level-2">An enumeration list</h2>
<dl class="enumeration enumerate"><dt>(a.)</dt><dd><p>First thing to see could be something nice. This line might also be pretty long, with lots of unnecessary text that I could have imagined cutting off at some point.</p></dd><dt>(b.)</dt><dd><p>Second nice thing! Check also <a href="#equation-2" class="href-ref ref-eq">(2)</a> and <a href="#figure-1" class="href-ref ref-figure">Figure&nbsp;1</a>.</p></dd></dl>
<p><span id="sec--A-paragraph-level-sectionin" class="heading-level-4 heading-inline">A paragraph-level sectioning command</span> Paragraph content goes here. How does this work? I can add a figure, too; let&#x27;s see how it works.</p>
<figure id="figure-1" class="float float-figure"><div class="float-contents"><img src="https://media.giphy.com/media/8Iv5lqKwKsZ2g/giphy.gif"></div>
<figcaption class="float-caption-content"><span><span class="float-number">Figure&nbsp;1</span>: This figure has a caption.</span></figcaption></figure>
<p>Don&#x27;t forget how the <a href="#defterm-Pauli_20Xmatrices" class="href-term">Pauli matrices</a> are defined. Also, <span class="inline-math">\(\sigma_X\)</span> is a <a href="#defterm-Pauli_20Xmatrices" class="href-ref ref-defterm">Pauli matrix</a> (or a <a href="#defterm-Pauli_20Xmatrices" class="href-term">Pauli matrix</a>). Recall the definition of a <a href="#defterm-qu_5cX_28Xd_5cX_29Xit" class="href-term">qu<span class="inline-math">\(d\)</span>it</a>, noting that a qubit is also a <a href="#defterm-qu_5cX_28Xd_5cX_29Xit" class="href-term">qu<span class="inline-math">\(d=2\)</span>it</a>.</p>
<p><span class="verbatimtext verbatimtext-environment">   Now <span>\</span>!this%$%^}] &lt;is&gt; some text that
   will be rendered
verbatim.
as <span>\</span>{is.<span>\</span>end{verbatim<span>\</span> ha no, gotcha parser!
  </span></p>
<p><span class="textit">Vertaim macros like <span class="verbatimtext verbatimtext-inline">}</span> and <span class="verbatimtext verbatimtext-inline">{</span> should be parsed correctly, even as macro arguments!</span></p>
<h1 id="sec-test-label-another-sec" class="heading-level-1">Another section</h1>
<p>We can try to add some footnotes, too.<a href="#footnote-1" class="href-endnote endnote footnote endnote-marks">a</a> Does it work?</p>
<p>See also the topic <a href="#defterm-qu_5cX_28Xd_5cX_29Xit" class="href-ref ref-topic">qu<span class="inline-math">\(d\)</span>it</a>.</p>
<h1 class="heading-level-1">Footnotes</h1>
<dl class="enumeration footnote-list"><dt id="footnote-1">a</dt><dd>Like this! Does it work?</dd></dl>
""".strip()
)




case_info = {
    'source': flm_text,
    'render_to': 'html',
    'render_result': flm_rendered_html,
    'endnotes': True,
    'standard_features': {
        'floats': True,
        'float_types': [
            {
                'float_type': 'figure',
                'float_caption_name': 'Figure',
                'counter_formatter': {
                    'format_num': { 'template': '${arabic}' },
                    'prefix_display': {
                        'singular': 'Figure~',
                        'plural': 'Figures~',
                    },
                    'delimiters': ('',''),
                    'join_spec': 'compact',
                },
                'content_handlers': ['includegraphics'],
            },
            {
                'float_type': 'table',
                'float_caption_name': 'Table',
                'counter_formatter': {
                    'format_num': { 'template': '${arabic}' },
                    'prefix_display': {
                        'singular': 'Table~',
                        'plural': 'Tables~',
                    },
                    'delimiters': ('',''),
                    'join_spec': 'compact',
                },
                'content_handlers': ['cells', 'includegraphics'],
            },
        ],
    },
}
