import re
import os.path
import logging
logger = logging.getLogger(__name__)


def get_image_file_info(filename, fp=None):
    if filename.endswith('.svg'):
        # use svg processor
        return get_image_file_info_svg(filename, fp)
    if filename.endswith('.pdf'):
        # use PDF processor
        return get_image_file_info_pdf(filename, fp)
    return get_image_file_info_pil(filename, fp)


def get_image_file_info_pdf(filename, fp):

    import pypdf

    pdf = pypdf.PdfReader(fp)

    if not pdf.pages or len(pdf.pages) == 0:
        logger.warning(f"PDF ‘{filename}’ has no pages!")
        return None

    if len(pdf.pages) != 1:
        logger.warning(f"PDF ‘{filename}’ has {len(pdf.pages)} pages, only the "
                       f"first one is inspected.")

    page = pdf.pages[0]
    width_uu = page.mediabox.width
    height_uu = page.mediabox.height

    width_pt = page.user_unit * width_uu
    height_pt = page.user_unit * height_uu

    return {
        'graphics_type': 'vector',
        'physical_dimensions': ( width_pt, height_pt ),
    }


def get_image_file_info_pil(filename, fp):

    import PIL
    import PIL.Image

    try:
        img = PIL.Image.open(fp or filename)
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
        'graphics_type': 'raster',
        'dpi': dpi,
        'pixel_dimensions': (width_px, height_px),
        'physical_dimensions': (width_pt, height_pt),
    }
        





# for parsing dimension info in the SVG (but are there really any units in the
# SVG file???)

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

_rx_dimen = re.compile(
    r'^\s*(?P<dimension>[0-9.e+-]+)\s*(?P<unit>'
    + "|".join([ re.escape(unitname) for unitname in _pt_per_u.keys() ])
    + r')?\s*$'
)






def get_image_file_info_svg(filename, fp):
    
    import xml.etree.ElementTree as ET

    tree = ET.parse(fp or filename)
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

    return {
        'graphics_type': 'vector',
        'physical_dimensions': ( width_pt, height_pt ),
    }

