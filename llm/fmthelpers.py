
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

def romancounter(n, lower=True):
    s = ''
    n -= 1 # start counting at 1
    for romancharvalue, romanchar in _romancounterchars:
        s += romanchar * (n // romancharvalue)
        n = n % romancharvalue
    if lower:
        return s.lower()
    return s

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
    if lower:
        return s
    return s.upper()
    



standard_counter_formatters = {
    'alph': lambda n: alphacounter(n, lower=True),
    'Alph': lambda n: alphacounter(n, lower=False),
    'roman': lambda n: romancounter(n, lower=True),
    'Roman': lambda n: romancounter(n, lower=False),
    'arabic': lambda n: str(n),
    'fnsymbol': lambda n: fnsymbolcounter(n),
}
