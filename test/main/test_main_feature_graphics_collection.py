import unittest

import io
import os
import os.path
import base64
import tempfile

from flm.main.main import main
from flm.main.feature_graphics_collection import FeatureGraphicsCollection


def _make_png_data_url(width=10, height=20, dpi=96):
    import PIL.Image
    img = PIL.Image.new('RGB', (width, height), (255, 0, 0))
    buf = io.BytesIO()
    img.save(buf, format='PNG', dpi=(dpi, dpi))
    b64 = base64.b64encode(buf.getvalue()).decode('ascii')
    return 'data:image/png;base64,' + b64


# ---------------------------------------------------------------------------
#  Feature.fetch_inspect_url — download + inspect + memoize
# ---------------------------------------------------------------------------

class TestFetchInspectUrl(unittest.TestCase):

    def test_data_url_inspected(self):
        data_url = _make_png_data_url(width=10, height=20)
        feat = FeatureGraphicsCollection()
        entry = feat.fetch_inspect_url(data_url)

        self.assertEqual(entry['detected_ext'], '.png')
        self.assertEqual(entry['mimetype'], 'image/png')
        self.assertIsNotNone(entry['temp_file_path'])
        self.assertTrue(entry['temp_file_path'].endswith('.png'))
        self.assertTrue(os.path.exists(entry['temp_file_path']))
        self.assertIsNotNone(entry['input_hash'])

        # the inspector found the dimensions of the PNG
        self.assertEqual(entry['info']['pixel_dimensions'], (10, 20))
        self.assertTrue('physical_dimensions' in entry['info'])
        w_pt, h_pt = entry['info']['physical_dimensions']
        # 10px / 96dpi * 72 ; 20px / 96dpi * 72 (allow for dpi rounding)
        self.assertTrue(abs(w_pt - (10 / 96) * 72) < 1e-1)
        self.assertTrue(abs(h_pt - (20 / 96) * 72) < 1e-1)

    def test_data_url_downloaded_once(self):
        data_url = _make_png_data_url()
        feat = FeatureGraphicsCollection()

        entry1 = feat.fetch_inspect_url(data_url)
        entry2 = feat.fetch_inspect_url(data_url)

        # same cached entry returned, only one download performed
        self.assertIs(entry1, entry2)
        self.assertEqual(len(feat._url_cache), 1)
        self.assertEqual(feat._url_download_counter, 1)

    def test_bad_url_failure_marker(self):
        feat = FeatureGraphicsCollection()
        # malformed data URL -> failure marker cached, no exception raised
        entry = feat.fetch_inspect_url('data:totally-not-valid')
        self.assertIsNone(entry['temp_file_path'])
        self.assertIsNone(entry['input_hash'])
        self.assertEqual(entry['info'], {})
        self.assertTrue('data:totally-not-valid' in feat._url_cache)


# ---------------------------------------------------------------------------
#  End-to-end through main() — collecting and non-collecting renders
# ---------------------------------------------------------------------------

class TestDataUrlRender(unittest.TestCase):

    maxDiff = None

    def test_data_url_collected(self):
        data_url = _make_png_data_url(width=10, height=20)

        with tempfile.TemporaryDirectory() as tmpdir:
            output_path = os.path.join(tmpdir, 'out.html')
            main(
                output=output_path,
                flm_content=(r'\begin{figure}\includegraphics{' + data_url
                          + r'}\end{figure}'),
                format='html',
            )

            collected_dir = os.path.join(tmpdir, '_flm_collected_graphics')
            self.assertTrue(os.path.isdir(collected_dir))

            png_files = [
                fn for fn in os.listdir(collected_dir) if fn.endswith('.png')
            ]
            self.assertEqual(len(png_files), 1)

            collected_path = os.path.join(collected_dir, png_files[0])
            self.assertTrue(os.path.exists(collected_path))

            # the collected file is a valid PNG of the expected size
            import PIL.Image
            img = PIL.Image.open(collected_path)
            self.assertEqual(img.width, 10)
            self.assertEqual(img.height, 20)

    def test_data_url_non_collecting_passthrough(self):
        data_url = _make_png_data_url(width=10, height=20)

        sout = io.StringIO()
        main(
            output=sout,
            flm_content=(r'\begin{figure}\includegraphics{' + data_url
                          + r'}\end{figure}'),
            format='html',
            inline_config=(
                '{"flm": {"features": {"flm.main.feature_graphics_collection":'
                ' {"collect_graphics_to_output_folder": false}}}}'
            ),
        )
        # the data URL survives verbatim in the output
        self.assertTrue(data_url in sout.getvalue())


if __name__ == '__main__':
    unittest.main()
