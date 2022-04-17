import html

from . import fragmentrenderer

class HtmlFragmentRenderer(fragmentrenderer.FragmentRenderer):

    use_link_target_blank = False

    # ------------------

    def htmlescape(self, value):
        return html.escape(value)

    def wrap_in_tag(self, tagname, content_html, *, attrs=None, class_=None):
        s = f'<{tagname}'
        if attrs:
            for aname, aval in dict(attrs).items():
                s += f' {aname}="{self.htmlescape(aval)}"'
        if class_:
            s += f' class="{self.htmlescape(class_)}"'
        s += '>'
        s += str(content_html)
        s += f'</{tagname}>'
        return s

    def wrap_in_link(self, display_html, target_href, *, class_=None):
        attrs = {
            'href': htmlescape(target_href)
        }
        if callable(self.use_link_target_blank):
            if self.use_link_target_blank(target_href):
                attrs['target'] = '_blank'
        elif self.use_link_target_blank:
            attrs['target'] = '_blank'
        return self.wrap_in_tag(
            'a',
            display_html,
            attrs=attrs,
            class_=class_,
        )

    # ------------------

    def render_value(self, value):
        return self.htmlescape(value)

    def render_empty_error_placeholder(self):
        return "<span class=\"empty-error-placeholder\">(?)</span>"

    def render_verbatim(self, value, annotation=None):
        return self.wrap_in_tag(
            'span',
            self.htmlescape(value),
            class_=(annotation if annotation else 'verbatim')
        )

    def render_text_format(self, text_format, value):
        if text_format == 'bold':
            return ......
        if text_format == 'italic':
            return ......

    
