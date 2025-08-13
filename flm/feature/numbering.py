import json
import logging
logger = logging.getLogger(__name__)

from ._base import Feature

from ..counter import CounterFormatter, build_counter_formatter


# --------------------------------------


class Counter:
    r"""
    A basic counter that can be formatted using a
    :py:class:`CounterFormatter`.
    """

    def __init__(self, counter_formatter, initial_value=0):
        self.formatter = counter_formatter
        self.value = initial_value
        
    def set_value(self, value):
        self.value = value
        return self.value

    def step(self):
        self.value += 1
        return self.value

    def reset(self):
        self.value = self.initial_value
        return self.value

    def format_flm(self, value=None, **kwargs):
        if value is None:
            value = self.value
        kwargs2 = {'with_prefix': False}
        kwargs2.update(kwargs)
        return self.formatter.format_flm(value, **kwargs2)

    def step_and_format_flm(self):
        val = self.step()
        return val, self.format_flm(val)


class CounterAlias:
    r"""
    Looks like a :py:class:`Counter`, but always reflects the value stored
    in another given :py:class:`Counter` instance.  The formatter can be set
    independently of the reference counter's formatter.
    """

    def __init__(self, counter_formatter, alias_counter):
        self.formatter = counter_formatter
        self.alias_counter = alias_counter

    @property
    def value(self):
        return self.alias_counter.value
        
    def step(self):
        return self.alias_counter.step()

    def reset(self):
        return self.alias_counter.reset()

    def format_flm(self, value=None, **kwargs):
        if value is None:
            value = self.value
        kwargs2 = {'with_prefix': False}
        kwargs2.update(kwargs)
        return self.formatter.format_flm(value, **kwargs2)

    def step_and_format_flm(self):
        val = self.step()
        return val, self.format_flm(val)





# ----------------------------------------------------------


class _CounterIface:
    def __init__(self, counter_name, simple_counter=None, numbering_render_manager=None):
        self.counter_name = counter_name
        self.simple_counter = simple_counter
        self.numbering_render_manager = numbering_render_manager

    def register_item(self, custom_label=None):
        if self.numbering_render_manager is not None:
            return self.numbering_render_manager.register_item(
                counter_name, custom_label=custom_label
            )
        if custom_label is not None:
            return {....None, custom_label}
        return {....self.simple_counter.step_and_format_flm()}


def get_document_render_counter(
        render_context, counter_name, counter_formatter, alias_counter=None
):
    
    if not render_context.supports_feature('numbering'):
        if alias_counter is not None:
            return _CounterIface(CounterAlias(
                counter_formatter, alias_counter,
                counter_formatter_id=counter_name,
            ))
        return _CounterIface(Counter(
            counter_formatter,
            counter_formatter_id=counter_name
        ))

    numbering_mgr = render_context.feature_render_manager('numbering')

    return numbering_mgr.register_counter(
        counter_name, counter_formatter, alias_counter=alias_counter
    )


# ----------------------------------------------------------


# This FLM feature's only job is to assign numbers to individual instances of
# equations, floats, section headings, etc.  Numbers are assigned at render
# time.




