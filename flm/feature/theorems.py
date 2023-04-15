
from pylatexenc.latexnodes import ParsedArgumentsInfo, ParsingStateDelta
from pylatexenc.latexnodes import nodes as latexnodes_nodes
from pylatexenc import macrospec

from ..flmenvironment import FLMArgumentSpec, make_invocable_node_instance
from .. import flmspecinfo

from ..counter import Counter, CounterAlias, build_counter_formatter

from ._base import Feature
from . import headings




optional_text_arg = FLMArgumentSpec(
    parser='[',
    argname='thmtitle',
    flm_doc='An optional theorem environment title',
)



class TheoremEnvironment(flmspecinfo.FLMEnvironmentSpecBase):
    
    is_block_level = True

    body_contents_is_block_level = True

    def __init__(self, environmentname, theorem_spec, theorem_type_spec,
                 allowed_ref_label_prefixes):
        super().__init__(
            environmentname,
            arguments_spec_list=[
                optional_text_arg,
                flmspecinfo.label_arg
                ### TODO: Allow also \noproofref instruction
            ],
        )
        self.theorem_spec = theorem_spec
        self.theorem_type_spec = theorem_type_spec
        self.allowed_ref_label_prefixes = allowed_ref_label_prefixes

    def make_body_parser(self, token, nodeargd, arg_parsing_state_delta):
        return macrospec.LatexEnvironmentBodyContentsParser(
            environmentname=token.arg,
            contents_parsing_state_delta=ParsingStateDelta(
                set_attributes={
                    'is_block_level': True,
                }
            )
        )

    def postprocess_parsed_node(self, node):
        
        node_args = ParsedArgumentsInfo(node=node).get_all_arguments_info(
            ('thmtitle', 'label'),
        )

        thmtitle_nodelist = None
        if node_args['thmtitle'].was_provided():
            thmtitle_nodelist = node_args['thmtitle'].get_content_nodelist()
        
        relation_ref_target = None
        relation_ref_show_ref = False
        if self.theorem_type_spec['title_enable_relation_ref']:
            # parse the theorem title as [*label] or [**label] (as for phfthm.sty)
            if (thmtitle_nodelist is not None and len(thmtitle_nodelist) > 0):
                chnode = thmtitle_nodelist[0]
                if (chnode is not None
                    and chnode.isNodeType(latexnodes_nodes.LatexCharsNode)
                    and chnode.chars.startswith('*')):
                    if len(thmtitle_nodelist) != 1:
                        raise LatexWalkerLocatedError(
                            "When specifying a proof-ref target as optional argument "
                            "(‘[*thm:xyz]’), the entire argument must consist of "
                            "regular characters with no special meaning in FLM.  Got: "
                            f"‘{thmtitle_nodelist.latex_verbatim()}’",
                            pos=chnode.pos
                        )
                    if chnode.chars.startswith('**'):
                        relation_ref_target = chnode.chars[2:]
                        relation_ref_show_ref = False
                    elif chnode.chars.startswith('*'):
                        relation_ref_target = chnode.chars[1:]
                        relation_ref_show_ref = True

        if relation_ref_target is not None:
            if ':' in relation_ref_target:
                relation_ref_target = relation_ref_target.split(':', maxsplit=1)
            else:
                relation_ref_target = None, relation_ref_target

        node.flmarg_thmtitle = {
            'nodelist': thmtitle_nodelist,
            'has_relation_ref': True if relation_ref_target is not None else False,
            'relation_ref_target': relation_ref_target,
            'relation_ref_show_ref': relation_ref_show_ref,
        }

        node.flmarg_labels = flmspecinfo.helper_collect_labels(
            node_args['label'],
            self.allowed_ref_label_prefixes,
        )

        return node

    def render(self, node, render_context):

        fragment_renderer = render_context.fragment_renderer

        thms_mgr = render_context.feature_render_manager('theorems')
        refs_mgr = render_context.feature_render_manager('refs')
        
        # ---

        flmarg_thmtitle = node.flmarg_thmtitle
        flmarg_labels = node.flmarg_labels

        # ---

        if self.theorem_type_spec['numbered']:

            counter = thms_mgr.counters[self.environmentname]
            prefix_variant = 'capital'

            ref_instance = refs_mgr.register_reference_step_counter(
                node=node,
                counter=counter,
                target_href_fn=lambda n: f'#{self.environmentname}-{n}',
                counter_with_prefix=True,
                counter_prefix_variant=prefix_variant
            )
            
            counter_value = ref_instance.counter_value

            title_heading_formatted_flm = ref_instance.formatted_ref_flm_text

            target_id = f'{self.environmentname}-{counter_value}'

            title_heading_formatted_flm_frag = render_context.make_standalone_fragment(
                title_heading_formatted_flm,
                what='Theorem heading',
            )
            title_heading_formatted_flm_frag_nodes = title_heading_formatted_flm_frag.nodes

            for ref_type, ref_label in flmarg_labels:
                # register references in refs
                refs_mgr.register_reference(
                    ref_type, ref_label,
                    node=node,
                    formatted_ref_flm_text=title_heading_formatted_flm_frag,
                    target_href='#' + target_id,
                    counter_value=counter_value,
                    counter_formatter_id=counter.formatter.counter_formatter_id
                )

        else:

            theorem_name = self.theorem_spec['title']['capital']['singular']

            target_id = None
            # counter = None

            title_heading_formatted_flm_frag_nodes = node.latex_walker.make_nodelist(
                [
                    node.latex_walker.make_node(
                        latexnodes_nodes.LatexCharsNode,
                        chars=theorem_name,
                        pos=node.pos,
                        pos_end=node.pos,
                        parsing_state=node.parsing_state,
                    )
                ],
                parsing_state=node.parsing_state,
            )

            # We're not generating target_id's for these elements, so don't pin
            # down labels here...
            if len(flmarg_labels):
                raise LatexWalkerLocatedError(
                    r"You cannot use \label{} in unnumbered theorem environment ‘"
                    + self.environmentname + r"’",
                    pos=node.pos
                )

            # for ref_type, ref_label in flmarg_labels:
            #     # register references in refs
            #     refs_mgr.register_reference(
            #         ref_type, ref_label,
            #         node,
            #         theorem_name,
            #         None,
            #         counter_value=None,
            #     )            
        
        # ---

        # process whether we should display an optional title or not, and if so,
        # what to display exactly.

        thmtitle_nodelist = None
        if flmarg_thmtitle['has_relation_ref']:
            # build the thmtitle node list from the relation ref, if applicable.
            if flmarg_thmtitle['relation_ref_show_ref']:

                ref_type, ref_label = flmarg_thmtitle['relation_ref_target']

                # pick up the reference
                thm_relation_ref_instance = refs_mgr.get_ref(
                    ref_type, ref_label, node.latex_walker.resource_info
                )

                thm_relation_ref_flm_text = thm_relation_ref_instance.formatted_ref_flm_text
                thm_relation_ref_flm_frag = render_context.make_standalone_fragment(
                    thm_relation_ref_flm_text,
                    what='Thm relation ref'
                )
                thmtitle_nodelist = thm_relation_ref_flm_frag.nodes

        elif flmarg_thmtitle['nodelist'] is not None:

            thmtitle_nodelist = flmarg_thmtitle['nodelist']


        if thmtitle_nodelist is not None:

            heading_nodelist = node.latex_walker.make_nodelist(
                list(title_heading_formatted_flm_frag_nodes) +
                [
                    node.latex_walker.make_node(
                        latexnodes_nodes.LatexCharsNode,
                        chars=self.theorem_type_spec['heading_title_pre'],
                        pos=node.pos,
                        pos_end=node.pos,
                        parsing_state=node.parsing_state,
                    )
                ] + list(thmtitle_nodelist) + [
                    node.latex_walker.make_node(
                        latexnodes_nodes.LatexCharsNode,
                        chars=self.theorem_type_spec['heading_title_post'],
                        pos=node.pos,
                        pos_end=node.pos,
                        parsing_state=node.parsing_state,
                    )
                ],
                parsing_state=node.parsing_state,
                pos=node.pos
            )

        else:

            heading_nodelist = title_heading_formatted_flm_frag_nodes


        heading_node = make_invocable_node_instance(
            latexnodes_nodes.LatexMacroNode,
            flm_spec=headings.HeadingMacro(
                macroname=None,
                heading_level=self.theorem_type_spec['theorem_heading_level'],
                inline_heading=True
            ),
            args={
                'text': heading_nodelist,
            },
            latex_walker=node.latex_walker,
            parsing_state=node.parsing_state,
        )

        heading_node.flm_heading_target_id = target_id

        # rendered_heading = fragment_renderer.render_heading(
        #     heading_nodelist,
        #     render_context,
        #     heading_level=
        #     inline_heading=True,
        #     target_id=target_id,
        # )

        # rendered_parts = [
        #     rendered_heading
        # ]

        # ---

        final_content_node = node.latex_walker.make_node(
            latexnodes_nodes.LatexCharsNode,
            chars=self.theorem_type_spec['body_final_content'],
            pos=node.pos,
            pos_end=node.pos,
            parsing_state=node.parsing_state,
        )
        final_content_node.flm_strip_preceding_whitespace = True


        if self.theorem_type_spec['body_final_content']:
            body_nodelist = (
                [
                    heading_node
                ]
                + list(node.nodelist)
                + [
                    final_content_node
                ]
            )
        else:
            body_nodelist = (
                [
                    heading_node
                ]
                + list(node.nodelist)
            )

        body_nodelist = node.latex_walker.make_nodelist(
            body_nodelist,
            parsing_state=node.parsing_state
        )

        rendered_contents = fragment_renderer.render_nodelist(
            body_nodelist,
            render_context,
        )

        return fragment_renderer.render_semantic_block(
            rendered_contents, #fragment_renderer.render_join(rendered_parts, render_context),
            self.theorem_spec['theorem_type'],
            render_context,
            annotations=[ self.environmentname ],
        )




