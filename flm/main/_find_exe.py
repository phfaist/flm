import os
import glob
import shutil

magick_patterns = [
    '/usr/local/bin/magick',
    '/opt/homebrew/bin/magick',
    r"C:\Programs Files*\Image Magick*\**\magick.exe",
]
latexmk_patterns = [
    '/usr/local/texlive/*/bin/*/latexmk',
    '/usr/local/bin/latexmk',
    r'C:\texlive\*\bin\*\latexmk.exe',
    r'C:\Program Files*\MikTeX*\miktex\bin\latexmk.exe'
]
pdftocairo_patterns = [
    '/usr/local/bin/pdftocairo',
    '/opt/homebrew/bin/pdftocairo',
    r"C:\Programs Files*\pdftocairo*\**\pdftocairo.exe", # ???
]
gs_patterns = [
    "/usr/local/bin/gs",
    "/opt/local/bin/gs",
    r"C:\Program Files*\gs*\gs*\bin\gswin*.exe", # ???
    r"C:\texlive\*\bin\win*\gswin*.exe", # ???
    r"C:\Program Files*\MiKTeX*\miktex\bin\mgs.exe", # ???
]


def _find_exe_value(exe_name, std_patterns, var_name):
    if var_name in os.environ:
        if os.environ[var_name] == 'false':
            return None
        return os.environ[var_name]
    for p in std_patterns:
        result = glob.glob(p, recursive=True)
        if len(result):
            return result[0]
    rexe = shutil.which(exe_name) # search in PATH
    if rexe:
        return rexe
    return None

def find_exe(exe_name, std_patterns, var_name, error=True):
    value = _find_exe_value(exe_name, std_patterns, var_name)
    if value:
        return value
    if not error:
        return None
    raise ValueError(f"Cannot find executable ‘{exe_name}’ on your system! "
                     f"Please set {var_name} to its full path.")

std_exe_dict = {
    'magick': [magick_patterns, 'MAGICK'],
    'latexmk': [latexmk_patterns, 'LATEXMK'],
    'pdftocairo': [pdftocairo_patterns, 'PDFTOCAIRO'],
    'gs': [gs_patterns, 'GHOSTSCRIPT'],
}
std_exe_found = {}

def find_std_exe(exe_name, error=True):
    if exe_name in std_exe_found:
        return std_exe_found[exe_name]

    result = find_exe(exe_name, *std_exe_dict[exe_name], error=error)
    std_exe_found[exe_name] = result
    return result
