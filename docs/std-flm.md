# std flm

*std flm* is the name of a common baseline of the FLM language: the constructs (commands,
environments and special characters) that an FLM document may use, with the meaning that
the default settings of the language give them. When the writer of a document tells a
reader "this is a std flm document", the reader knows which constructs the document may
contain and what they mean, even if the writer and the reader share nothing else. In
particular, the reader does not need to know which program the writer used, or how that
program was set up.

A baseline is needed because FLM can be configured. A program that processes FLM documents
chooses the commands and environments it accepts and the settings that decide how the
source is read. For instance, a program set up for a blog might add commands of its own,
leave out others, and read `$…$` as mathematics. A document written for that program means
something else to a program set up otherwise. The name std flm refers to one common
choice of these settings, whatever the program that reads the document.

A *std flm document* is FLM content that uses only the constructs of std flm, possibly
together with its *metadata*: the title, the authors and the date of the document (see
[the metadata](#the-metadata) below). The name refers to this content and this
metadata, and not to the form in which they are stored in a file.

This page sets expectations; it is not a full specification of the language. For
instance, a std flm document that contains mathematics is expected to be processed
without errors by a program that uses the default settings of the language and a
reasonable processor for mathematics. The page describes documents and makes no promise
about any program. The chapter [FLM language syntax](crate::guide::language_syntax)
describes every construct in full.

## How the content is read

The source of a std flm document is text, read with the default parsing settings of the
language ([`FlmParsing`](crate::lang::FlmParsing)):

- A comment starts with `%%` and runs to the end of the line. The percent sign itself is
  written `\%`.
- Mathematics is written between `\(` and `\)` (inline mathematics) or between `\[` and
  `\]` (displayed mathematics). The character `$` does not delimit mathematics; the dollar
  sign itself is written `\$`.

```text
The price drops by 10\% to \$5. %% a comment, up to the end of the line
The area of a disk is \(\pi r^2\).
```

## What an author can write

A std flm document may use the constructs listed below, with the meaning that the default
settings of the language give them. The constructs are grouped by the *feature* of this
library in which they are defined and documented: a feature is a named group of related
definitions, such as the commands that write headings. Each entry links to the
documentation of its feature.

Some constructs may be written only in certain places. For instance, `\item` may be
written only in the body of a list. A *label*, written `\label{type:label}`, may be
written only where a construct accepts it, as each entry below states. The *label type*,
the part before the colon, states what sort of construct is labeled, and each construct
accepts only certain label types.

### Text

- **Paragraphs and typography** ([`blocks`](crate::stdlib::features::blocks)). A blank
  line separates paragraphs. A *block* is a construct that stands between paragraphs,
  such as a heading, a list or a figure. Some spellings of the source stand for
  typographic characters: `--` for an en dash, `---` for an em dash, `...` for an
  ellipsis, two backticks for an opening curly double quote, `''` for a closing one, and
  one backtick for an opening single quote. The straight quotes `"` and `'` stand for an
  opening or a closing curly quote, depending on their position; for instance, the `'`
  of `don't` stands for the apostrophe `’`. No command is defined in this feature.
- **Characters with a special meaning** ([`literals`](crate::stdlib::features::literals)).
  `\%`, `\#`, `\&`, `\$`, `\{`, `\}` and `\textbackslash` stand for the characters `%`,
  `#`, `&`, `$`, `{`, `}` and `\`. `\ ` (a backslash followed by a space) stands for a
  space, and `~` for a non-breaking space. `\@` states whether a period ends a sentence:
  in `cf.\@ the appendix`, the period ends an abbreviation, and in `NASA\@.`, the period
  ends a sentence.
- **Inline formatting** ([`textstyles`](crate::stdlib::features::textstyles)).
  `\emph{…}` marks emphasis, `\textbf{…}` bold text, `\textit{…}` italic text and
  `\texttt{…}` monospaced text.
- **Verbatim content** ([`verbatim`](crate::stdlib::features::verbatim)). Verbatim
  content is taken exactly as written: no character has a special meaning in it.
  `\verbtext+…+` is verbatim text, `\verbcode[lang]+…+` a piece of code and
  `\verba[lang]+…+` an identifier, such as a keyword. The content of these three
  commands is delimited by the character that follows the command, written again at the
  end, as `+` is here, or by a pair of braces. The environment `verbatimtext` holds
  verbatim text, and the environment `verbatimcode` a block of code:
  `\begin{verbatimcode}[python]…\end{verbatimcode}`. The optional argument `[lang]`
  gives the programming language of the code.

### Headings, lists and other blocks

- **Headings** ([`headings`](crate::stdlib::features::headings)). Five commands write
  a heading, from the outermost level to the innermost: `\section{…}`,
  `\subsection{…}`, `\subsubsection{…}`, `\paragraph{…}` and `\subparagraph{…}`. The
  argument is the heading's title. A heading accepts labels of the types `sec` and
  `topic`, written after the title: `\section{Introduction}\label{sec:intro}`.
- **Lists** ([`lists`](crate::stdlib::features::lists)). Three environments write a
  list: `itemize`, a bulleted list; `enumerate`, a numbered list; and `description`, a
  list of terms, each with its description. In the body of a list, `\item` begins an
  item. `\item[…]` gives the item a *tag*, a text that marks the item in place of the
  list's bullet or number. In a `description`, each item gives its term as its tag:
  `\item[Entropy] A measure of disorder.` An item accepts labels of the type `item`,
  written directly after `\item` and its tag.
- **Quotations** ([`quote`](crate::stdlib::features::quote)). Three environments write
  a quotation: `quote`, `blockquote` and `address`. The body of a `quote` is made of
  *quotation sections*: `\text{…}`, a quoted passage; `\lines{…}`, lines kept as
  written, separated by `\\`, where `\indent` at the start of a line indents it;
  `\attributed{…}`, the author or the source of the quotation; and `\block{…}`,
  paragraphs and other blocks. These four commands may be written only in the body of
  a `quote`. The body of a `blockquote` holds paragraphs and other blocks, and the body
  of an `address` holds lines separated by `\\`.
- **Theorems** ([`theorems`](crate::stdlib::features::theorems)). Eight environments
  write the theorems, definitions and proofs of mathematical writing: `theorem`,
  `proposition`, `lemma`, `corollary`, `conjecture`, `definition`, `remark` and
  `proof`. Each takes an optional title in brackets, `\begin{lemma}[Key lemma]`, then,
  in any order, labels and `\noproofref`, which states that no proof of the statement
  follows. Each of these environments except `proof` accepts labels of the types `thm`,
  `prop`, `cor`, `lem`, `rem`, `def`, `dfn`, `x` and `topic`. A proof states which
  statement it proves with a title made of a star and the statement's label,
  `[*lem:key]`, or of two stars and the label, `[**lem:key]`.
- **Defined terms** ([`defterm`](crate::stdlib::features::defterm)).
  `\begin{defterm}{Markov chain}…\end{defterm}` defines the term `Markov chain`, and
  its body explains the term. After the term, the environment accepts labels of the
  types `term` and `topic`, and `\aliasterm{…}`, another form of the term, such as a
  plural. `\term{Markov chain}` refers to the definition of that term, and
  `\term[Markov chain]{such a process}` refers to it with other text.

```text
\begin{theorem}[Irrationality]\label{thm:sqrt2}
  The square root of two is irrational.
\end{theorem}

\begin{proof}[*thm:sqrt2]
  We argue by contradiction.
\end{proof}
```

### Footnotes, references and links

- **Footnotes** ([`xnotes`](crate::stdlib::features::xnotes)). `\footnote{…}` attaches
  a note to the place where it is written. Its content is text, or paragraphs where it
  holds a blank line or a block. A footnote accepts labels of the type `fn`, written in
  its optional argument: `\footnote[\label{fn:proof}]{…}`.
- **Labels and references** ([`refs`](crate::stdlib::features::refs)).
  `\label{type:label}` attaches a label to a construct that accepts it; each entry on
  this page gives the label types that its constructs accept, and where a label is
  written. `\ref{type:label}` refers to the construct that carries the label, and
  `\ref{eq:a,eq:b}` refers to several constructs at once.
  `\hyperref[type:label]{text}` refers to the construct with the text in braces.
- **Hyperlinks** ([`href`](crate::stdlib::features::href)). `\href{address}{text}`
  links the text to an address outside the document, such as `https://example.com`.
  `\url{address}` is a link to the address with no other text, and `\email{address}` a
  link to an email address. The address is taken exactly as written.
- **Citations** ([`citations`](crate::stdlib::features::citations)). `\cite{key}` cites
  the work that the *citation key* `key` refers to, and `\cite{key1,key2}` cites
  several works. The optional argument, as in `\cite[Thm 4]{key}`, is a note about the
  place in the work. `\R` before `\cite` makes the citation a noun of its sentence, as
  in `As \R\cite{key} shows`. The form of a citation key is given under [file names and
  citation keys](#file-names-and-citation-keys) below.

```text
\section{Results}\label{sec:results}

As shown in \ref{sec:results} and in earlier work \cite{knuth84}, the bound
holds.\footnote{See also \url{https://example.com/notes}.}
```

### Figures and tables

- **Floats** ([`floats`](crate::stdlib::features::floats)). A *float* is a figure or a
  table, with an optional caption. Two environments write a float: `figure` and `table`.
  The body of a float holds the float's content and, in any order, the caption
  `\caption{…}`, labels, and `\nonumber`. `\caption` may be written only in the body of
  a float. The label type of a float is the name of its environment:
  `\label{figure:plot}`, `\label{table:people}`. A `figure` holds exactly one image, and
  a `table` holds exactly one table of cells or one image.
- **Images** ([`graphics`](crate::stdlib::features::graphics)).
  `\includegraphics[options]{path}` inserts the image of a picture file. The options are
  `alt`, the alternative text, and `width` and `height`, lengths such as `5cm`. The
  command may be written only in the body of a float.
- **Tables of cells** ([`cells`](crate::stdlib::features::cells)). The environment
  `cells` writes a table of cells. It may be written only in the body of a `table`. In
  its body, `\cell{…}` adds one cell, and `\celldata{…}` adds one row of cells or
  several, with `&` between two cells of a row and `\\` at the end of a row.
  `\rownames{…}` and `\colnames{…}` name the rows and the columns, and `\merge{…}`
  makes a cell span several rows or columns. The *style tokens* between angle brackets,
  as in `\celldata<H>{…}`, state facts about a cell, such as its alignment or that it
  heads its column.

```text
\begin{figure}
  \includegraphics[alt=A plot of the data]{figures/plot.png}
  \caption{The data, before and after.}
  \label{figure:plot}
\end{figure}

\begin{table}
  \begin{cells}
    \celldata<H>{Name & City}
    \celldata{John & Berlin \\ Anna & Zurich}
  \end{cells}
  \caption{Who lives where.}
  \label{table:people}
\end{table}
```

### Included files, mathematics and metadata

- **Included files** ([`input`](crate::stdlib::features::input)). `\input{file}`
  brings the content of another file into the document, where the command is written.
  The form of the file name is given under [file names and citation
  keys](#file-names-and-citation-keys) below.
- **Mathematics** ([`math`](crate::stdlib::features::math)): see
  [mathematics](#mathematics) below.
- **The metadata** ([`metadata`](crate::stdlib::features::metadata)): see [the
  metadata](#the-metadata) below. No command is defined in this feature.

## What is not part of std flm

A std flm document uses no command other than those listed above. In particular, it uses
no annotation command, such as a reviewer's comment, and no *user definition*, that is, a
macro, an environment or a special character sequence defined in settings. Annotation
commands are defined in the feature
[`annotations`](crate::stdlib::features::annotations), one for each reviewer given in the
settings, and user definitions in the feature
[`userdefs`](crate::stdlib::features::userdefs). These settings may be written, for
instance, in a document's front matter. With the default settings, no command is defined
in either feature. A std flm document consists of content and metadata only, and these
settings are part of neither.

The baseline says nothing about how a document is rendered. Whether and how the headings,
equations, figures and footnotes are numbered, whether a table of contents is shown, and
how each construct appears in an output are left to the program that renders the
document and to its settings. They change nothing about whether a document is a std flm
document. Some constructs exist for these purposes:

- the star and the options of a heading command, as in `\section*{…}` and
  `\section[…]{…}`;
- the options of a list environment, and those of `\ref`;
- the flags `S` and `I` of `\R`, as in `\R[S]\cite{key}`;
- `\tag` and `\nonumber` in displayed mathematics, and `\nonumber` in a float;
- the options `width` and `height` of `\includegraphics`.

A std flm document may use them, and what they change is left to the program.

## Mathematics

The constructs of mathematics ([`math`](crate::stdlib::features::math)) are recognized:

- `\(…\)` for inline mathematics and `\[…\]` for displayed mathematics;
- the environments of displayed mathematics `equation`, `align` and `gather`, and their
  starred forms `equation*`, `align*` and `gather*`;
- in the body of these environments, `\\`, which ends a line, `\label{eq:…}`, `\tag{…}`
  and `\nonumber`;
- inside a formula, the text commands `\text{…}`, `\textnormal`, `\textup`, `\textrm`,
  `\textmd`, `\textbf`, `\textit`, `\textsl`, `\textsc`, `\textsf`, `\texttt` and
  `\mbox`, whose argument is ordinary FLM text;
- `\ensuremath{…}`, whose argument is mathematics, whether the command is written in
  text or inside a formula;
- `\eqref{eq:…}`, which refers to a line of displayed mathematics.

```text
The energy is \(E = mc^2\), and
\begin{align}
  a &= b + c \label{eq:sum} \\
  d &= e \text{ for all } e
\end{align}
by \eqref{eq:sum}.
```

No particular promise is made about which mathematical content is understood. The
commands that a formula may contain, such as `\alpha`, `\frac` or a `cases` environment,
are read by the program that processes the document, with a processor for mathematics
of its choice. A formula in common LaTeX notation is expected to be processed correctly
by a reasonable processor, but the baseline does not list the commands that such a
processor understands.

## File names and citation keys

`\input{…}` and `\includegraphics{…}` accept a file name written as a UNIX-style path,
with `/` between the names of the folders: `\input{parts/intro.flm}`,
`\includegraphics{figures/plot.png}`. How a path is resolved is left to the program,
within reason, and depends on the context in which the document is processed. For a
document kept in files, a path is expected to be relative, for instance to the folder of
the file that holds the command. In some contexts, `\includegraphics` may also accept an
address, such as `https://example.org/plot.svg`.

A citation key is the text written between the braces of `\cite`. Several keys are
separated by commas, and each key is taken without the spaces around it:
`\cite{knuth84, arxiv:1234.5678}`. A comma inside braces belongs to the key, and every
run of spaces inside a key counts as one space. The baseline gives no meaning to a key:
which keys lead to a cited work depends on the program that processes the document. For
instance, a program may look up keys of the form `arxiv:…` in an online database, and
other keys in a bibliography file.

## The metadata

The metadata of a std flm document is a set of keys and their values. Three keys have a
meaning, and each may be left out:

- `title`: one text, the title of the document;
- `author`: one author, or a list of authors;
- `date`: one text, the date of the document as its writer writes it.

An author is a text, the author's name, or a map with the following keys:

- `name`: one text, the author's name; a map must have this key;
- `affiliation`: one text, or a list of texts;
- `email`: one email address;
- `note`: one text, or a list of texts, notes about the author.

Every text of the metadata is FLM text, read as inline content (text without a paragraph
break), except the email address, which is plain text. For instance, a title may be
`The \emph{Art} of Parsing`. The metadata may hold other keys, and an author's map may
hold other keys too; the baseline gives them no meaning. Written in YAML, the metadata of
a document with two authors reads:

```yaml
title: 'The \emph{Art} of Parsing'
author:
  - name: 'Ada Lovelace'
    affiliation: ['University of X', 'Institute of Y']
    email: 'ada@example.com'
    note: 'Corresponding author'
  - 'Grace Hopper'
date: 'November 2026'
```

The module [`metadata::data`](crate::stdlib::features::metadata::data) describes the
same form as data that a program reads.

## How std flm relates to the flm program

The `flm` command-line program loads its package `std` into every document by default; a
*package* is a named group of features (see [the packages a document
uses](crate::guide::command_line#the-packages-a-document-uses)). Consider a document
whose front matter (the block of YAML at the start of the file) holds nothing but the
metadata. The program reads such a document as std flm, plus what its package `std`
adds, such as the services that find cited works and image files, and it processes the
document's mathematics with its default processor for mathematics. The package `std`
belongs to this program, and another program may put other features in a package of that
name. What every program that reads FLM has in common is std flm, the baseline that this
page describes and that no code defines.