_default_theorem_environments_simpleset = {
    'theoremlike': {
        'theorem': {
            'title': {
                # 'lowercase': {
                #     'singular': 'theorem',
                #     'plural': 'theorems',
                # },
                # 'capital': {
                #     'singular': 'Theorem',
                #     'plural': 'Theorems',
                # },
                    'lowercase': 'theorem', # rest is guessed automatically 
                'abbreviated': {
                    'singular': 'Thm.',
                    'plural': 'Thms.',
                },
            },
        },
        'proposition': {
            'title': {
                'lowercase': 'proposition',
                'abbreviated': {
                    'singular': 'Prop.',
                    'plural': 'Props.',
                },
            },
        },
        'lemma': {
            'title': {
                'lowercase': 'lemma',
                'abbreviated': {
                    'singular': 'Lem.',
                    'plural': 'Lems.',
                },
            },
        },
        'corollary': {
            'title': {
                'lowercase': 'corollary',
                'abbreviated': {
                    'singular': 'Cor.',
                    'plural': 'Cors.',
                },
            },
        },
    },
    'definitionlike': {
        'definition': {
            'title': {
                'lowercase': 'definition',
                'abbreviated': {
                    'singular': 'Def.',
                    'plural': 'Defs.',
                },
            },
        },
    },
    'prooflike': {
        'proof': {
            'title': {
                'lowercase': 'proof',
                'abbreviated': {
                    'singular': 'Proof',
                    'plural': 'Proofs',
                },
            },
        },
    },
}


