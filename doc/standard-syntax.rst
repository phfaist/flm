.. _standard-syntax:

FLM Standard Syntax
===================

This page documents the markup provided by the *standard features* that ship
with FLM.

FLM is a flexible framework for parsing text content as a markup language, using
a LaTeX-inspired syntax.  The actual macros and environments available in
your documents depend entirely on which *features* are loaded and how they are
configured.  Everything described on this page reflects the default set of
features and their default configuration.  You are free to enable, disable, or
reconfigure any of these features, and you can write your own features that
define completely different macro names and syntax.  See :doc:`features` and
:doc:`configuration` for details.

The standard syntax definitions closely mirror similar LaTeX constructs.  Watch
for some subtle differences—for instance, ``\ref`` behaves much more like
``\cref`` from `cleveref` than LaTeX' native ``\ref``.


How FLM Parsing Works
---------------------

FLM uses a LaTeX-like syntax.  The basic building blocks are:

**Macros** --- invoked with a backslash followed by a name and arguments.  Macro
arguments are typically mandatory arguments in curly braces ``{...}`` and/or
optional arguments in square brackets ``[...]``.  (Macros can also be defined in
python code to parse their arguments in more elaborate ways.)

.. code-block:: latex

    \macroname{argument}
    \macroname[optional argument]{mandatory argument}
    \section*{Section title}
    \verbcode+let x : int = 0+

**Environments** --- delimited by ``\begin{name}`` and ``\end{name}``; some
environments accept arguments immediately after ``\begin{environmentname}``:

.. code-block:: latex

    \begin{environmentname}
    ... content ...
    \end{environmentname}

    \begin{enumerate}[(i)]
    \item ... content ...
    \end{enumerate}

**Specials** --- certain characters or character sequences that have special
meaning, such as ``~`` for a non-breaking space or a blank line for a paragraph
break.

**Comments** --- by default, FLM supports comments starting with ``%%`` (two
percent signs) through the end of the line.  A single ``%`` is forbidden in
the input by default.  Use ``\%`` to typeset a percent sign.
This behavior differs from standard LaTeX where a single
``%`` starts a comment.  The double-percent convention avoids accidental
comments (e.g., "up 10% from last year").  This behavior is configurable; see
:ref:`config-parser`.


.. _standard-syntax-formatting:

Text Formatting
---------------

*Provided by the* ``baseformatting`` *feature.*

Emphasis and bold text:

.. code-block:: latex

    \emph{emphasized text}
    \textit{italic text}
    \textbf{bold text}

Special characters are produced by escaping them with a backslash:

.. code-block:: latex

    \%  \#  \&  \$  \{  \}  \textbackslash

A tilde ``~`` produces a non-breaking space.  The macro ``\@`` suppresses
extra space after a period (same as in LaTeX).  A double newline (blank line)
produces a paragraph break.


.. _standard-syntax-headings:

Headings
--------

*Provided by the* ``headings`` *feature.*

.. code-block:: latex

    \section{Section Title}
    \subsection{Subsection Title}
    \subsubsection{Subsubsection Title}
    \paragraph{Paragraph Heading}
    \subparagraph{Subparagraph Heading}
    \subsubparagraph{Subsubparagraph Heading}

Levels 1--3 (``\section`` through ``\subsubsection``) are block-level headings.
Levels 4--6 (``\paragraph`` and below) are inline headings that introduce a
paragraph.

Use the star form to suppress numbering:

.. code-block:: latex

    \section*{Unnumbered Section}

Heading numbering depth, counter formatters, and the mapping of level numbers to
command names are all configurable.


.. _standard-syntax-lists:

Lists
-----

*Provided by the* ``enumeration`` *feature.*

**Numbered lists:**

.. code-block:: latex

    \begin{enumerate}
    \item First item
    \item Second item
    \item Third item
    \end{enumerate}

You can specify a custom counter format with an optional argument:

.. code-block:: latex

    \begin{enumerate}[(a)]
    \item First item
    \item Second item
    \end{enumerate}

**Bulleted lists:**

.. code-block:: latex

    \begin{itemize}
    \item First item
    \item Second item
    \end{itemize}

**Description lists:**

.. code-block:: latex

    \begin{description}
    \item[Term] Description of the term.
    \item[Another term] Another description.
    \end{description}

Lists can be nested.  The enumeration environments and their counter formatters
are configurable.


.. _standard-syntax-math:

Mathematics
-----------

*Provided by the* ``math`` *feature.*

**Inline math** uses ``\(`` and ``\)`` delimiters:

.. code-block:: latex

    The equation \(E = mc^2\) is well known.

.. note::

   By default, the dollar-sign math mode (``$...$``) is disabled to avoid
   accidental math mode in prose.  You can enable it by setting
   ``dollar_inline_math_mode: true`` in the parser configuration.  See
   :ref:`config-parser`.

**Display math** uses equation environments:

.. code-block:: latex

    \begin{equation}
      \label{eq:euler}
      e^{i\pi} + 1 = 0
    \end{equation}

