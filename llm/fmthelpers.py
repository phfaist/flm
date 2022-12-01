import re

#_alpha = 'abcdefghijklmnopqrstuvwxyz'

def alphacounter(n, lower=True):
    # a, b, c, d, ..., y, z, aa, bb, cc, ..., zz, aaa, ...
    n -= 1 # start counting at 1
    w = 1 + (n // 26)
    m = n % 26
    #s = _alpha[m] * w
    s = chr(97+m) * w
    if lower:
        return s
    return s.upper()
    
def Alphacounter(n):
    return alphacounter(n, lower=False)


_romancounterchars = (
    (1000, "M"),
    (900, "CM"),
    (500, "D"),
    (400, "CD"),
    (100, "C"),
    (90, "XC"),
    (50, "L"),
    (40, "XL"),
    (10, "X"),
    (9, "IX"),
    (5, "V"),
    (4, "IV"),
    (1, "I"),
)

def romancounter(n, lower=True, zero=''):
    s = ''
    if n == 0:
        return zero
    for romancharvalue, romanchar in _romancounterchars:
        s += romanchar * (n // romancharvalue)
        n = n % romancharvalue

    if lower:
        return s.lower()

    return s

def Romancounter(n):
    return romancounter(n, lower=False)

_fnsymbols = [
    '*',
    '†',
    '‡',
    '§',
    '¶',
    '‖',
]

def fnsymbolcounter(n, symbols=_fnsymbols):
    # *, †, ..., **, ††, ..., ***, †††, ... ...
    n -= 1 # start counting at 1
    N = len(symbols)
    w = 1 + (n // N)
    m = n % N
    s = symbols[m] * w
    return s
    

# _unicodesuperscriptdigits[4] == '⁴'
# _unicodesubscriptdigits[4] == '₄'
#
# cf. https://en.wikipedia.org/wiki/Unicode_subscripts_and_superscripts
_unicodesuperscriptdigits = [
    chr(0x2070), chr(0x00B9), chr(0x00B2), chr(0x00B3), chr(0x2074),
    chr(0x2075), chr(0x2076), chr(0x2077), chr(0x2078), chr(0x2079),
]
_unicodesubscriptdigits = [
    chr(0x2080+j)
    for j in range(10)
]


def customdigitscounter(n, digits='0123456789'):
    base = len(digits)
    s = ''
    while n:
        q, r = n // base, n % base
        s = digits[r] + s
        n = q
    return ''.join(s)

def unicodesuperscriptcounter(n):
    return customdigitscounter(n, digits=_unicodesuperscriptdigits)
def unicodesubscriptcounter(n):
    return customdigitscounter(n, digits=_unicodesubscriptdigits)


standard_counter_formatters = {
    'alph': lambda n: alphacounter(n, lower=True),
    'Alph': lambda n: alphacounter(n, lower=False),
    'roman': lambda n: romancounter(n, lower=True),
    'Roman': lambda n: romancounter(n, lower=False),
    'arabic': lambda n: str(n),
    'fnsymbol': lambda n: fnsymbolcounter(n),
    'unicodesuperscript': unicodesuperscriptcounter,
    'unicodesubscript': unicodesubscriptcounter,
}

_standard_tag_template_initials_formatters = {
    'a': alphacounter,
    'A': Alphacounter,
    'i': romancounter,
    'I': Romancounter,
    '1': str,
}

def parse_counter_formatter(
        counter_formatter,
        named_counter_formatters=standard_counter_formatters,
        str_use_tag_template=False,
        tag_template_initials_counters=_standard_tag_template_initials_formatters,
):
    if callable(counter_formatter):
        return counter_formatter
    if isinstance(counter_formatter, str):
        if counter_formatter in named_counter_formatters:
            return named_counter_formatters[counter_formatter]
        if str_use_tag_template:
            return _parse_counter_formatter_from_tag_template(
                counter_formatter,
                tag_template_initials_counters
            )
    if isinstance(counter_formatter, dict):
        if 'template' in counter_formatter:
            tmpl = counter_formatter['template']
            # simple template parsing ${arabic}
            pat = "|".join(re.escape(k) for k in named_counter_formatters.keys())
            _rx_counter = re.compile(r'\$\{(' + pat + r')\}')
            return lambda n: (
                _rx_counter.sub(
                    lambda m:  named_counter_formatters[m.group(1)] (n),
                    tmpl,
                )
            )
    raise ValueError(f"Invalid counter_formatter: ‘{repr(counter_formatter)}’")
            
def _parse_counter_formatter_from_tag_template(
        tag_template,
        tag_template_initials_counters=_standard_tag_template_initials_formatters,
):
    rx = re.compile(r'['+''.join(tag_template_initials_counters.keys())+r']')
    m = rx.search(tag_template)
    if m is not None:
        # substitute a counter
        left = tag_template[:m.start()]
        right = tag_template[m.end():]
        counter_formatter = tag_template_initials_counters[m.group()]
        return lambda n: (left + counter_formatter(n) + right)

    # no counter. E.g., a bullet symbol
    return lambda n: tag_template