_default_theorem_environments_defaultset = {
    'theoremlike': dict(_default_theorem_environments_simpleset['theoremlike'], **{
        'conjecture': {
            'title': {
                'lowercase': 'conjecture',
                'abbreviated': {
                    'singular': 'Conj.',
                    'plural': 'Conjs.',
                },
            },
        },
    }),
    'definitionlike': dict(_default_theorem_environments_simpleset['definitionlike'], **{
        'remark': {
            'title': {
                'lowercase': 'remark',
                'abbreviated': {
                    'singular': 'Rem.',
                    'plural': 'Rems.',
                },
            },
        },
    }),
    'prooflike': dict(_default_theorem_environments_simpleset['prooflike']),
}

_default_theorem_environments_richset = {
    'theoremlike': dict(_default_theorem_environments_defaultset['theoremlike']),
    'definitionlike': dict(_default_theorem_environments_defaultset['definitionlike'], **{
        'idea': {
            'title': {
                'lowercase': 'idea',
                'abbreviated': {
                    'singular': 'Idea',
                    'plural': 'Ideas',
                },
            },
        },
        'question': {
            'title': {
                'lowercase': 'question',
                'abbreviated': {
                    'singular': 'Qtn.',
                    'plural': 'Qtns.',
                },
            },
        },
        'claim': {
            'title': {
                'lowercase': 'claim',
                'abbreviated': {
                    'singular': 'Clm.',
                    'plural': 'Clms.',
                },
            },
        },
        'observation': {
            'title': {
                'lowercase': 'observation',
                'abbreviated': {
                    'singular': 'Obs.',
                    'plural': 'Obs.',
                },
            },
        },
        'problem': {
            'title': {
                'lowercase': 'problem',
                'abbreviated': {
                    'singular': 'Prob.',
                    'plural': 'Probs.',
                },
            },
        },
    }),
    'prooflike': dict(_default_theorem_environments_defaultset['prooflike']),
}


