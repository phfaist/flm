import json
import logging
logger = logging.getLogger(__name__)

from ._base import Feature

from ..counter import CounterFormatter, build_counter_formatter, ValueWithSubNums
from .. import counter


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
            return {
                'value': None,
                'numprefix': None,
                'formatted_value': custom_label,
            }
        number, formatted_number = self.simple_counter.step_and_format_flm()
        return {
            'value': ValueWithSubNums(number),
            'numprefix': None,
            'formatted_value': formatted_number,
        }


def get_document_render_counter(
        render_context, counter_name, counter_formatter, alias_counter=None,
        always_number_within=None
):
    # if counter_name != counter_formatter.counter_formatter_id:
    #     raise ValueError(
    #         "get_document_render_counter(): Please use the "
    #         "same counter_name as the counter_formatter's internal "
    #         "counter_formatter_id.  We might run into weird bugs if you don't."
    #     )
    # ### headings/section: all section types use the same counter formatter
    # ### despite different "counters"...
    
    if not render_context.supports_feature('numbering'):

        logger.debug("Requesting counter ‘%s’, feature 'numbering' is not enabled. "
                     "alias_counter=%r, always_number_within=%r",
                     counter_name, alias_counter, always_number_within)

        if always_number_within is not None:
            logger.error(
                f"Requested {always_number_within=} but 'numbering' feature is not supported; "
                f"for {counter_name=}."
            )
            raise ValueError(
                f"Counter ‘{counter_name}’ cannot be numbered within "
                f"‘{always_number_within.get('reset_at', '???')}’ "
                f"without the 'numbering' feature enabled."
            )

        if alias_counter is not None:
            return _CounterIface(
                counter_name,
                simple_counter=CounterAlias(
                    counter_formatter,
                    alias_counter,
                )
            )
        return _CounterIface(
            counter_name,
            simple_counter=Counter(
                counter_formatter,
            )
        )

    logger.debug("Requesting counter ‘%s’, creating via feature 'numbering'. "
                 "alias_counter=%r, always_number_within=%r",
                 counter_name, alias_counter, always_number_within)

    numbering_mgr = render_context.feature_render_manager('numbering')

    return numbering_mgr.register_counter(
        counter_name, counter_formatter, alias_counter=alias_counter,
        always_number_within=always_number_within,
    )


# ----------------------------------------------------------


# This FLM feature's only job is to assign numbers to individual instances of
# equations, floats, section headings, etc.  Numbers are assigned at render
# time.



class _DocCounterState:
    def __init__(self, formatter, rdr_mgr, counter_name,
                 use_doc_state_keys,
                 numprefix_and_value_for_doc_state):

        self.formatter = formatter
        self.rdr_mgr = rdr_mgr
        self.counter_name = counter_name

        self.counter_by_filtered_doc_states = {}
        # values are { 'value': 0, 'numprefix': None|'Blah-' }

        self.cur_filtered_doc_state = None
        self.cur_counter_state = None
        # { 'value': 0, 'numprefix': None|'Blah-' }

        self.use_doc_state_keys = list(use_doc_state_keys)
        self.numprefix_and_value_for_doc_state = numprefix_and_value_for_doc_state

    def __repr__(self):
        return f"_DocCounterState<‘{self.counter_name}’ {repr(self.cur_counter_state)}>"

    def get_filtered_doc_state(self):
        r"""
        A unique string that changes each time the relevant part of the doc
        state changes.  When this string changes, we will reset the counter.
        """
        rs = self.rdr_mgr.render_doc_states
        return json.dumps([rs.get(k, None) for k in self.use_doc_state_keys])

    def register_item(self, **kwargs):
        return self.rdr_mgr.register_item(self.counter_name, **kwargs)

    def _impl_register_item(self):

        new_filtered_doc_state = self.get_filtered_doc_state()
        if new_filtered_doc_state == self.cur_filtered_doc_state:
            # no document state changes, can simply increase our current counter
            # state
            pass
        elif new_filtered_doc_state in self.counter_by_filtered_doc_states:
            # Document state has changed for the state keys that we're
            # monitoring, but we've seen this state before (e.g. moved out of a
            # subequations state).  Go back to the counter in that state.
            self.cur_filtered_doc_state = new_filtered_doc_state
            self.cur_counter_state = self.counter_by_filtered_doc_states[new_filtered_doc_state]
        else:
            # Document state has changed for the state keys that we're
            # monitoring (e.g. moved on to next section), and we haven't seen
            # this state yet.  Need to reset counter

            self.cur_filtered_doc_state = new_filtered_doc_state

            if self.numprefix_and_value_for_doc_state:
                cur_numprefix, cur_value = self.numprefix_and_value_for_doc_state(
                    self.rdr_mgr.render_doc_states
                )
                cur_value = ValueWithSubNums(cur_value) # ensure ValueWithSubNums instance
            else:
                cur_numprefix = None
                cur_value = ValueWithSubNums(0)

            new_counter_state = {
                'value': cur_value,
                'numprefix': cur_numprefix,
            }
            self.counter_by_filtered_doc_states[new_filtered_doc_state] = new_counter_state
            self.cur_counter_state = new_counter_state

        self.cur_counter_state['value'] = self.cur_counter_state['value'].incremented()

        cur_value = self.cur_counter_state['value']
        cur_numprefix = self.cur_counter_state['numprefix']
        formatted_value = self.formatter.format_flm(
            cur_value.get_num(),
            numprefix=cur_numprefix,
            subnums=cur_value.get_subnums(),
            with_prefix=False,
        )
        return {
            'value': cur_value,
            'number': cur_value.get_num(),
            'subnums': cur_value.get_subnums(),
            'numprefix': cur_numprefix,
            'formatted_value': formatted_value,
        }

    def get_formatted_counter_value(
            self,
            **kwargs
    ):
        return self.formatter.format_flm(
            value=self.cur_counter_state['value'].get_num(),
            subnums=self.cur_counter_state['value'].get_subnums(),
            numprefix=self.cur_counter_state['numprefix'],
            **kwargs
        )


