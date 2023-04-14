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
    'arabic': str,
    'fnsymbol': fnsymbolcounter,
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
        'pair_mid': ' ${and} ',
        'pair_post': '',
        'range_pre': '',
        'range_mid': '${endash}',
        'range_pairmid': '${sep}',
        'range_post': '',
        'list_pre': '',
        'list_mid': '${sep} ',
        'list_midlast': '${sep} ${and} ',
        'list_post': '',
        
        'and': 'and',
        'sep': ',',
        'endash': '–',
        'empty': '(empty)',
    },
    'compact': {
        'pair_mid': ',',
        'range_mid': '–', # en dash
        'range_pairmid': ',',
        'list_mid': ',',
        'list_midlast': ',',

        'empty': '(empty)',
    },
}


# TODO: deprecate parse_counter_formatter to avoid confusion with CounterFormatter
parse_counter_format_num = parse_counter_formatter


def build_counter_formatter(counter_formatter, default_counter_formatter_spec, *,
                            counter_formatter_id):
    r"""
    Build a CounterFormatter() instance from the given configuration
    counter_formatter.  It can be a string (e.g. 'arabic'), a 
    """

    if isinstance(counter_formatter, CounterFormatter):
        return counter_formatter

    if counter_formatter is None:
        return CounterFormatter(**default_counter_formatter_spec)

    default_counter_formatter_spec = dict(default_counter_formatter_spec)

    if isinstance(counter_formatter, str):
        d = default_counter_formatter_spec
        d['format_num'] = counter_formatter
        d['counter_formatter_id'] = counter_formatter_id
        return CounterFormatter(**d)

    if isinstance(counter_formatter, dict):
        if 'template' in counter_formatter:
            d = default_counter_formatter_spec
            d['format_num'] = counter_formatter
            d['counter_formatter_id'] = counter_formatter_id
            return CounterFormatter(**d)
        d = default_counter_formatter_spec
        d['counter_formatter_id'] = counter_formatter_id
        d.update(counter_formatter)
        return CounterFormatter(**d)
        
    raise ValueError("Invalid counter_formatter specification: " + repr(counter_formatter))