default_theorem_environments = {
    'simpleset': _default_theorem_environments_simpleset,
    'defaultset': _default_theorem_environments_defaultset,
    'richset': _default_theorem_environments_richset,
}

default_theorem_theorem_types = {
    'theoremlike': {
        'numbered': True,
        'shared_numbering': True, # Thm. 1, Cor. 2, Rem. 3, False -> each its own counter
        # -- spec for counter-formatter if shared_numbering is False, defaults
        # -- are taken from the shared_counter_formatter spec
        'counter_formatter': None,
        # 'body_text_formats': ['textit'], # cf. FragmentRenderer.render_text_format()
        'theorem_heading_level': 'theorem', # special "heading level" for rendering theorem headings
        'heading_title_pre': ' (',
        'heading_title_post': ')',
        'title_enable_relation_ref': False, # for "Proof of..."
        'body_final_content': '',
    },
    'definitionlike': {
        'numbered': True,
        'shared_numbering': True,
        'counter_formatter': None,
        # 'body_text_formats': [],
        'theorem_heading_level': 'theorem',
        'heading_title_pre': ' (',
        'heading_title_post': ')',
        'title_enable_relation_ref': False, # for "Proof of..."
    },
    'prooflike': {
        'numbered': False,
        # 'body_text_formats': [],
        'title_enable_relation_ref': True, # for "Proof of..."
        'body_final_content': ' □',
    }
}

default_thm_shared_counter_formatter_spec = {
    'format_num': 'arabic',
    'delimiters': ('',''),
    'join_spec': 'default',
    # no prefixes, each theorem type will be added as a prefix variant
}

default_allowed_ref_label_prefixes = [
    'thm', 'prop', 'cor', 'lem',
    'rem', 'def', 'dfn',
    'x', 'topic'
]


