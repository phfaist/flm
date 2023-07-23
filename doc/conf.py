# Configuration file for the Sphinx documentation builder.
#
# For the full list of built-in configuration values, see the documentation:
# https://www.sphinx-doc.org/en/master/usage/configuration.html

import os.path
import sys
sys.path.insert(0, os.path.abspath(os.path.join(os.path.dirname(__file__), '..')))

import flm

# -- Project information -----------------------------------------------------
# https://www.sphinx-doc.org/en/master/usage/configuration.html#project-information

project = 'FLM'
copyright = '2023, Philippe Faist'
author = 'Philippe Faist'

# The version info for the project you're documenting, acts as replacement for
# |version| and |release|, also used in various other places throughout the
# built documents.
#
# The short X.Y version.
version = flm.__version__
# The full version, including alpha/beta/rc tags.
release = version


# -- General configuration ---------------------------------------------------
# https://www.sphinx-doc.org/en/master/usage/configuration.html#general-configuration

# Add any Sphinx extension module names here, as strings. They can be
# extensions coming with Sphinx (named 'sphinx.ext.*') or your custom
# ones.
extensions = [
    'sphinx.ext.autodoc',
    'sphinx.ext.intersphinx',
    'sphinx.ext.mathjax',

    'sphinx_issues',
]

templates_path = ['_templates']
exclude_patterns = ['_build', 'Thumbs.db', '.DS_Store']

# The master toctree document.
master_doc = 'index'

# The name of the Pygments (syntax highlighting) style to use.
pygments_style = 'sphinx'

# If true, `todo` and `todoList` produce output, else they produce nothing.
todo_include_todos = False



#autodoc_docstring_signature = True
autodoc_member_order = 'bysource'
autodoc_inherit_docstrings = False


# -- Options for sphinx_issues --------------------------------------------

# GitHub repo
issues_github_path = "phfaist/flm"


# -- Options for HTML output -------------------------------------------------
# https://www.sphinx-doc.org/en/master/usage/configuration.html#options-for-html-output

html_theme = 'alabaster'
html_static_path = ['_static']

# Theme options are theme-specific and customize the look and feel of a theme
# further.  For a list of options available for each theme, see the
# documentation.
#
html_theme_options = {
    'font_family': 'Merriweather',
    'font_size': '14px',
    'head_font_family': 'Merriweather',
    'code_font_family': 'Source Code Pro',
    # 'github_user': 'phfaist',
    # 'github_repo': 'flm',
    # 'github_button': True,
    # 'github_type': 'star',
    # 'github_count': 'true',

    'fixed_sidebar': True,
    'page_width': '950px',
    'sidebar_width': '220px',
}
html_sidebars = {
    '**': [
        'about.html',
        'navigation.html',
        'relations.html',
        'searchbox.html',
#        'donate.html',
    ]
}



# -- Options for LaTeX output ---------------------------------------------

latex_elements = {
    # The paper size ('letterpaper' or 'a4paper').
    #
    # 'papersize': 'letterpaper',

    # The font size ('10pt', '11pt' or '12pt').
    #
    # 'pointsize': '10pt',

    # Additional stuff for the LaTeX preamble.
    #
    # 'preamble': '',

    # Latex figure (float) alignment
    #
    # 'figure_align': 'htbp',
}

# Grouping the document tree into LaTeX files. List of tuples
# (source start file, target name, title,
#  author, documentclass [howto, manual, or own class]).
latex_documents = [
    (master_doc, 'flm.tex', 'FLM Documentation',
     'Philippe Faist', 'manual'),
]







# Configuration for intersphinx: where to link to documentation for the standard
# python library and for pylatexenc.
intersphinx_mapping = {
    'python': ('https://docs.python.org/3', None),
    'pylatexenc': ('https://pylatexenc.readthedocs.io/en/latest/', None)
}
