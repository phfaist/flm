import re

class delayedstr:
    def __init__(self, str_fn):
        self.str_fn = str_fn

    def __repr__(self):
        return self.str_fn()

    def __str__(self):
        return self.str_fn()




class abbrev_value_str:
    def __init__(self, value, *, level=0, **options):
        self.value = value
        self.level = level

        self.options = options
        self.maxlevel = options.get('maxlevel', 5)
        self.maxitems = options.get('maxitems', 15)
        self.maxstrlen = options.get('maxstrlen', 80)
        self.maxitemslinelen = options.get('maxitemslinelen', 60)
        self.indent = options.get('indent', 4) # number of spaces to indent

    def __str__(self):
        v = self.value

        if isinstance(v, dict):
            if self.maxlevel is not None and self.level >= self.maxlevel:
                return '{ … }'

            s_items = []
            for k2, v2 in v.items():
                fmtval2 = str(abbrev_value_str(v2, level=self.level+1, **self.options))
                fmtkey2 = self._fmt_key(k2)
                s_items.append(f"{fmtkey2}: {fmtval2}")

            return self._fmt_list_items(s_items, ('{', ',', '}'))

        if isinstance(v, list):
            if self.maxlevel is not None and self.level >= self.maxlevel:
                return '[ … ]'

            s_items = []
            for v2 in v:
                fmtval2 = str(abbrev_value_str(v2, level=self.level+1, **self.options))
                s_items.append(fmtval2)

            return self._fmt_list_items(s_items, ('[', ',', ']'))

        # format simply as string
        if isinstance(v, str):
            fmtval = v
            if len(fmtval) > self.maxstrlen:
                fmtval = fmtval[:self.maxstrlen] + ' …'

            return repr(fmtval)

        # anything else -- use repr().  Bool, int, float, custom objects.
        return repr(v)
        

    def _fmt_key(self, key):
        if re.match('^[a-zA-Z0-9_]+$', key):
            return key
        return repr(key)

    def _fmt_list_items(self, s_items, s_delim):
        if len(s_items) > self.maxitems:
            s_items[self.maxitems:] = ['…']

        use_multiline = False
        total_len = 0
        for s in s_items:
            if '\n' in s:
                use_multiline = True
                break

            total_len += len(s_delim[1]) + 1 + len(s)
            if total_len > self.maxitemslinelen:
                use_multiline = True
                break

        if use_multiline:
            theindent = (' '*self.indent)*self.level
            theindent2 = (' '*self.indent)*(self.level+1)
            thesep = '\n'
        else:
            theindent = ''
            theindent2 = ''
            thesep = ' '

        return (
            s_delim[0] + thesep + theindent2 +
            (thesep + theindent2).join([f"{theval}{s_delim[1]}" for theval in s_items]) +
            thesep + theindent + s_delim[2]
        )