class CounterFormatter:
    def __init__(self, format_num, prefix_display=None, 
                 delimiters=None, join_spec=None, name_in_link=True,
                 counter_formatter_id=None):
        self.format_num = parse_counter_formatter(format_num)
        if prefix_display is None:
            prefix_display = {
                'singular': '',
                'plural': '',
            }
        elif isinstance(prefix_display, str):
            prefix_display = {
                'singular': prefix_display,
                'plural': prefix_display,
            }
        self.prefix_display = prefix_display
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
        self.name_in_link = name_in_link

        self.counter_formatter_id = counter_formatter_id

        self._fields = ('format_num', 'prefix_display', 'delimiters', 'join_spec',
                        'name_in_link', 'counter_formatter_id',)

    def asdict(self):
        return {k: getattr(self, k) for k in self._fields}

    def __repr__(self):
        return (
            f"{self.__class__.__name__}("
            + ", ".join(f"{k}={repr(getattr(self,k))}" for k in self._fields)
            + ")"
        )

    def format_flm(self, value, prefix_variant=None, with_delimiters=True, with_prefix=True,
                   wrap_format_num=None, wrap_link_fn=None):
        prefix, pre, post = self._get_format_pre_post(
            with_delimiters,
            with_prefix,
            1,
            prefix_variant,
        )
        s_num = self.format_num(value)
        if wrap_format_num is not None:
            s_num = wrap_format_num(s_num)
        s = prefix + pre + s_num + post
        if wrap_link_fn is not None:
            return wrap_link_fn(value, s)
        return s

    def _get_format_pre_post(self, with_delimiters, with_prefix,
                             num_values, prefix_variant):
        prefix, pre, post = '', '', ''

        if with_delimiters:
            pre = self.delimiters[0]
            post = self.delimiters[1]

        if with_prefix:
            prefixinfo = self.prefix_display
            if prefix_variant is not None and prefix_variant in self.prefix_display:
                prefixinfo = self.prefix_display[prefix_variant]
            if num_values == 1:
                prefix = prefixinfo['singular']
            elif num_values in prefixinfo:
                prefix = prefixinfo[num_values]
            else:
                prefix = prefixinfo['plural']

        return prefix, pre, post


    def format_many_flm(self, values, prefix_variant=None, with_delimiters=True,
                        with_prefix=True, wrap_link_fn=None,
                        wrap_format_num=None, get_raw_s_items=False, s_items_join=None):

        join_spec = self.join_spec
        name_in_link = self.name_in_link

        if s_items_join is None:
            s_items_join = lambda a, b: a + b

        if len(values) == 0:
            return join_spec['empty']

        values = [ _expect_int(v) for v in values ]

        # key= appears to be required for Transcrypt(), so that a comparison
        # function is provided in the JS code, otherwise JavaScript's sort()
        # converts to string and sorts alphabetically ... :/
        values = sorted(values, key=int) #lambda x: int(x))

        num_values = len(values)

        only_one_value = False
        if num_values == 1:
            only_one_value = True

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
        if len(list_of_ranges) == 1:
            if list_of_ranges[0][0] + 1 == list_of_ranges[0][1]:
                # single pair of consecutive values -> format as pair of values,
                # each value seen as a range
                list_of_ranges = [ (list_of_ranges[0][0], list_of_ranges[0][0]),
                                   (list_of_ranges[0][1], list_of_ranges[0][1]), ]

        def _format_num(n):
            if wrap_format_num is not None:
                return wrap_format_num( self.format_num(n) )
            return self.format_num(n)

        def _render_range_items(a, b):
            if a == b:
                return [ { 's': _format_num(a), 'n': a } ]
            s_a = _format_num(a)
            s_b = _format_num(b)
            if b == a + 1:
                mid = join_spec['range_pairmid']
            else:
                mid = join_spec['range_mid']
            return [
                { 's': join_spec['range_pre'], 'n': False },
                { 's': s_a, 'n': a },
                { 's': mid, 'n': False },
                { 's': s_b, 'n': b },
                { 's': join_spec['range_post'], 'n': False },
            ]

        if len(list_of_ranges) == 1:
            s_items = (
                [ { 's': join_spec['one_pre'], 'n': None } ]
                + _render_range_items(*list_of_ranges[0])
                + [ { 's': join_spec['one_post'], 'n': None } ]
            )
        elif len(list_of_ranges) == 2:
            s_items = (
                [ { 's': join_spec['pair_pre'], 'n': False } ]
                + _render_range_items(*list_of_ranges[0])
                + [ { 's': join_spec['pair_mid'], 'n': False } ]
                + _render_range_items(*list_of_ranges[1])
                + [ { 's':  join_spec['pair_post'], 'n': False } ]
            )
        else:
            s_items = [ { 's': join_spec['list_pre'], 'n': False } ]
            for rngj, rng in enumerate(list_of_ranges[:-1]):
                if rngj > 0:
                    s_items += [ { 's': join_spec['list_mid'], 'n': False } ]
                s_items += _render_range_items(*rng)
            s_items += (
                [ { 's': join_spec['list_midlast'], 'n': False } ]
                + _render_range_items(*list_of_ranges[-1])
                + [ { 's': join_spec['list_post'], 'n': False } ]
            )

        s_prefix, s_pre, s_post = self._get_format_pre_post(
            with_delimiters, with_prefix, num_values, prefix_variant
        )

        first_n = None
        if not name_in_link:
            first_n = False
        else:
            for si in s_items:
                nn = si.get('n', None)
                if nn is not None and nn is not False:
                    first_n = nn
                    break

        s_pre_items = []
        if len(s_prefix):
            s_pre_items.append( { 's': s_prefix, 'n': first_n } )
        s_pre_items.append(
            { 's': s_pre,
              'n': None if (name_in_link and only_one_value) else False }
        )

        # add pre/post text
        s_items = (
            s_pre_items
            + s_items
            + [ { 's': s_post, 'n': None if only_one_value else False } ]
        )

        #print('s_items = ', s_items) #DEBUG

        # first, compress items by common link targets (if necessary)
        if wrap_link_fn is not None or get_raw_s_items:
            s_all = []
            cur_s = None
            cur_n = False
            for s_item in s_items:
                si = s_item['s']
                ni = s_item.get('n', None)
                if ni is False and cur_n is False and cur_s is not None:
                    cur_s = s_items_join(cur_s, si)
                    continue
                if cur_n is not False and (ni is None or cur_n is None or ni == cur_n):
                    if ni is not None and cur_n is None:
                        cur_n = ni
                    # add to current link
                    if cur_s is None:
                        cur_s = si
                    else:
                        # don't use += in case the type is mutable, e.g., a nodelist!
                        cur_s = s_items_join(cur_s, si)
                    continue
                # end link here
                if cur_s is not None:
                    s_all.append({'s': cur_s, 'n': cur_n})
                # start anew
                cur_s = si
                cur_n = ni

            if cur_s is not None:
                s_all.append({'s': cur_s, 'n': cur_n})

            s_items = s_all

        #print('compressed s_items = ', s_items) # DEBUG

        if get_raw_s_items:
            return s_items

        if wrap_link_fn is not None:
            s = "".join([
                ( wrap_link_fn(x['n'], x['s'])
                  if (x['n'] is not None and x['n'] is not False)
                  else  x['s'] )
                for x in s_items
            ])
        else:
            # ignore link information
            s = "".join([ x['s'] for x in s_items ])

        return s



def _expect_int(v):
    try:
        return int(v)
    except TypeError:
        raise ValueError("Invalid value, expected integer: " + repr(v))


# --------------------------------------


class Counter:
    def __init__(self, counter_formatter, initial_value=0):
        self.formatter = counter_formatter
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

    def format_flm(self, value=None, **kwargs):
        if value is None:
            value = self.value
        kwargs2 = {'with_prefix': False}
        kwargs2.update(kwargs)
        return self.formatter.format_flm(value, **kwargs2)

    def step_and_format_flm(self):
        val = self.step()
        return val, self.format_flm(val)


class CounterAlias:
    def __init__(self, counter_formatter, alias_counter):
        self.formatter = counter_formatter
        self.alias_counter = alias_counter

    @property
    def value(self):
        return self.alias_counter.value
        
    def step(self):
        return self.alias_counter.step()

    def reset(self):
        return self.alias_counter.reset()

    def format_flm(self, value=None, **kwargs):
        if value is None:
            value = self.value
        kwargs2 = {'with_prefix': False}
        kwargs2.update(kwargs)
        return self.formatter.format_flm(value, **kwargs2)

    def step_and_format_flm(self):
        val = self.step()
        return val, self.format_flm(val)