class _DocCounterState:
    def __init__(self, formatter, rdr_mgr,
                 use_doc_state_keys,
                 numprefix_for_doc_state,
                 get_override_formatter_for_doc_state):

        self.formatter = formatter
        self.rdr_mgr = rdr_mgr

        self.counter_by_filtered_doc_states = {}
        # values are { 'value': 0, 'numprefix': None|'Blah-', 'formatter': <object>, }

        self.cur_counter_state = None
        # { 'value': 0, 'numprefix': None|'Blah-', 'formatter': <object>, }

        self.use_doc_state_keys = list(use_doc_state_keys)
        self.numprefix_for_doc_state = numprefix_for_doc_state
        self.get_override_formatter_for_doc_state = get_override_formatter_for_doc_state

    def get_filtered_doc_state(self):
        r"""
        A unique string that changes each time the relevant part of the doc
        state changes.  When this string changes, we will reset the counter.
        """
        rs = self.rdr_mgr.render_doc_states
        return json.dumps([rs[k] for k in self.use_doc_state_keys])

    def register_item(self):
        new_filtered_doc_state = self.get_filtered_doc_state()
        if new_filtered_doc_state == self.cur_filtered_doc_state:
            # no document state changes, can simply increase our current counter
            # state
            self.cur_counter_state['value'] += 1
        elif new_filtered_doc_state in self.counter_by_filtered_doc_states:
            # Document state has changed for the state keys that we're
            # monitoring, but we've seen this state before (e.g. moved out of a
            # subequations state).  Go back to the counter in that state.
            self.cur_filtered_doc_state = new_filtered_doc_state
            self.cur_counter_state = self.counter_by_filtered_doc_states[new_filtered_doc_state]
            self.cur_counter_state['value'] += 1
        else:
            # Document state has changed for the state keys that we're
            # monitoring (e.g. moved on to next section), and we haven't seen
            # this state yet.  Need to reset counter
            self.cur_filtered_doc_state = new_filtered_doc_state
            if self.numprefix_for_doc_state:
                cur_numprefix = self.numprefix_for_doc_state(self.rdr_mgr.render_doc_states)
            else:
                cur_numprefix = None
            if self.get_override_formatter_for_doc_state is not None:
                cur_formatter = self.get_override_formatter_for_doc_state(
                    self.rdr_mgr.render_doc_states
                )
            new_counter_state = {
                'value': 1,
                'numprefix': cur_numprefix,
                'formatter': cur_formatter,
            }
            self.counter_by_filtered_doc_states[new_filtered_doc_state] = new_counter_state
            self.cur_counter_state = new_counter_state

        cur_value = self.cur_counter_state['value']
        cur_numprefix = self.cur_counter_state['numprefix']
        cur_formatter = self.cur_counter_state['formatter']
        formatted_value = cur_formatter.format_flm(cur_value, numprefix=cur_numprefix)
        return {
            'value': cur_value,
            'numprefix': cur_numprefix,
            'formatter': cur_formatter,
            'formatted_value': formatted_value,
        }


