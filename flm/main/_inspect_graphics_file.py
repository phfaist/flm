import re
import os.path



def get_image_file_info(filename):

    if filename.endswith('.svg'):
        # use svg processor
        return get_image_file_info_svg(filename)
    return get_image_file_info_pil(filename)


def get_image_file_info_pil(filename):
    try:
        img = PIL.Image.open(filename)
    except PIL.UnidentifiedImageError:
        logger.critical(
            f"PIL Failed to identify the image {filename}.  If you created this image "
            f"using the GIMP, you can try ‘Image’ → ‘Color Management’ → Disable to work "
            f"around a bug in GIMP's image exporter"
        )
        raise

    width_px, height_px = img.width, img.height

    dpi_x, dpi_y = img.info['dpi']

    if abs(dpi_x - dpi_y) > 1e-2:
        raise ValueError(
            "Your image seems to have different DPI values for the X and Y dimensions: "
            f"({dpi_x!r}, {dpi_y!r}).  I don't know how to handle this.  Please fix "
            "your image so that it has a fixed DPI setting."
        )

    # round up DPI setting a bit (two decimal places' equivalent in binary)
    dpi = int(dpi_x * 128 + 0.5) / 128

    # There are 72 pts in an inch. Don't use 96 here, it's the DPI value that
    # should reflect the value 96 that your googling might have alerted you to.
    width_pt = (width_px / dpi) * 72
    height_pt = (height_px / dpi) * 72

    return {
        'type': 'raster',
        'dpi': dpi,
        'pixel_dimensions': (width_px, height_px),
        'physical_dimensions': ((width_pt, 'pt'), (height_pt, 'pt'))
    }
        


_pt_per_u = {
    # 1 pt = 1 pt
    'pt': 1,

    # SVG pixels should be interpreted at 96DPI so 1 px = 1/(96) in = 72/96 pt
    'px': 72/96,
    
    # 1 in = 72 pt
    'in': 72,
    
    # 1 cm = (1/2.54) in = 72/2.54 pt
    'cm': 72/2.54,

    # 1 mm = (1/25.4) in = 72/25.4 pt
    'mm': 72/25.4,

    # 1 m = (1/0.0254) in = 72/0.0254 pt
    'm': 72/0.0254,
}

_rx_dimen = re.compile(r'^\s*(?P<dimension>[0-9.e+-]+)\s*(?P<unit>'
                       + "|".join(_pt_per_u.keys())
                       + r')?\s*$')



#_source_size_threshold = 1e5   # 100 kB


def get_image_file_info_svg(filename):
    
    tree = ET.parse(filename)
    root = tree.getroot()

    try:
        width = root.attrib['width']
        height = root.attrib['height']
    except KeyError:
        raise ValueError(f"Failed to parse SVG image dimensions in ‘{filename}‘, "
                         f"can't read width/height")

    m_width = _rx_dimen.match(width)
    m_height = _rx_dimen.match(height)
    if m_width is None or m_height is None:
        raise ValueError(f"Failed to parse SVG image dimensions in ‘{filename}‘: "
                         f"{width=!r}, {height=!r}")

    width_unit = m_width.group('unit')
    if not width_unit:
        width_unit = 'pt'

    height_unit = m_height.group('unit')
    if not height_unit:
        height_unit = 'pt'

    width_dimension_u = float(m_width.group('dimension'))
    height_dimension_u = float(m_height.group('dimension'))

    width_pt = width_dimension_u * _pt_per_u[width_unit]
    height_pt = height_dimension_u * _pt_per_u[height_unit]

    d = {
        'type': 'vector',
        'physical_dimensions': (
            ( _uniform_svg_scale * width_pt, 'pt' ),
            ( _uniform_svg_scale * height_pt, 'pt' ),
        )
    }

    # ### Don't paste in the source SVG into the HTML, it's a bad idea (risk of
    # ### ids / resources clashing, etc.)
    #
    #if os.path.getsize(filename) < _source_size_threshold:
    #    # for small enough files, get the SVG source for direct inclusion.
    #    # Reconverting to string should remove "<?xml" tags etc.
    #    d['svg_source'] = ET.tostring(root, encoding='unicode')

    return d