class FeatureTheorems(Feature):

    feature_name = 'theorems'

    feature_dependencies = [ 'refs' ]

    feature_default_config = {
        'environments': default_theorem_environments['defaultset'],
        'theorem_types': default_theorem_theorem_types,
        'shared_counter_formatter': 'arabic',
        'allowed_ref_label_prefixes': default_allowed_ref_label_prefixes,
    }


    class RenderManager(Feature.RenderManager):

        def initialize(self):
            self.shared_counter = Counter(self.feature.shared_counter_formatter)

            refs_mgr = self.render_context.feature_render_manager('refs')

            self.counters = {}
            for env_name, counter_formatter in self.feature.thm_counter_formatters.items():
                thm_spec = self.feature.environments[env_name]
                thm_type_spec = self.feature.theorem_types[ thm_spec['theorem_type'] ]
                if thm_type_spec['shared_numbering']:
                    self.counters[env_name] = CounterAlias(
                        counter_formatter=counter_formatter,
                        alias_counter=self.shared_counter
                    )
                else:
                    self.counters[env_name] = Counter(
                        counter_formatter=counter_formatter,
                    )

                # register the counter_formatter in our refs manager
                refs_mgr.register_counter_formatter(counter_formatter=counter_formatter)


    # ---

    def __init__(self,
                 environments=None,
                 theorem_types=None,
                 shared_counter_formatter=None,
                 allowed_ref_label_prefixes=None):
        super().__init__()
        if environments is None:
            environments = default_theorem_environments['defaultset']
        if isinstance(environments, str):
            environments = default_theorem_environments[environments]

        if theorem_types is None:
            theorem_types = default_theorem_theorem_types

        # set up theorem type specs

        self.theorem_types = {
            thm_type: self._standardize_type_spec(thm_type, thm_type_spec)
            for thm_type, thm_type_spec in dict(theorem_types).items()
            if thm_type_spec is not None
        }

        # set up thm_spec objects

        self.environments = {}
        for thm_type, env_list in environments.items():
            for env_name, thm_spec in env_list.items():
                if 'env_name' in self.environments:
                    raise ValueError(
                        f"Duplicate definition of theorem environment ‘{env_name}’"
                    )
                self.environments[env_name] = \
                    self._standardize_thm_spec(thm_type, env_name, thm_spec)
                
        # set up counter formatters

        self.shared_counter_formatter = build_counter_formatter(
            shared_counter_formatter,
            default_thm_shared_counter_formatter_spec,
            counter_formatter_id='_theorems_shared',
        )

        use_default_counter_formatter_spec = self.shared_counter_formatter.asdict()

        self.thm_counter_formatters = {}
        for env_name, thm_spec in self.environments.items():
            thm_type_spec = self.theorem_types[thm_spec['theorem_type']]
            if not thm_type_spec['numbered']:
                # no numbering
                continue

            counter_formatter_spec = thm_type_spec.get('counter_formatter', None)
            if counter_formatter_spec is None:
                counter_formatter_spec = {}
            elif isinstance(counter_formatter_spec, str):
                counter_formatter_spec = {'format_num': counter_formatter_spec}
            else:
                counter_formatter_spec = dict(counter_formatter_spec)

            # customize the prefix_display of this counter formatter to match
            # the theorem name in all its glorious variants (lowercase, capital,
            # etc.)
            counter_formatter_spec['prefix_display'] = \
                self._make_counter_formatter_prefix_for_thm(
                    env_name,
                    thm_spec,
                )
            self.thm_counter_formatters[env_name] = build_counter_formatter(
                counter_formatter_spec,
                use_default_counter_formatter_spec,
                counter_formatter_id=env_name,
            )

        self.allowed_ref_label_prefixes = list(
            allowed_ref_label_prefixes
            if allowed_ref_label_prefixes is not None
            else default_allowed_ref_label_prefixes
        )


    def _standardize_type_spec(self, thm_type, thm_type_spec):
        # use just SOME default so that all keys are populated with some values
        spec = dict(default_theorem_theorem_types['theoremlike'])
        spec.update(thm_type_spec)
        return spec

    def _standardize_thm_spec(self, thm_type, env_name, thm_spec):
        if thm_spec is True:
            thm_spec = env_name
        if isinstance(thm_spec, str):
            thm_spec = {'title': thm_spec}
        else:
            thm_spec = dict(thm_spec)

        thm_spec['theorem_type'] = thm_type

        # standardize the title/prefix
        new_title_spec = {}
        title = thm_spec.get('title', None)
        if title is None:
            title = env_name
        if isinstance(title, str):
            title = { 'lowercase': {'singular': title,
                                    'plural': title + 's'} }

        if 'lowercase' in title:
            lowercase = title['lowercase']
            if isinstance(lowercase, str):
                new_title_spec['lowercase'] = {
                    'singular': lowercase,
                    'plural': lowercase + 's',
                }
            else:
                new_title_spec['lowercase'] = lowercase
        else:
            # ??? fallback ???
            new_title_spec['lowercase'] = { 'singular': '??', 'plural': '???' }

        if 'capital' in title:
            capital = title['capital']
            if isinstance(capital, str):
                new_title_spec['capital'] = {
                    'singular': capital,
                    'plural': capital + 's',
                }
            else:
                new_title_spec['capital'] = capital
        else:
            # simple fallback based on lowercase
            new_title_spec['capital'] = {
                'singular': new_title_spec['lowercase']['singular'].capitalize(),
                'plural': new_title_spec['lowercase']['plural'].capitalize(),
            }

        if 'abbreviated' in title:
            abbreviated = title['abbreviated']
            if isinstance(abbreviated, str):
                new_title_spec['abbreviated'] = {
                    'singular': abbreviated,
                    'plural': abbreviated + 's',
                }
            else:
                new_title_spec['abbreviated'] = abbreviated
        else:
            # simple fallback based on lowercase
            new_title_spec['abbreviated'] = {
                'singular': new_title_spec['capital']['singular'][:3] + '.',
                'plural': new_title_spec['capital']['plural'][:3].rstrip('s') + 's.',
            }

        thm_spec['title'] = new_title_spec

        return thm_spec


    def _make_counter_formatter_prefix_for_thm(self, env_name, thm_spec):
        prefix = {}

        def _add_space_values(x):
            return { k: v + '~' for (k,v) in x.items() }

        prefix['lowercase'] = _add_space_values(thm_spec['title']['lowercase'])
        prefix['capital'] = _add_space_values(thm_spec['title']['capital'])
        prefix['abbreviated'] = _add_space_values(thm_spec['title']['abbreviated'])

        prefix.update(prefix['capital'])

        return prefix


    def add_latex_context_definitions(self):

        environment_specs = []

        for env_name, thm_spec in self.environments.items():
            environment_specs.append( TheoremEnvironment(
                environmentname=env_name,
                theorem_spec=thm_spec,
                theorem_type_spec=self.theorem_types[thm_spec['theorem_type']],
                allowed_ref_label_prefixes=self.allowed_ref_label_prefixes,
            ) )

        return {
            'environments': environment_specs,
        }


# ------------------------------------------------

FeatureClass = FeatureTheorems
