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



_rx_dollar_template = re.compile(r'\$\{([a-zA-Z0-9_.]+)\}')
def _replace_dollar_template(x, vrs):
    return _rx_dollar_template.sub(lambda m: vrs[m.group(1)], x)
def _replace_dollar_template_delayed(x, vrs):
    return lambda arg: (
        _rx_dollar_template.sub(lambda m: vrs[m.group(1)] (arg) , x)
    )


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
            return _replace_dollar_template_delayed(tmpl, named_counter_formatters)
            # pat = "|".join(re.escape(k) for k in named_counter_formatters.keys())
            # _rx_counter = re.compile(r'\$\{(' + pat + r')\}')
            # return lambda n: (
            #     _rx_counter.sub(
            #         lambda m:  named_counter_formatters[m.group(1)] (n),
            #         tmpl,
            #     )
            # )
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



_default_formatter_join_spec = {
    'default': {
        'one_pre': '',
        'one_post': '',
        'pair_pre': '',
        'pair_mid': ' ${and} ',
        'pair_post': '',
        'range_pre': '',
        'range_mid': '${endash}',
        'range_post': '',
        'list_pre': '',
        'list_mid': '${sep} ',
        'list_midlast': '${sep} ${and} ',
        'list_post': '',
        
        'and': 'and',
        'sep': ',',
        'endash': '–',
        'empty': '(empty)',
    },
    'compact': {
        'pair_mid': ',',
        'range_mid': '–', # en dash
        'list_mid': ',',
        'list_midlast': ',',

        'empty': '(empty)',
    },
}



class CounterFormatter:
    def __init__(self, format_num, prefix_display=None, ref_type=None,
                 delimiters=None, join_spec=None, nameinlink=True):
        self.format_num = parse_counter_formatter(format_num)
        if isinstance(prefix_display, str):
            prefix_display = {
                'singular': prefix_display,
                'plural': prefix_display,
                'Singular': prefix_display,
                'Plural': prefix_display,
            }
        self.prefix_display = prefix_display
        self.ref_type = ref_type
        self.delimiters = delimiters if delimiters is not None else ('', '')
        jd = dict(_default_formatter_join_spec['default'])
        if join_spec is not None:
            if isinstance(join_spec, str):
                jd.update(_default_formatter_join_spec[join_spec])
            else:
                jd.update(join_spec)
        self.join_spec = {
            k: _replace_dollar_template(v, jd)
            for (k,v) in jd.items()
        }
        self.nameinlink = nameinlink

    def format_llm(self, value, capital=False, with_delimiters=True, with_prefix=True,
                   wrap_link_fn=None):
        glob_wrap_link_fn = wrap_link_fn and (lambda s: wrap_link_fn(value, s))
        return self._format_with_delimiters_prefix(
            self.format_num(value),
            with_delimiters,
            with_prefix,
            1,
            capital,
            glob_wrap_link_fn,
            glob_wrap_link_fn,
            glob_wrap_link_fn,
        )

    def _format_with_delimiters_prefix(self, s, with_delimiters, with_prefix,
                                       num_values, capital,
                                       glob_wrap_link_fn_pre,
                                       glob_wrap_link_fn_mid,
                                       glob_wrap_link_fn_post):
        if glob_wrap_link_fn_pre is None:
            glob_wrap_link_fn_pre = lambda s: s
        if glob_wrap_link_fn_mid is None:
            glob_wrap_link_fn_mid = lambda s: s
        if glob_wrap_link_fn_post is None:
            glob_wrap_link_fn_post = lambda s: s

        pre, post = '', ''

        if with_delimiters:
            pre = self.delimiters[0]
            post = self.delimiters[1]

        if with_prefix:
            sing, plur = 'singular', 'plural'
            if capital:
                sing, plur = 'Singular', 'Plural'
            if num_values == 1:
                prefix = self.prefix_display[sing]
            elif num_values in self.prefix_display:
                prefix = self.prefix_display[num_values]
            else:
                prefix = self.prefix_display[plur]
            pre = prefix + pre

        return (
            glob_wrap_link_fn_pre(pre) + glob_wrap_link_fn_mid(s)
            + glob_wrap_link_fn_post(post)
        )

    def format_many_llm(self, values, capital=False, with_delimiters=True, with_prefix=True,
                        wrap_link_fn=None):
        if len(values) == 0:
            return self.join_spec['empty']
        if wrap_link_fn is None:
            wrap_link_fn = lambda n, s: s
        #
        values = sorted(values)
        num_values = len(values)
        #
        list_of_ranges = []
        cur_range = None
        for v in values:
            if not cur_range:
                cur_range = (v, v)
                continue
            if v == cur_range[1]+1:
                cur_range = (cur_range[0], v)
                continue
            list_of_ranges.append(cur_range)
            cur_range = (v, v)

        list_of_ranges.append(cur_range)
        if len(list_of_ranges) == 1 and list_of_ranges[0][0] == list_of_ranges[0][1]+1:
            # single pair of consecutive values -> format as pair of values,
            # each value seen as a range
            list_of_ranges = [ (list_of_ranges[0][0], list_of_ranges[0][0]),
                               (list_of_ranges[0][1], list_of_ranges[0][1]), ]
        fmtd_ranges = [
            self._format_range(rng[0], rng[1], wrap_link_fn)
            for rng in list_of_ranges
        ]
        #
        fmtd = self._format_list_of_ranges(fmtd_ranges)
        #
        return self._format_with_delimiters_prefix(
            fmtd, with_delimiters, with_prefix, num_values, capital,
            lambda s: wrap_link_fn(list_of_ranges[0][0], s),
            None,
            lambda s: wrap_link_fn(list_of_ranges[-1][1], s),
        )

    def _format_range(self, a, b, wrap_link_fn):
        if a == b:
            return wrap_link_fn(a, self.format_num(a))
        return (
            self.join_spec['range_pre'] + wrap_link_fn(a, self.format_num(a))
            + self.join_spec['range_mid'] + wrap_link_fn(b, self.format_num(b))
            + self.join_spec['range_post']
        )
    def _format_list_of_ranges(self, fmtd_ranges):
        if len(fmtd_ranges) == 1:
            return (
                self.join_spec['one_pre'] + fmtd_ranges[0] + self.join_spec['one_post']
            )
        if len(fmtd_ranges) == 2:
            return (
                self.join_spec['pair_pre'] + fmtd_ranges[0]
                + self.join_spec['pair_mid'] + fmtd_ranges[1]
                + self.join_spec['pair_post']
            )
        return (
            self.join_spec['list_pre']
            + self.join_spec['list_mid'].join(fmtd_ranges[:-1])
            + self.join_spec['list_midlast'] + fmtd_ranges[-1]
            + self.join_spec['list_post']
        )


class Counter:
    def __init__(self, counter_formatter, initial_value=0):
        self.formatter = CounterFormatter(**counter_formatter)
        self.value = initial_value
        
    def set_value(self, value):
        self.value = value
        return self.value

    def step(self):
        self.value += 1
        return self.value

    def reset(self):
        self.value = self.initial_value
        return self.value

    def format_llm(self, value=None):
        if value is None:
            value = self.value
        return self.formatter.format_llm(self.value)

    def step_and_format_llm(self):
        val = self.step()
        return val, self.format_llm(val)

