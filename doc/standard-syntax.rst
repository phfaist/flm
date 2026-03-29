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

A backslash followed by a space (``\ ``) produces a regular space (useful after
a macro name).  A tilde ``~`` produces a non-breaking space.  The macro ``\@``
placed immediately after a period that does not end a sentence avoids awkward
inter-sentence spacing (same convention as in LaTeX).  A double newline (blank
line) produces a paragraph break.


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
    The identifier \verba{myVariable} is also verbatim.

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

The ``\verbcode``/``verbatimcode`` and ``\verba`` variants accept an optional
language argument: ``\verbcode[python]{...}``.


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

While the ``\includegraphics`` macro accepts an optional argument, this argument
is currently completely ignored.  You should prepare your figure directly at the
correct size.  Bear in mind that if you have text elements in your figure, then
resizing the figure will cause a visual mismatch with the article text
appearance.

The format to use for figures depends on what destination format you are
targetting.  A good choice is to use SVG figures (for HTML output), PNG or JPEG
figures (both HTML and LaTeX/PDF), and PDF figures (for LaTeX/PDF output).  You
can use the
:py:class:`flm.main.feature_graphics_collection:FeatureGraphicsCollection`
feature (`standard-graphics-collection-feature`_) to automatically convert
between formats while also collecting your source graphics to a common target
location.
  
The size at which you place elements in your SVG file is important.  The stated
physical dimensions that are present in the SVG file are used to place the image
at the correct size and resolution to match the surrounding article.
    
Be sure also to **set the page dimensions of your SVG document correctly** to
match the size of your graphic, with the correct scale for rendering on our web
page.  If you use Inkscape, you can select “File” → “Document Properties” →
“Resize page to drawing or selection” and make sure you have reasonable figure
dimensions.  Take into account the final rendering target to decide your figure
width (e.g., margins in your destination latex template).

If you set the SVG image's dimensions in units of pixels ("px"), we'll assume a
resolution of 96 DPI (as per web standards).  [If you're using Affinity
Designer, set the document resolution to 96 dpi, and uncheck ‘Set viewBox’ in
the SVG export options.]
    
