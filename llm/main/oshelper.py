import subprocess, os, platform

# ------------------------------------------------------------------------------

# thanks https://stackoverflow.com/a/435669/1694896
def os_open_file(filepath):

    if platform.system() == 'Darwin':       # macOS
        subprocess.call(('open', filepath))
    elif platform.system() == 'Windows':    # Windows
        os.startfile(filepath)
    else:                                   # linux variants
        subprocess.call(('xdg-open', filepath))


