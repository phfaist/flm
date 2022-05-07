import html
import re

from . import fragmentrenderer

class HtmlFragmentRenderer(fragmentrenderer.FragmentRenderer):

    supports_delayed_render_markers = True
    # we use <LLM:DLYD:delayed_key/> marker, which cannot be confused with
    # the rest of the HTML code that can be generated


    use_link_target_blank = False

    # ------------------

    def htmlescape(self, value):
        return html.escape(value)

    def wrap_in_tag(self, tagname, content_html, *, attrs=None, class_names=None):
        s = f'<{tagname}'
        if attrs:
            for aname, aval in dict(attrs).items():
                s += f''' {aname}="{self.htmlescape(aval)}"'''
        if class_names:
            s += f''' class="{self.htmlescape(' '.join(class_names))}"'''
        s += '>'
        s += str(content_html)
        s += f'</{tagname}>'
        return s

    def wrap_in_link(self, display_html, target_href, *, class_names=None):
        attrs = {
            'href': self.htmlescape(target_href)
        }
        if callable(self.use_link_target_blank):
            if self.use_link_target_blank(target_href):
                attrs['target'] = '_blank'
        elif self.use_link_target_blank and not target_href.startswith('#'):
            attrs['target'] = '_blank'
        return self.wrap_in_tag(
            'a',
            display_html,
            attrs=attrs,
            class_names=class_names,
        )

    # -----------------

    def render_join_as_paragraphs(self, paragraphs_content):
        return "\n".join([ f"<p>{p}</p>" for p in paragraphs_content ])

    # ------------------

    def render_value(self, value):
        return self.htmlescape(value)

    def render_empty_error_placeholder(self):
        return "<span class=\"empty-error-placeholder\">(?)</span>"

    def render_nothing(self, annotation):
        annotation = annotation.replace('--', '- - ')
        return '<!-- {} -->'.format(annotation)

    def render_verbatim(self, value, annotation):
        return self.wrap_in_tag(
            'span',
            self.htmlescape(value),
            class_names=[annotation if annotation else 'verbatim']
        )

    def render_math_content(self, delimiters, nodelist, doc, displaytype, environmentname):
        class_names = [ f"{displaytype}-math" ]
        if environmentname is not None:
            class_names.append(f"env-{environmentname.replace('*','-star')}")
        return self.wrap_in_tag(
            'span',
            self.htmlescape( delimiters[0] + nodelist.latex_verbatim() + delimiters[1] ),
            class_names=class_names
        )

    def render_text_format(self, text_formats, content):
        r"""
        The argument `content` is already valid HTML
        """
        return self.wrap_in_tag(
            'span',
            content,
            class_names=text_formats
        )


    def render_link(self, ref_type, href, display_content):
        return self.wrap_in_link(display_content, href, class_names=[ f"href-{ref_type}" ])

    
    def render_delayed_marker(self, node, delayed_key, doc):
        return f"<LLM:DLYD:{delayed_key}/>"

    def render_delayed_dummy_placeholder(self, node, delayed_key, doc):
        return '<!-- delayed:{delayed_key} -->'

    def replace_delayed_markers_with_final_values(self, content, delayed_values):
        return _rx_delayed_markers.sub(
            lambda m: delayed_values[int(m.group('key'))],
            content
        )




_rx_delayed_markers = re.compile(r'\<LLM:DLYD:(?P<key>\d+)\s*\/\>')