class FeatureNumbering(Feature):

    feature_name = 'numbering'
    feature_title = 'Numbering for figures, sections, equations, theorems, and more'

    class RenderManager(Feature.RenderManager):

        def initialize(self, number_within=None):
            self.counters = {}
            self.number_within = number_within
            
            self.render_doc_states = dict()

            # number_within = {
            #   'equation': {'reset_at': 'subsection', 'numprefix': '${subsection}.'}
            # }

        def register_counter(
                self,
                counter_name,
                counter_formatter,
                *
                alias_counter=alias_counter,
                use_subcounter_doc_states=None,
        ):
            # use_subcounter_doc_states = None | { 'doc_state_keys': ..., 'sub_formatter':  }

            if counter_name in self.counters:
                raise ValueError(f"Counter ‘{counter_name}’ already registered!")

            reset_doc_state_keys = []
            if counter_name in number_within:
                parent_counter = number_within[counter_name]['reset_at']
                reset_doc_state_keys.append( f'cnt-{parent_counter}' )

            if use_subcounter_doc_states is not None:
                

            def numprefix_for_doc_state(rs):
                

            self.counters[counter_name] = _DocCounterState(
                formatter=counter_formatter,
                rdr_mgr=self,
                reset_doc_state_keys=reset_doc_state_keys,
                
            )

        def set_render_doc_state(self, state_type, state_value):
            self.render_doc_states[state_type] = state_value

        def clear_render_doc_state(self, state_type):
            del self.render_doc_states[state_type]

        def register_item(self, counter_name, custom_label=None):
            

        def get_citation_content_flm(self, cite_prefix, cite_key, resource_info):

            if self.external_citations_providers is None:
                raise ValueError("No external citations providers are set!")

            # retrieve citation from citations provider --
            citation_flm_text = None
            for external_citations_provider in self.external_citations_providers:
                citation_flm_text = \
                    external_citations_provider.get_citation_full_text_flm(
                        cite_prefix, cite_key,
                        resource_info
                    )
                if citation_flm_text:
                    break
            
            if citation_flm_text is None:
                raise ValueError(f"Citation not found: ‘{cite_prefix}:{cite_key}’")

            if isinstance(citation_flm_text, FLMFragment):
                citation_flm = citation_flm_text
            else:
                citation_flm = self.render_context.make_standalone_fragment(
                    citation_flm_text,
                    is_block_level=False,
                    what=f"Citation text for {cite_prefix}:{cite_key}",
                )

            #logger.debug("Got citation content FLM nodelist = %r", citation_flm.nodes)

            return citation_flm
            

        def get_citation_endnote(self, cite_prefix, cite_key, resource_info):
            endnotes_mgr = None
            if not self.use_endnotes:
                return None

            endnotes_mgr = self.render_context.feature_render_manager('endnotes')

            if (cite_prefix, cite_key) in self.citation_endnotes:
                return self.citation_endnotes[(cite_prefix, cite_key)]

            citation_flm = self.get_citation_content_flm(cite_prefix, cite_key,
                                                         resource_info)

            endnote = endnotes_mgr.add_endnote(
                category_name='citation', 
                content_nodelist=citation_flm.nodes,
                ref_label_prefix=cite_prefix,
                ref_label=cite_key,
                node=(cite_prefix,cite_key),
            )

            # also add a custom field, the formatted inner counter text (e.g.,
            # "1" for citation "[1]").  It'll be useful for combining a citation
            # number with an optional text as in [31; Theorem 4].
            endnote.formatted_inner_counter_value_flm = \
                self.render_context.make_standalone_fragment(
                    self.feature_document_manager.endnote_category.counter_formatter.format_flm(
                        endnote.number,
                        with_delimiters=False
                    ),
                    is_block_level=False,
                    what=f"citation counter (inner)",
                )

            self.citation_endnotes[(cite_prefix, cite_key)] = endnote

            return endnote

        # -----

        def render_citation_marks(self, cite_items, node):

            render_context = self.render_context
            fragment_renderer = render_context.fragment_renderer

            endnotes_mgr = None
            if render_context.supports_feature('endnotes'):
                endnotes_mgr = render_context.feature_render_manager('endnotes')

            if (self.use_endnotes and endnotes_mgr is not None
                and endnotes_mgr.inhibit_render_endnote_marks):
                #
                return fragment_renderer.render_nothing(render_context)


            resource_info = node.latex_walker.resource_info

            #
            # First pass -- sort out any citations that have an optional cite extra
            # text (as in "\cite[Theorem 3, p.54]{Key}").
            #
            citations_compressible = []
            citations_manual_render = []
            for cd in cite_items:
                citation_key_prefix, citation_key, extra = cd['prefix'], cd['key'], cd['extra']

                endnote = None
                if self.use_endnotes:
                    endnote = self.get_citation_endnote(
                        citation_key_prefix,
                        citation_key,
                        resource_info
                    )

                if extra is None:
                    citations_compressible.append(
                        (citation_key_prefix, citation_key, extra, endnote)
                    )
                if extra is not None:
                    citations_manual_render.append(
                        (citation_key_prefix, citation_key, extra, endnote)
                    )

            #
            # Render citation list for those citations without any additional extra
            # text ("compressible") using our endnote mark renderer.  This will
            # automatically sort & compress citation ranges.
            #

            s_items = []

            delimiters_part_of_link = True

            if self.use_endnotes and self.sort_and_compress:

                endnote_numbers = [
                    endnote for (key_prefix, key, extra, endnote) in citations_compressible
                ]
                if len(endnote_numbers) > 1:
                    delimiters_part_of_link = False

                rendered_citations_woextra = endnotes_mgr.render_endnote_mark_many(
                    endnote_numbers,
                    wrap_with_semantic_span=False
                )

                logger.debug("rendered_citations_woextra = %r", rendered_citations_woextra)

                s_items.append(rendered_citations_woextra)
            else:
                # otherwise, simply render the "compressible" citations along with
                # the other ones
                citations_manual_render = citations_compressible + citations_manual_render

            #
            # Render any further citations.  These are either full text citaions
            # because we're not using endnotes, or they are citations with
            # additional extra text ("Theorem 3, p.54").
            #

            citation_delimiters = self.feature.counter_formatter.delimiters

            for cite_item in citations_manual_render:

                (citation_key_prefix, citation_key,
                 optional_cite_extra_nodelist, endnote) = cite_item

                citation_content_flm = None
                show_inline_content_flm = None
                if self.use_endnotes:
                    show_inline_content_flm = endnote.formatted_inner_counter_value_flm
                else:
                    citation_content_flm = self.get_citation_content_flm(
                        citation_key_prefix,
                        citation_key,
                        resource_info
                    )
                    show_inline_content_flm = citation_content_flm

                # don't use endnotes_mgr.render_endnote_mark(endnote) because it
                # can't render the optional citation text.  Form the citation mark
                # ourselves, using the citation delimiters etc.
                cite_content_list_of_nodes = []

                # Don't necessarily make the citation delimiters themselves part of the link
                if delimiters_part_of_link and citation_delimiters[0] is not None:
                    cite_content_list_of_nodes.append(
                        node.latex_walker.make_node(
                            latexnodes_nodes.LatexCharsNode,
                            chars=citation_delimiters[0],
                            pos=node.pos,
                            pos_end=node.pos_end,
                            parsing_state=node.parsing_state,
                        )
                    )

                # list() apparently needed for transcrypt ... :/ -->
                cite_content_list_of_nodes.extend( list(show_inline_content_flm.nodes) )
                if optional_cite_extra_nodelist is not None:
                    cite_content_list_of_nodes.append(
                        node.latex_walker.make_node(
                            latexnodes_nodes.LatexCharsNode,
                            chars=self.feature.citation_optional_text_separator,
                            pos=node.pos,
                            pos_end=node.pos_end,
                            parsing_state=node.parsing_state,
                        )
                    )
                    # list() apparently needed for transcrypt ... :/ -->
                    cite_content_list_of_nodes.extend( list(optional_cite_extra_nodelist) )

                # Don't necessarily make the citation delimiters themselves part of the link
                if delimiters_part_of_link and citation_delimiters[1] is not None:
                    cite_content_list_of_nodes.append(
                        node.latex_walker.make_node(
                            latexnodes_nodes.LatexCharsNode,
                            chars=citation_delimiters[1],
                            pos=node.pos,
                            pos_end=node.pos_end,
                            parsing_state=node.parsing_state,
                        )
                    )

                citation_nodes_parsing_state = node.parsing_state.sub_context(
                    is_block_level=False,
                )

                display_nodelist = node.latex_walker.make_nodelist(
                    cite_content_list_of_nodes,
                    parsing_state=citation_nodes_parsing_state,
                )

                if self.use_endnotes:

                    full_cite_mark = endnotes_mgr.render_endnote_mark(
                        endnote, display_nodelist,
                        wrap_with_semantic_span=False,
                    )

                    if not delimiters_part_of_link:
                        full_cite_mark = \
                            citation_delimiters[0] + full_cite_mark + citation_delimiters[1]

                    s_items.append( full_cite_mark )

                else:

                    full_inline_citation = fragment_renderer.render_nodelist(
                        display_nodelist,
                        render_context
                    )

                    if not delimiters_part_of_link:
                        full_inline_citation = (
                            citation_delimiters[0] + full_inline_citation
                            + citation_delimiters[1]
                        )

                    s_items.append( full_inline_citation )

            return fragment_renderer.render_semantic_span(
                fragment_renderer.render_join(s_items, render_context),
                'citation-marks',
                render_context,
            )


    def __init__(self,
                 external_citations_providers,
                 counter_formatter='arabic',
                 citation_delimiters=None,
                 citation_optional_text_separator="; ",
                 references_heading_title='References',
                 sort_and_compress=True
                 ):
        super().__init__()
        self.external_citations_providers = external_citations_providers
        dflt = dict(_cite_default_counter_formatter_spec)
        if citation_delimiters is not None:
            dflt['delimiters'] = citation_delimiters
        self.counter_formatter = build_counter_formatter(
            counter_formatter,
            dflt,
            counter_formatter_id='citation',
        )
        #self.citation_delimiters = citation_delimiters
        self.citation_optional_text_separator = citation_optional_text_separator
        self.references_heading_title = references_heading_title
        self.sort_and_compress = sort_and_compress

    def set_external_citations_providers(self, external_citations_providers):
        if self.external_citations_providers is not None:
            logger.warning(
                "FeatureExternalPrefixedCitations.set_external_citations_providers(): "
                "There are already external citation providers set.  They will be replaced."
            )
        self.external_citations_providers = external_citations_providers

    def add_external_citations_provider(self, external_citations_provider):
        if self.external_citations_providers is None:
            logger.warning(
                "FeatureExternalPrefixedCitations.add_external_citations_provider(): "
                "External citations provider list was not initialized, creating an empty list."
            )
            self.external_citations_providers = []

        self.external_citations_providers.append( external_citations_provider )

    def add_latex_context_definitions(self):
        return {
            'macros': [
                CiteMacro('cite',),
            ]
        }