class FeatureNumbering(Feature):

    feature_name = 'numbering'
    feature_title = 'Numbering for figures, sections, equations, theorems, and more'

    class RenderManager(Feature.RenderManager):

        def initialize(self, number_within=None):
            self.counters = {}
            
            self.render_doc_states = dict()

            # number_within = {
            #   'equation': {'reset_at': 'subsection', 'numprefix': '${subsection}.'}
            # }
            if number_within is not None:
                self.number_within = number_within
            else:
                self.number_within = self.feature.number_within

            self.number_within_dependants = {
                v['reset_at']: [
                    k2
                    for (k2,v2) in self.number_within.items()
                    if v2['reset_at'] == v['reset_at']
                ]
                for v in self.number_within.values()
            }

            logger.debug("FeatureNumbering.RenderManager, using number_within=%r",
                         self.number_within)

        def register_counter(
                self,
                counter_name,
                counter_formatter,
                *,
                alias_counter=None,
                always_number_within=None,
                use_doc_state_keys=None,
                numprefix_for_doc_state_fn=None,
                value_for_doc_state_fn=None,
        ):
            # use_subcounter_doc_states = None | { 'doc_state_keys': ..., 'sub_formatter':  }

            if counter_name in self.counters:
                raise ValueError(f"Counter ‘{counter_name}’ already registered!")

            if always_number_within is not None:
                if counter_name in self.number_within:
                    self.number_within[counter_name]['reset_at'] = \
                        always_number_within['reset_at']
                else:
                    self.number_within[counter_name] = {
                        'reset_at': always_number_within['reset_at'],
                        'numprefix': always_number_within['numprefix'],
                    }
                logger.debug(
                    "Counter ‘%s’ is registered to always be numbered within "
                    "another counter, config=%r",
                    counter_name, self.number_within[counter_name]
                )

            use_doc_state_keys = []
            if counter_name in self.number_within:
                parent_counter = self.number_within[counter_name]['reset_at']
                use_doc_state_keys.append( f'cnt-{parent_counter}' )

            def _numprefix_and_value_for_doc_state(rs):
                if numprefix_for_doc_state_fn is not None:
                    numprefix = numprefix_for_doc_state_fn(rs, self)
                elif counter_name in self.number_within:
                    numprefix_template = self.number_within[counter_name]['numprefix']
                    logger.debug(
                        f"_numprefix_and_value_for_doc_state: "
                        f"{counter_name=}, {rs=}, {self.number_within[counter_name]=}, "
                        f"{self.counters=}"
                    )
                    def _mkfmtfunc(dcstate):
                        return lambda dummyarg: (
                            dcstate.get_formatted_counter_value(with_prefix=False)
                        )
                    numprefix = counter._replace_dollar_template_delayed(
                        numprefix_template,
                        dict([
                            (dcname, _mkfmtfunc(dcstate))
                            for dcname, dcstate in self.counters.items()
                        ])
                    ) (None)
                else:
                    numprefix = None

                if value_for_doc_state_fn is not None:
                    value = value_for_doc_state_fn(rs, self)
                else:
                    value = 0

                return numprefix, value

            self.counters[counter_name] = _DocCounterState(
                formatter=counter_formatter,
                rdr_mgr=self,
                counter_name=counter_name,
                use_doc_state_keys=use_doc_state_keys,
                numprefix_and_value_for_doc_state=_numprefix_and_value_for_doc_state,
            )

            return self.counters[counter_name]

        def set_render_doc_state(self, state_type, state_value):
            self.render_doc_states[state_type] = state_value

        def clear_render_doc_state(self, state_type):
            del self.render_doc_states[state_type]

        def register_item(self, counter_name, custom_label=None):

            if custom_label is not None:
                item_info = {
                    'value': None,
                    'number': None,
                    'subnums': None,
                    'numprefix': None,
                    'formatted_value': custom_label,
                }
            else:
                item_info = self.counters[counter_name]._impl_register_item()
            
            # update the render doc state, in case we have dependants.
            self.set_render_doc_state(f'cnt-{counter_name}', item_info['formatted_value'])

            logger.debug("registered numbered ‘%s’ item -> %r", counter_name, item_info)
            logger.debug("render_doc_states is now %r", self.render_doc_states)

            return item_info

        def get_formatted_counter_value(
                self, counter_name,
                **kwargs
        ):
            return self.counters[counter_name].get_formatted_counter_value(**kwargs)



    def __init__(self, number_within=None):
        super().__init__()
        self.number_within = number_within or {}



FeatureClass = FeatureNumbering
