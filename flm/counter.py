import re

def alphacounter(n, lower=True):
    r"""
    Returns a string representing the number `n` using a latin letter,
    starting at `n=1`.  The sequence is 'a', 'b', 'c', ..., 'y', 'z', 'aa',
    'bb', ..., 'zz', 'aaa', ...

    If `lower=True` (the default), then lowercase letters are used.  Uppercase
    letters are used if `lower=False`.
    """
    n -= 1 # start counting at 1
    w = 1 + (n // 26)
    m = n % 26
    s = chr(97+m) * w
    if lower:
        return s
    return s.upper()
    
def Alphacounter(n):
    r"""
    Shorthand for ``alphacounter(n, lower=False)``.
    """
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
    r"""
    Returns a string representing the number `n` as a roman numeral (I, II,
    III, IV, etc.).
    
    If `lower=True` (the default), then lowercase letters are used; uppercase
    letters are used if `lower=False`.

    The argument `zero` is a string to return in case `n` is equal to zero (by
    default, an empty string).
    """
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
    r"""
    Shorthand for ``romancounter(n, lower=False)``.
    """
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
    r"""
    Return a symbol that is suitable for representing a footnote whose
    internal numbering is given by `n`.  Numbering starts at `n=1`.

    The argument `symbols` provides a sequence of symbols to go through.  By
    default, the sequence is ``'*', '†', '‡', '§', '¶', '‖'``.

    For `n` larger than the `symbols` sequence length, we begin returning the
    symbol repeated multiple times.  I.e., with the default symbols sequence,
    we'll get ``'*', '†', '‡', '§', '¶', '‖', '**', '††', ..., '‖‖', '***', ...``
    """

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
    r"""
    Return a string representation of `n` using the digits `digits` (the
    base is determined by the length of the `digits` list).

    For example, to get a binary representation of `n` using 'F' and 'T' instead
    of '0' and '1', you can use ``customdigitscounter(n, 'FT')``
    """
    base = len(digits)
    s = ''
    while n:
        q, r = n // base, n % base
        s = digits[r] + s
        n = q
    return s #''.join(s)

def unicodesuperscriptcounter(n):
    r"""
    Return a unicode string representation of the number `n`, in base 10,
    using unicode superscript characters.
    
    For instance, ``unicodesuperscriptcounter(17) == "¹⁷"``.
    """
    return customdigitscounter(n, digits=_unicodesuperscriptdigits)

def unicodesubscriptcounter(n):
    r"""
    Return a unicode string representation of the number `n`, in base 10,
    using unicode subscript characters.
    
    For instance, ``unicodesubscriptcounter(17) == "₁₇"``.
    """
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
r"""
Dictionary providing standard counter formatters by name.  Some names
mirror their LaTeX counter formatter counterparts (e.g., 'arabic', 'Roman').

The value of the dictionary is a callable (function or lambda) that takes a
positional argument (the number) and returns a string representation of that
number.
"""

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

def _replace_dollar_template_use_callable(x, get_var_value):
    return _rx_dollar_template.sub(lambda m: get_var_value(m.group(1)), x)

def _replace_dollar_template_delayed(x, vrs):
    return lambda arg, numprefix=None: (
        (numprefix or '') +
        _rx_dollar_template.sub(lambda m: vrs[m.group(1)] (arg) , x)
    )




def parse_counter_format_num(
        counter_formatter,
        named_counter_formatters=standard_counter_formatters,
        str_use_tag_template=False,
        tag_template_initials_counters=_standard_tag_template_initials_formatters,
):
    r"""
    Doc...........
    """
    #
    # TODO, deprecate this function by this name (see below) -- rename to
    # `parse_counter_format_num()` & document it under that name below.
    #

    if callable(counter_formatter):
        return counter_formatter
    if isinstance(counter_formatter, str):
        if counter_formatter in named_counter_formatters:
            return lambda n, numprefix=None: (
                (numprefix or '') + named_counter_formatters[counter_formatter](n)
            )
        if str_use_tag_template:
            return _parse_counter_formatter_from_tag_template(
                counter_formatter,
                tag_template_initials_counters
            )

    counter_formatter_template = None
    try:
        # avoid isinstance(counter_formatter, dict) in case counter_formatter is a
        # raw JS object in Transcrypt...
        counter_formatter_template = counter_formatter['template']
    except:
        pass
    if counter_formatter_template:
        tmpl = counter_formatter['template']
        # simple template parsing ${arabic}
        return _replace_dollar_template_delayed(tmpl, named_counter_formatters)

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
        return lambda n, numprefix=None: (
            (numprefix or '') + (left + counter_formatter(n) + right)
        )

    # no counter. E.g., a bullet symbol
    return lambda n, numprefix=None: tag_template



def parse_counter_format_subnum(format_subnum):
    if callable(format_subnum):
        return format_subnum

    if 'format_num' in format_subnum and 'prefix' in format_subnum:
        _format_num_fn = parse_counter_format_num(format_subnum['format_num'])
    else:
        _format_num_fn = parse_counter_format_num(format_subnum)

    def _format_subnum_fn(n):
        return {
            'formatted': _format_num_fn(n),
            'prefix': format_subnum['prefix']
        }
    return _format_subnum_fn



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




def build_counter_formatter(counter_formatter, default_counter_formatter_spec, *,
                            counter_formatter_id):
    r"""
    Build a :py:class:`CounterFormatter` instance from the given
    configuration counter_formatter.  It can be a string (e.g. 'arabic') or a
    dictionary (doc......................)
    """

    if isinstance(counter_formatter, CounterFormatter):
        f = counter_formatter
        if f.counter_formatter_id is None:
            f.counter_formatter_id = counter_formatter_id
        return f

    if counter_formatter is None:
        f = CounterFormatter(**default_counter_formatter_spec)
        if f.counter_formatter_id is None:
            f.counter_formatter_id = counter_formatter_id
        return f

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


_rx_safenumprefix = re.compile(r'[^A-Za-z0-9_-]+')

class ValueWithSubNums:
    def __init__(self, value, subnums=()):
        self.values_tuple = None
        if hasattr(value, 'values_tuple'):
            self.values_tuple = tuple(value.values_tuple)
        else:
            # avoid "tuple([_expect_int(value), *subnums])" because of issues with Transcrypt.
            values_list = [_expect_int(value)]
            values_list += list(subnums)
            self.values_tuple = tuple(values_list)

    def get_num(self):
        return self.values_tuple[0]
    def get_subnums(self):
        return self.values_tuple[1:]

    def astuple(self):
        return self.values_tuple

    def targetidstr(self, numprefix=None):
        return (
            _rx_safenumprefix.sub('-', numprefix or '')
            + "-".join([str(x) for x in self.values_tuple])
        )

    def does_immediately_succeed(self, val2):
        val2p1 = list(val2.values_tuple)
        val2p1[len(val2p1)-1] += 1
        return ( self.values_tuple == tuple(val2p1) )

    def equals(self, val2):
        return self.values_tuple == val2.values_tuple

    def incremented(self, subnum_level=None):

        if subnum_level is None:
            subnum_level = len(self.values_tuple)-1

        new_values_list = list(self.values_tuple)
        new_values_list[subnum_level] += 1
        for j in range(subnum_level+1, len(new_values_list)):
            new_values_list[j] = 0

        return ValueWithSubNums(new_values_list[0], new_values_list[1:])

    def extended_by_one(self, subnum_value=0):
        new_value = ValueWithSubNums(self)
        new_value.values_tuple = tuple(list(new_value.values_tuple) + [ subnum_value ])
        return new_value

    def __repr__(self):
        return "V{"+repr(self.values_tuple)+"}"

        


#__pragma__('skip')
def _sorted_values(a):
    return sorted(a, key=lambda v: v.values_tuple)
#__pragma__('noskip')

#__pragma__("js", "{}", "var _lexicographical_array_cmp = (a, b) => { for (let i = 0; i < a.length && i < b.length; ++i) { if (a[i] < b[i]) { return -1; } if (a[i] > b[i]) { return +1; } } return a.length - b.length; }")
#__pragma__("js", "{}", "var _sorted_values = (vals) => { let va = [...vals]; va.sort( (a, b) => _lexicographical_array_cmp(a.values_tuple, b.values_tuple) ); return va; };")



class CounterFormatter:
    r"""
    Engine to format one or more counter values, including
    prefixes/suffixes (with plural forms), hyperlinks, labels, ranges, and
    conjuctions.

    E.g. ``Equation (2)``, ``Equations (3)–(5)``.

    FIXME: Document me!  Doc.......................
    """

    def __init__(self, format_num, prefix_display=None, 
                 delimiters=None, join_spec=None, name_in_link=True,
                 repeat_numprefix_in_range=False,
                 counter_formatter_id=None,
                 subnums_format_nums=(),
                 ):

        self.format_num = format_num
        self._format_num_fn = parse_counter_format_num(format_num)
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
        self.repeat_numprefix_in_range = repeat_numprefix_in_range
        self.subnums_format_nums = subnums_format_nums
        self._subnums_format_nums_fns = [
            parse_counter_format_subnum(format_subnum)
            for format_subnum in self.subnums_format_nums
        ]

        self.counter_formatter_id = counter_formatter_id

        # note that if the format_num arg of the constructor is a method,
        # then the field format_num is that function and this object cannot
        # be serialized.  To serialize this method, better pass a template dict
        # like {'template': ...}.
        self._fields = (
            'format_num',
            'prefix_display', 'delimiters', 'join_spec',
            'name_in_link', 'repeat_numprefix_in_range',
            'counter_formatter_id',
            'subnums_format_nums',
        )

    def format_number(self, n, numprefix=None, subnums=None):
        s = ''
        skipprefix = True
        if n is not None:
            s += self._format_num_fn(n, numprefix=numprefix)
            skipprefix = False
        if subnums and len(subnums):
            subnums_format_nums = self.get_subnums_format_nums(
                n, numprefix=numprefix, subnums=subnums
            )
            for j in range(len(subnums)):
                if subnums[j]:
                    sfmtted = subnums_format_nums[j] ( subnums[j] )
                    if not skipprefix:
                        s += sfmtted['prefix']
                    s += sfmtted['formatted']
                    skipprefix = False
        return s

    def get_subnums_format_nums(self, n, *, numprefix=None, subnums=None):
        return self._subnums_format_nums_fns

    def asdict(self):
        return {k: getattr(self, k) for k in self._fields}

    def __repr__(self):
        return (
            f"{self.__class__.__name__}("
            + ", ".join(f"{k}={repr(getattr(self,k))}" for k in self._fields)
            + ")"
        )

    def format_flm(self, value, numprefix=None, subnums=None,
                   prefix_variant=None, with_delimiters=True, with_prefix=True,
                   wrap_format_num=None, wrap_link_fn=None):
        r"""
        Doc.......
        
        Value is an integer, any numprefix should be specified separately via
        the `numprefix` arg. ..............
        """
        prefix, pre, post = self._get_format_pre_post(
            with_delimiters,
            with_prefix,
            1,
            prefix_variant,
        )

        if isinstance(value, ValueWithSubNums):
            if subnums is not None:
                raise ValueError(
                    f"format_flm(): cannot specify both "
                    f"ValueWithSubNums instance and subnums= argument; "
                    f"got format_flm({repr(value)}, subnums={repr(subnums)})"
                )
            value, subnums = value.get_num(), value.get_subnums()

        s_num = self.format_number(value, numprefix=numprefix, subnums=subnums)
        if wrap_format_num is not None:
            s_num = wrap_format_num(s_num)
        s = prefix + pre + s_num + post
        if wrap_link_fn is not None:
            return wrap_link_fn(n=value, s=s, numprefix=numprefix, subnums=subnums)
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


    def format_many_flm(self, values,
                        prefix_variant=None, with_delimiters=True,
                        with_prefix=True, wrap_link_fn=None,
                        wrap_format_num=None, get_raw_s_items=False, s_items_join=None):
        r"""
        DOC..........
        
        Here, values may include a "numprefix" and "subnums".....

        values = [list of values...]
        values = [(numprefix, [list of values...]), (numprefix2, [...]), ...]
        
        also each [list of values...] can be a list that contains ints but also
        ValueWithSubNums() instances with sub-numbers.
        """

        join_spec = self.join_spec
        name_in_link = self.name_in_link

        if s_items_join is None:
            s_items_join = lambda a, b: a + b

        # print("***DEBUG: values=", values)

        values = list(values) # make sure we collect any items of some iterable

        if len(values) == 0:
            return join_spec['empty']

        if not isinstance(values[0], (int,ValueWithSubNums)) and len(values[0]) == 2 \
           and (isinstance(values[0][0], str) or values[0][0] is None):
            # `values` is a list of values sorted by numprefix.  All good,
            # continue.
            pass
        else:
            # pack values into a single dummy numprefix 'None'.
            values = [(None, values)]

        # ### Update: This comment now longer relevant given that we deal with
        # ### ValueWithSubNums instances.
        #
        # The key= in sorted() appears to be required for Transcrypt(), so that
        # a comparison function is provided in the JS code, otherwise
        # JavaScript's sort() converts to string and sorts alphabetically ... :/
        values = [
            (valuenumprefix, _sorted_values([ValueWithSubNums(v) for v in valuelist]))
            for (valuenumprefix, valuelist) in values
        ]

        # print("***DEBUG: sanitized values = ", repr(values))

        num_values = sum([len(valuelist) for _, valuelist in values])

        only_one_value = False
        if num_values == 1:
            only_one_value = True

        list_of_ranges_with_numprefix = []
        for numprefix, valuelist in values:
            cur_range = None
            for v in valuelist:
                if cur_range is None:
                    cur_range = (v, v)
                    continue
                if v.does_immediately_succeed(cur_range[1]):
                    cur_range = (cur_range[0], v)
                    continue
                list_of_ranges_with_numprefix.append( (numprefix, cur_range) )
                cur_range = (v, v)
            list_of_ranges_with_numprefix.append( (numprefix, cur_range) )

        if len(list_of_ranges_with_numprefix) == 1:
            numprefix, single_range = list_of_ranges_with_numprefix[0]
            if single_range[1].does_immediately_succeed(single_range[0]):
                # single pair of consecutive values -> format as pair of values,
                # each single value seen as a "range"
                list_of_ranges_with_numprefix = [
                    (numprefix, (single_range[0],single_range[0])),
                    (numprefix, (single_range[1],single_range[1])),
                ]

        def _format_val(val, *, numprefix, range_from=None):
            n = val.values_tuple[0]
            subnums = tuple(val.values_tuple[1:])
            if range_from is not None:
                numprefix = None
                if range_from.values_tuple[0] == n:
                    n = None
                v0subnums = range_from.values_tuple[1:]
                for j in range(len(subnums)):
                    if subnums[j] == v0subnums[j]:
                        subnums[j] = None
            if wrap_format_num is not None:
                return wrap_format_num(
                    self.format_number(
                        n, numprefix=numprefix, subnums=subnums
                    ),
                    numprefix=numprefix, subnums=subnums
                )
            return self.format_number(n, numprefix=numprefix, subnums=subnums)

        def _render_range_items(a, b, numprefix):
            if a.equals(b):
                return [ { 's': _format_val(a, numprefix=numprefix), 'n': a, 'np': numprefix } ]
            is_pairmid = b.does_immediately_succeed(a)
            s_a = _format_val(a, numprefix=numprefix)
            s_b = _format_val(
                b,
                numprefix=numprefix,
                range_from=(a if not (is_pairmid or self.repeat_numprefix_in_range) else None)
            )
            if is_pairmid:
                mid = join_spec['range_pairmid']
            else:
                mid = join_spec['range_mid']
            return [
                { 's': join_spec['range_pre'], 'n': False },
                { 's': s_a, 'n': a, 'np': numprefix },
                { 's': mid, 'n': False },
                { 's': s_b, 'n': b, 'np': numprefix },
                { 's': join_spec['range_post'], 'n': False },
            ]

        if len(list_of_ranges_with_numprefix) == 1:
            numprefix, single_range = list_of_ranges_with_numprefix[0]
            s_items = (
                [ { 's': join_spec['one_pre'], 'n': None } ]
                + _render_range_items(*single_range, numprefix=numprefix)
                + [ { 's': join_spec['one_post'], 'n': None } ]
            )
        elif len(list_of_ranges_with_numprefix) == 2:
            first_numprefix, first_range = list_of_ranges_with_numprefix[0]
            second_numprefix, second_range = list_of_ranges_with_numprefix[1]
            s_items = (
                [ { 's': join_spec['pair_pre'], 'n': False } ]
                + _render_range_items(*first_range, numprefix=first_numprefix)
                + [ { 's': join_spec['pair_mid'], 'n': False } ]
                + _render_range_items(*second_range, numprefix=second_numprefix)
                + [ { 's':  join_spec['pair_post'], 'n': False } ]
            )
        else:
            s_items = [ { 's': join_spec['list_pre'], 'n': False } ]
            for rngj, rnginfo in enumerate(list_of_ranges_with_numprefix[:-1]):
                numprefix, rng = rnginfo
                if rngj > 0:
                    s_items += [ { 's': join_spec['list_mid'], 'n': False } ]
                s_items += _render_range_items(*rng, numprefix=numprefix)
            last_numprefix, last_range = \
                list_of_ranges_with_numprefix[len(list_of_ranges_with_numprefix)-1]
                # ^^^ unsure if Transcryprt accepts [-1].
            s_items += (
                [ { 's': join_spec['list_midlast'], 'n': False } ]
                + _render_range_items(*last_range, numprefix=last_numprefix)
                + [ { 's': join_spec['list_post'], 'n': False } ]
            )

        s_prefix, s_pre, s_post = self._get_format_pre_post(
            with_delimiters, with_prefix, num_values, prefix_variant
        )

        first_n = None
        first_numprefix = None
        if not name_in_link:
            first_n = False
        else:
            for si in s_items:
                nn = si.get('n', None)
                if nn is not None and nn is not False:
                    first_n = nn
                    first_numprefix = si['np']
                    break

        s_pre_items = []
        if len(s_prefix):
            s_pre_items.append( { 's': s_prefix, 'n': first_n, 'np': first_numprefix } )
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
            cur_np = None
            for s_item in s_items:
                si = s_item['s']
                ni = s_item.get('n', None)
                np = s_item.get('np', None)
                if ni is False and cur_n is False and cur_s is not None:
                    cur_s = s_items_join(cur_s, si)
                    continue
                if cur_n is not False and ni is not False and (
                        ni is None or cur_n is None
                        or (ni == cur_n and _eqfornone(np, cur_np))
                ):
                    if ni is not None and cur_n is None:
                        cur_n = ni
                        cur_np = np
                    # add to current link
                    if cur_s is None:
                        cur_s = si
                    else:
                        # don't use += in case the type is mutable, e.g., a nodelist!
                        cur_s = s_items_join(cur_s, si)
                    continue
                # end link here
                if cur_s is not None:
                    s_all.append({'s': cur_s, 'n': cur_n, 'np': cur_np})
                # start anew
                cur_s = si
                cur_n = ni
                cur_np = np

            if cur_s is not None:
                s_all.append({'s': cur_s, 'n': cur_n, 'np': cur_np})

            s_items = s_all

        # print('compressed s_items = ', s_items) # DEBUG

        if get_raw_s_items:
            return s_items

        if wrap_link_fn is not None:
            def _wrap_link_fn_call(x):
                n = x['n'].values_tuple[0]
                subnums = x['n'].values_tuple[1:]
                return wrap_link_fn(n=n, s=x['s'], numprefix=x['np'], subnums=subnums)
            s = "".join([
                ( _wrap_link_fn_call(x)
                  if (x['n'] is not None and x['n'] is not False)
                  else  x['s'] )
                for x in s_items
            ])
        else:
            # ignore link information
            s = "".join([ x['s'] for x in s_items ])

        return s


def _eqfornone(a, b):
    return (
        ((a is None) and (b is None))
        or
        (a == b)
    )


def _expect_int(v):
    try:
        return int(v)
    except TypeError:
        raise ValueError("Invalid value, expected integer: " + repr(v))



