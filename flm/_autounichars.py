#
# Includes code adapted from smartypants.py
# https://github.com/justinmayer/smartypants.py/blob/main/smartypants.py
#
# Original Copyright:
#
# =========
# Copyright
# =========
# 
# SmartyPants
# ===========
# 
# ::
# 
#   Copyright (c) 2003 John Gruber
#   (http://daringfireball.net/)
#   All rights reserved.
# 
#   Redistribution and use in source and binary forms, with or without
#   modification, are permitted provided that the following conditions are
#   met:
# 
#   *   Redistributions of source code must retain the above copyright
#     notice, this list of conditions and the following disclaimer.
# 
#   *   Redistributions in binary form must reproduce the above copyright
#     notice, this list of conditions and the following disclaimer in
#     the documentation and/or other materials provided with the
#     distribution.
# 
#   *   Neither the name "SmartyPants" nor the names of its contributors
#     may be used to endorse or promote products derived from this
#     software without specific prior written permission.
# 
#   This software is provided by the copyright holders and contributors "as
#   is" and any express or implied warranties, including, but not limited
#   to, the implied warranties of merchantability and fitness for a
#   particular purpose are disclaimed. In no event shall the copyright
#   owner or contributors be liable for any direct, indirect, incidental,
#   special, exemplary, or consequential damages (including, but not
#   limited to, procurement of substitute goods or services; loss of use,
#   data, or profits; or business interruption) however caused and on any
#   theory of liability, whether in contract, strict liability, or tort
#   (including negligence or otherwise) arising in any way out of the use
#   of this software, even if advised of the possibility of such damage.
# 
# 
# smartypants
# ===========
# 
# ::
# 
#   smartypants is a derivative work of SmartyPants.
# 
#   Copyright (c) 2025–present, Justin Mayer
#   Copyright (c) 2017, Leo Hemsted
#   Copyright (c) 2013, 2014, 2015, 2016 Yu-Jie Lin
#   Copyright (c) 2004, 2005, 2007, 2013 Chad Miller
# 
#   Redistribution and use in source and binary forms, with or without
#   modification, are permitted provided that the following conditions are
#   met:
# 
#   *   Redistributions of source code must retain the above copyright
#     notice, this list of conditions and the following disclaimer.
# 
#   *   Redistributions in binary form must reproduce the above copyright
#     notice, this list of conditions and the following disclaimer in
#     the documentation and/or other materials provided with the
#     distribution.
# 
#   This software is provided by the copyright holders and contributors "as
#   is" and any express or implied warranties, including, but not limited
#   to, the implied warranties of merchantability and fitness for a
#   particular purpose are disclaimed. In no event shall the copyright
#   owner or contributors be liable for any direct, indirect, incidental,
#   special, exemplary, or consequential damages (including, but not
#   limited to, procurement of substitute goods or services; loss of use,
#   data, or profits; or business interruption) however caused and on any
#   theory of liability, whether in contract, strict liability, or tort
#   (including negligence or otherwise) arising in any way out of the use
#   of this software, even if advised of the possibility of such damage.



import re


def convert_auto_quotes(text):
    """
    Convert quotes in `text` into unicode curly quote entities.
    """

    punct_class = r"""[!"#\$\%'()*+,-.\/:;<=>?\@\[\\\]\^_`{|}~]"""

    #uni8216 = '‘' # 8216 = 0x2018
    #uni8217 = '’' # 8217 = 0x2019
    #uni8220 = '“' # 8220 = 0x201c
    #uni8221 = '”' # 8221 = 0x201d

    # Special case if the very first character is a quote
    # followed by punctuation at a non-word-break. Close the quotes by brute
    # force:
    text = re.sub(r"""^'(?=%s\\B)""" % (punct_class,), '’', text)
    text = re.sub(r"""^"(?=%s\\B)""" % (punct_class,), '”', text)

    # Special case for double sets of quotes, e.g.:
    #   <p>He said, "'Quoted' words in a larger quote."</p>
    text = re.sub(r""""'(?=\w)""", '“‘', text)
    text = re.sub(r"""'"(?=\w)""", '‘“', text)

    # Special case for decade abbreviations (the '80s):
    text = re.sub(r"""\b'(?=\d{2}s)""", '’', text)

    close_class = r'[^\ \t\r\n\[\{\(\-]'
    #dec_dashes = '–|—'
    # &#8211; -> 0x2013
    # &#8212; -> 0x2014

    # Get most opening single quotes:
    opening_single_quotes_regex = re.compile(r"""
            (
                \s          |   # a whitespace char, or
                [ ]         |   # a non-breaking space entity, or
                --          |   # dashes, or
                [–—-]           # unicode dashes
            )
            '                 # the quote
            (?=\w)            # followed by a word character
            """, re.VERBOSE)
    text = opening_single_quotes_regex.sub(r'\1‘', text)

    closing_single_quotes_regex = re.compile(r"""
            (%s)
            '
            (?!\s | s\b | \d)
            """ % (close_class,), re.VERBOSE)
    text = closing_single_quotes_regex.sub(r'\1’', text)

    closing_single_quotes_regex = re.compile(r"""
            (%s)
            '
            (\s | s\b)
            """ % (close_class,), re.VERBOSE)
    text = closing_single_quotes_regex.sub(r'\1’\2', text)

    # Any remaining single quotes should be opening ones:
    text = re.sub("'", '‘', text)

    # Get most opening double quotes:
    opening_double_quotes_regex = re.compile(r"""
            (
                \s          |   # a whitespace char, or
                [ ]         |   # a non-breaking space entity, or
                --          |   # dashes, or
                [–—-]           # unicode dashes
            )
            "                 # the quote
            (?=\w)            # followed by a word character
            """, re.VERBOSE)
    text = opening_double_quotes_regex.sub(r'\1“', text)

    # Double closing quotes:
    closing_double_quotes_regex = re.compile(r"""
            #(%s)?   # character that indicates the quote should be closing
            "
            (?=\s)
            """ % (close_class,), re.VERBOSE)
    text = closing_double_quotes_regex.sub('”', text)

    closing_double_quotes_regex = re.compile(r"""
            ^
            "
            (?=%s)
            """ % (punct_class,), re.VERBOSE)
    text = closing_double_quotes_regex.sub('”', text)

    closing_double_quotes_regex = re.compile(r"""
            (%s)   # character that indicates the quote should be closing
            "
            """ % (close_class,), re.VERBOSE)
    text = closing_double_quotes_regex.sub(r'\1”', text)

    # Any remaining quotes should be opening ones.
    text = re.sub('"', '“', text)

    return text



def convert_ligature_quotes(text):
    """
    Convert ````backticks''``-style singel and double quotes in `text` into
    unicode curly quote entities.
    """

    text = re.sub('``', '“', text)
    text = re.sub("''", '”', text)
    text = re.sub('`', '‘', text)
    text = re.sub("'", '’', text)
    return text


def convert_ligature_dashes(text):
    """
    Convert ``--`` and ``---`` in `text` into en-dash and em-dash unicode
    entities, respectively.
    """

    text = re.sub(r'---', '—', text)
    text = re.sub(r'--', '–', text)
    return text


def convert_ligature_ellipses(text):
    text = re.sub(r'\.\.\.', '…', text)
    return text
