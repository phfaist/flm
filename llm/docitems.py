


class DocumentItem:
    # object that might have a side effect, a label, a float, etc.
    pass


# ------------------


class Equation(DocumentItem):
    r"""
    Represents a numbered equation, e.g., {equation}, {align}, etc.
    """
    def __init__(
            self,
            environment_name,
            label
    ):
        self.environment_name = environment_name
        self.label = label

class Float(DocumentItem):
    def __init__(
            self,
            float_type='figure',
            float_caption_name='Figure',
            caption=None,
            label=None, # label is stored WITHOUT the float_type ("figure:") prefix
            contents=None, # dictionary with information about contents.
    ):
        self.float_type = float_type
        self.float_caption_name = float_caption_name
        self.caption = caption
        self.label = label
        self.contents = contents

class Footnote(DocumentItem):
    def __init__(
            self,
            footnote_content,
    ):
        self.footnote_content = footnote_content


class Citation(DocumentItem):
    def __init__(
            self,
            citation_key_prefix,
            citation_key,
            optional_cite_extra_html=None
    ):
        self.citation_key_prefix = citation_key_prefix
        self.citation_key = citation_key
        self.optional_cite_extra_html = optional_cite_extra_html




# # ----------------------------------------------------------

# class DocumentWithItems:
#     def __init__(self):
#         self.equations = []
#         self.floats = []
#         self.footnotes = []
#         self.citations = []


#     def render_label