If you use PNG or JPG/JPEG files.  **Please make sure you set the physical
resolution (DPI) of the image correctly**, as this information is honored to
determine the figure size.  Most graphics software will allow you to adjust this
setting (look for "physical dimensions", "dots-per-inch", "pixels-per-inch", or
"DPI").  In [GIMP](https://www.gimp.org/), you can set the resolution with
"Image" -> "Resize" and providing any two of physical dimensions (e.g.,
mm,cm,in,pt), pixel dimensions, and DPI.

See also the `standard-graphics-collection-feature`_ (below).


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

For typesetting tabular content (tables), see “cells” below.


.. _standard-syntax-cells:

Tables and Cells
----------------

*Provided by the* ``cells`` *feature.*

The ``cells`` environment provides a table-like layout:

.. code-block:: latex

    \begin{cells}
      \celldata<H>{
        Header 1 & Header 2
      }
      \celldata{
        Row 1, Col 1 & Row 1, Col 2 \\
        Row 2, Col 1 & Row 2, Col 2
      }
    \end{cells}


The ``{cells}`` environment may contain any number of calls to the
macros ``\celldata`` and ``\cell``.  Each such macro call adds new
cells to the table.  The macros have the following syntax:

- ``\cell{contents}``, ``\cell<style1 style2 ...>{contents}``,
  ``\cell[location]{contents}``,
  ``\cell<styles...>[location]{contents}``: Add a single cell to
  the table.  Unless a location is specified, the cell is added in
  the next column on the current row.
  
  The ``[location]`` can either specify a column ``[COL]`` (will use the current
  row) or a pair ``[ROW;COL]``.  Each of `COL` and `ROW` can be:

  - a row/column number (row and column numbers start at 1);

  - left empty, or set to the character ``.``, to mean the current
    row/column;

  - a ``\merge{INDEXRANGE}`` command to create a merged cell
    spanning multiple rows/columns.  Here, the `INDEXRANGE` can
    be comma-separated numbers (e.g., ``1,2,3`` to include the
    given row/column numbers), including ranges specified as
    ``START-END`` (such as e.g. ``2-4`` for row/column indices 2
    through 4, included) as well as ``START+NUMBER`` to include
    ``NUMBER`` columns starting at index number ``START`` (e.g.,
    ``2+3`` will include column numbers ``2``, ``3`` and ``4``).
    Ranges can be combined, separated by commas, but in any case
    the `INDEXRANGE` must correspond to a contiguous set of
    indices.
    
  The ``<styles...>`` argument specifies the style that will be
  applied to the cell.  See information on cell styles below.  You
  can specify multiple styles by separating them with spaces.
  
  *Example:* ``\cell<H l>[1;\merge{3-4}]{Hi!}`` will create a
  left-aligned (``l``) header (``H``) cell at row ``1`` and
  spanning the columns ``3`` and ``4``, with the contents
  “``Hi!``”.
  
- ``\celldata{tabular data}``, ``\celldata<cellstyles>{tabular
  data}``, ``\celldata[locations]{tabular data}``,
  ``\celldata<cellstyles>[locations]{tabular data}``: A shorthand
  for sequentially adding multiple cells.
  
  The ``{tabular data}`` is specified as you'd do for the LaTeX
  ``\begin{tabular} ... \end{tabular}`` environment, by separating
  rows with ``\\`` and separating columns with ``&``.

  The cells can be placed at given locations, and each cell is
  styled according to the given styles.
  
  The optional ``[locations]`` is of the form of either
  ``[COLUMNS]`` or ``[ROWS;COLUMNS]``.  If `ROWS` are not
  specified, then rows are added sequentially starting from the
  current row.  Each of `ROWS` and `COLUMNS` specify a
  sequence of row/column indices to which the given data will be
  assigned.  For instance, ``\celldata[3,2,1]{A & B & C}`` will
  place ``A`` in the 3rd column, ``B`` in the 2nd column, and
  ``C`` in the 1st column.  More specifically, `ROWS` and
  `COLUMNS` are specified a comma-separated list of indices, of
  ranges of the form ``START-END`` or ``START+NUMBER`` (see
  above), and of ``\merge{}`` commands (see above).  Omitting
  `START` or `END` is interpreted as the current or the last
  index, respectively.  For instance:

  - ``\celldata[2-5]{A & B & C & D\\ E & F & G & H}`` will place
    ``A``, ``B``, ``C``, ``D`` in columns 2, 3, 4, 5 of the first
    row, and place ``E``, ``F``, ``G``, ``H`` in columns 2, 3, 4,
    5 of the following row;

  - ``\celldata[\merge{1,2},\merge{+2};4,6]{A & AA \\ B & BB}``
    will place ``A`` in a merged cell spanning rows ``1,2`` and in
    column ``4``, ``AA`` in a merged call spanning rows ``1,2``
    and in column ``6``, ``B`` in a merged call spanning rows
    ``3,4`` and in column ``4``, and ``BB`` in a merged call
    spanning rows ``3,4`` and in column ``6``.
  
  The ``styles`` is a comma-separated list of styles to apply to
  the different columns.  The same style is applied to all
  provided cells of that column.  The sequence corresponds to the
  given data, not the final column number.  If there are fewer
  style specifications than columns, then the last style is
  repeated as necessary.  For each column style you can specify
  multiple styles by separating them with spaces.  For instance:
  
  - ``\celldata<H>{One & Two & Three}`` Creates three header
    (``H``) cells on a single row.
    
  - ``\celldata<H l, H c>{One & Two & Three}`` Creates one
    left-aligned (``l``) header (``H``) cell with content
    “``One``”, and two centered (``c``) header (``H``) cells with
    respective content “``Two``” and “``Three``”.

  You can also override the style and/or location for individual cells
  in the tabular data by wrapping that cell's content in an explicit
  ``\cell`` call.  Styles are merged with any styles specified in the
  ``\celldata`` call.  For instance, ``\celldata<H>{A & B &
  \cell<green>[\merge{+2}]{C}}`` will generate a row with a header
  cell ``A`` in the first column, a header cell ``B`` in the second
  column, and a green-background header cell ``C`` spanning the third
  and fouth columns.


*Cell styles:*

- ``H`` — this cell is a column header.  It will be typeset in a
  more prominent form (e.g., boldface with a light bottom border);

- ``rH`` — this cell is a row header.  It will be typeset more
  prominently (e.g., boldface);
  
- ``l``, ``c``, ``r`` — align cell contents ``l``\ eft, ``c``\ entered
  or ``r``\ ight;
  
- ``lvert``, ``rvert`` — add a thin vertical border on the left or
  the right side of the cell;
  
- ``green``, ``blue``, ``yellow``, ``red`` — apply a background
  color to the cell to emphasize it in some way.
    

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
proofs.  The default environments include ``theorem``, ``proposition``,
``lemma``, ``corollary``, ``conjecture``, ``definition``, ``remark``, and
``proof``.

.. code-block:: latex

    \begin{theorem}[Euler's theorem]
      \label{thm:euler}
      If \(a\) and \(n\) are coprime, then
      \(a^{\varphi(n)} \equiv 1 \pmod{n}\).
    \end{theorem}

    \begin{proof}
      The proof follows from Lagrange's theorem...
    \end{proof}

You can pin labels to theorem statements using ``\label{thm:XYZ}``.  The
following ref label prefixes are all accepted for theorems: ``thm:``, ``prop:``,
``cor:``, ``lem:``, ``rem:``, ``def:``, ``dfn:``, ``x:``.  You can use any of
these regardless of the theorem type, so you can easily change the type without
updating the label.

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

Use ``\noproofref`` immediately after a theorem's opening to indicate that there
is no corresponding ``proof`` environment for this theorem in the document.


.. _standard-syntax-quotes:

Block Quotes
------------

*Provided by the* ``quotation`` *feature.*

The default quote-type environments are ``quote``, ``blockquote``, and
``address``.

.. code-block:: latex

    \begin{quote}
      \text{To be, or not to be, that is the question.}
      \attributed{William Shakespeare, \emph{Hamlet}}
    \end{quote}

Within the quote environment, you can use:

- ``\text{...}`` for quoted text
- ``\lines{...}`` for text with line breaks (use ``\\`` for line breaks and
  ``\indent`` for indentation)
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



.. _standard-graphics-collection-feature:

Graphics collections — autoconverting formats and collecting
------------------------------------------------------------

*Provided by the* ``flm.main.feature_graphics_collection`` *feature.*

Collect input graphics files into a specific output folder, while applying a
series of custom transformation rules to convert between chosen formats.

TODO: Doc...