The default equation environments are ``equation``, ``equation*``, ``align``,
``align*``, ``gather``, and ``gather*``.  Starred variants suppress equation
numbering.

**Equation references** use ``\eqref``:

.. code-block:: latex

    See Eq.~\eqref{eq:euler}.


.. _standard-syntax-links:

Hyperlinks
----------

*Provided by the* ``href`` *feature.*

.. code-block:: latex

    \href{https://example.com}{Link text}
    \url{https://example.com}
    \email{user@example.com}


.. _standard-syntax-verbatim:

Verbatim and Code
-----------------

*Provided by the* ``verbatim`` *feature.*

**Inline verbatim:**

.. code-block:: latex

    Use \verbcode{my_function()} in your code.
    Some \verbtext{literal text here}.

**Block verbatim:**

.. code-block:: latex

    \begin{verbatimcode}
    def hello():
        print("Hello, world!")
    \end{verbatimcode}

    \begin{verbatimtext}
    This text is rendered literally,
    preserving   spacing.
    \end{verbatimtext}

The ``\verbcode`` and ``verbatimcode`` variants accept an optional language
argument: ``\verbcode[python]{...}``.


.. _standard-syntax-footnotes:

Footnotes
---------

*Provided by the* ``endnotes`` *feature.*

.. code-block:: latex

    This claim needs a source\footnote{See the original paper for details.}.

Footnotes are collected and rendered at the end of the document under a
"Footnotes" heading.  The counter formatter (alphabetic by default), heading
title, and other options are configurable.

The endnotes feature is general-purpose: it can be configured to provide
multiple categories of endnotes beyond footnotes (e.g., end-of-document
references).


.. _standard-syntax-refs:

Cross-References
----------------

*Provided by the* ``refs`` *feature (and the features that define referenceable items).*

Many constructs (headings, equations, figures, theorems, definition terms) can
be given a label with ``\label``:

.. code-block:: latex

    \section{Introduction}
    \label{sec:intro}

Reference them with ``\ref``:

.. code-block:: latex

    See \ref{sec:intro} for the introduction.  \ref[S]{eq:one} is what we focus
    on now.
    %% [Result:-->]
    %% See Section 1 for the introduction.  Equation (2.4) is what we focus on now.

The ``\ref`` macro **includes the item label (Section, Equation) in the typeset content**,
similarly to `cleveref`'s ``\cref`` or `zref-clever`'s ``\zcref``.  It accepts an optional
argument: use ``\ref[S]{...}`` to typeset a capitalized version of the label suitable for
the beginning of a sentence.  (This syntax is borrowed from `zref-clever`.)

Labels pinned with ``\label{..}`` must have a type prefix that depends on
the kind of item being referenced:

- ``sec:`` for sections (provided by the ``headings`` feature)
- ``eq:`` for equations (provided by the ``math`` feature)
- ``figure:`` for figures, ``table:`` for tables (provided by the ``floats``
  feature)
- ``thm:`` or ``x:`` for theorems (provided by the ``theorems`` feature)
- ``defterm:`` for definition terms (provided by the ``defterm`` feature)
- ``topic:`` for definition terms with the ``topic:`` prefix

Use ``\hyperref`` to provide custom display text for a reference:

.. code-block:: latex

    See \hyperref[sec:intro]{the introduction section}.


.. _standard-syntax-figures:

Figures and Graphics
--------------------

*Provided by the* ``floats`` *and* ``graphics`` *features.*

**Figures:**

.. code-block:: latex

    \begin{figure}
      \includegraphics{my-image}
      \caption{A description of the figure.}
      \label{figure:my-image}
    \end{figure}

The ``\includegraphics`` macro accepts an optional argument for graphics
options such as width:

.. code-block:: latex

    \includegraphics[width=300px]{my-image}

**Tables:**

.. code-block:: latex

    \begin{table}
      \begin{cells}
        \celldata{
          & Name & Value \\
          & Alpha & 1 \\
          & Beta & 2
        }
      \end{cells}
      \caption{A simple table.}
      \label{table:my-table}
    \end{table}

Float types (``figure``, ``table``, etc.), their caption names ("Fig.", "Tab."),
counter formatters, and permitted content handlers are all configurable.

TODO: missing docs: ``\cellstyle``, ``\cell``, optional style arguments to these
commands and ``\celldata``; document styles.


.. _standard-syntax-cells:

Tables and Cells
----------------

*Provided by the* ``cells`` *feature.*

The ``cells`` environment provides a table-like layout:

.. code-block:: latex

    \begin{cells}
      \celldata
        & Header 1 & Header 2 \\
        & Row 1, Col 1 & Row 1, Col 2 \\
        & Row 2, Col 1 & Row 2, Col 2
    \end{cells}

Columns are separated by ``&`` and rows by ``\\``, similar to LaTeX tabular
syntax.  Use ``\cell`` for individual cells and ``\merge`` for merged cells.


.. _standard-syntax-defterm:

Definition Terms
----------------

*Provided by the* ``defterm`` *feature.*

.. code-block:: latex

    \begin{defterm}{quantum error correction}
      \label{defterm:qec}
      Quantum error correction is a set of techniques ...
    \end{defterm}

This defines a term that can be referenced elsewhere:

.. code-block:: latex

    We use \term{quantum error correction} to protect information.

The ``\term`` macro creates a reference to the definition.


.. _standard-syntax-theorems:

Theorems and Proofs
-------------------

*Provided by the* ``theorems`` *feature.*

The theorem feature provides environments for mathematical statements and their
proofs.  The default environments include ``theorem``, ``lemma``,
``proposition``, ``corollary``, ``definition``, ``remark``, and ``proof``.

.. code-block:: latex

    \begin{theorem}[Euler's theorem]
      \label{thm:euler}
      If \(a\) and \(n\) are coprime, then
      \(a^{\varphi(n)} \equiv 1 \pmod{n}\).
    \end{theorem}

    \begin{proof}
      The proof follows from Lagrange's theorem...
    \end{proof}

You can pin labels to theorem statements using ``\label{thm:XYZ}`` (use the
``thm:`` or ``x:`` prefix regardless of the theorem type, so you can easily
change the type without updating the label).

**Proof references.**  The proof environment's optional argument supports a
special syntax for referencing the theorem being proved:

.. code-block:: latex

    %% Displays "Proof (Theorem 3)." with a reference:
    \begin{proof}[*thm:euler]
      ...
    \end{proof}

    %% Displays just "Proof." but records the association:
    \begin{proof}[**thm:euler]
      ...
    \end{proof}

Use ``\begin{proof}[*thm:XYZ]`` to typeset "Proof (Theorem N)" where the
theorem number is automatically resolved from the label.  Use
``\begin{proof}[**thm:XYZ]`` to typeset just "Proof" while still recording the
association between the proof and the theorem.


.. _standard-syntax-quotes:

Block Quotes
------------

*Provided by the* ``quote`` *feature.*

.. code-block:: latex

    \begin{quote}
      \text{To be, or not to be, that is the question.}
      \attributed{William Shakespeare, \emph{Hamlet}}
    \end{quote}

Within the quote environment, you can use:

- ``\text{...}`` for quoted text
- ``\lines{...}`` for text with line breaks (use ``\\`` for line breaks)
- ``\attributed{...}`` for attribution
- ``\block{...}`` for a generic block of content


.. _standard-syntax-substmacros:

Substitution Macros
-------------------

*Provided by the* ``substmacros`` *feature.*

Substitution macros let you define custom macros that expand to FLM content,
similar to ``\newcommand`` in LaTeX.  They are defined in the configuration, not
in the document source.

In your ``flmconfig.yaml``:

.. code-block:: yaml

    flm:
      features:
        substmacros:
          definitions:
            macros:
              # \mymacro expands to "Substitution \emph{code}"
              mymacro:
                content: 'Substitution \emph{code}'

              # \greet[Hello]{World} expands to "Hello, World!"
              greet:
                arguments_spec_list: '[{'
                content: '#1, #2!'
                default_argument_values:
                  1: 'Hello'

              # Named arguments: \greeet[Hi]{Alice}
              greeet:
                arguments_spec_list:
                  - parser: '['
                    argname: 'the_greeting'
                  - parser: '{'
                    argname: 'person_name'
                default_argument_values:
                  the_greeting: 'Hello'
                content: '\textit{#{the_greeting}: #{person_name}}'

              # Different definitions for text mode and math mode:
              Ident:
                content:
                  textmode: null
                  mathmode: '\mathbb{1}'

Arguments are referenced with ``#1``, ``#2``, etc. (by position) or
``#{argname}`` (by name).


.. _standard-syntax-citations:

Citations
---------

*Provided by the* ``citations`` *feature (via the*
`flm-citations <https://github.com/phfaist/flm-citations>`_ *extension package).*

Citations are not included in the core FLM package but are available through the
``flm-citations`` extension.  Install it with::

    pip install flm-citations

Import it in your configuration:

.. code-block:: yaml

    $import:
      - pkg:flm_citations
    bibliography:
      - mybibliography.yaml

Then cite in your document:

.. code-block:: latex

    Kitaev introduced the surface code~\cite{arXiv:quant-ph/9707021}.

Citations are organized by prefix and are automatically retrieved depending on
the type:

- ``arxiv:<arXiv ID>`` --- retrieved from the arXiv API
- ``doi:<DOI>`` --- retrieved from `doi.org <https://doi.org/>`_
- ``manual:{Manual citation text}`` --- uses the given text directly as a
  citation
- ``bib:key`` --- looks up ``key`` in the specified bibliography files
  (CSL-JSON or CSL-YAML format)

See the `flm-citations documentation <https://github.com/phfaist/flm-citations>`_
for configuration details.
