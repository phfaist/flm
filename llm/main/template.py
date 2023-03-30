
import string

import yaml
import json

from .configmerger import ConfigMerger
configmerger = ConfigMerger()

from .importclass import importclass


# def _flatten_dict(d, joiner='.'):
#     r = {}
#     _flatten_dict_impl(r, d, [], joiner)
#     return r

# def _flatten_dict_impl(r, d, prefix, joiner):
#     for k, v in d.items():
#         p = prefix + [str(k)]
#         if isinstance(v, dict):
#             _flatten_dict_impl(r, v, p, joiner)
#         else:
#             r[ joiner.join(p) ] = v


_emptydict = {}

class _ProxyDictVarConfig:
    def __init__(self, config, ifmarks):
        self.config = config
        self.ifmarks = ifmarks

    def __getitem__(self, key):
        
        if key.startswith('if:'):
            boolkey = key[3:]
            value = self.get_config_value(key)
            if value:
                return self.ifmarks['iftrue']
            else:
                return self.ifmarks['iffalse']
        elif key == 'else':
            return self.ifmarks['else']
        elif key == 'endif':
            return self.ifmarks['endif']
            
        return self.get_config_value(key)

    def get_config_value(key):
        parts = key.split('.')
        value = self.config
        for part in parts:
            value = value.get(part, _emptydict)
        if value is _emptydict:
            return None
        return value

class _StrTemplate(string.Template):
    braceidpattern = r'(?a:[_.:a-z0-9-]+)'


# h = hashlib.sha1(); h.update('if-then-else'.encode('ascii')); h.hexdigest()
_default_ifmarks = {
    'iftrue': '<IF:1:5c38e214ae2df4e928130170a825af0fb03f8d9d/>',
    'iffalse': '<IF:0:5c38e214ae2df4e928130170a825af0fb03f8d9d/>',
    'else': '<ELSE:5c38e214ae2df4e928130170a825af0fb03f8d9d/>',
    'endif': '<ENDIF:5c38e214ae2df4e928130170a825af0fb03f8d9d/>'
}


class SimpleStringTemplate:

    def __init__(self, template_info_file, *, template_content_extension='.html'):
        super().__init__()
        self.template_info_file = template_info_file # the .yaml file

        # the template content
        template_content_file = (
            re.sub(r'\.(ya?ml|json.*)$', '', self.template_info_file, flags=re.IGNORECASE)
            + template_content_extension
        )
        with open(template_content_file, encoding='utf-8') as f:
            self.template_content = f.read()

        self.ifmarks = dict(_default_ifmarks)

    def render_template(self, rendered_content, config, **kwargs):
        
        tpl = _StrTemplate(self.template_content)

        content = tpl.substitute(_ProxyDictVarConfig(config))
        
        content = replace_ifmarks(content, self.ifmarks)

        return content
        


def replace_ifmarks(content, ifmarks):

    # cheap if/else/endif mechanism

    rxp_iftrue = re.escape(self.ifmarks['iftrue'])
    rxp_iffalse = re.escape(self.ifmarks['iffalse'])
    rxp_else = re.escape(self.ifmarks['else'])
    rxp_endif = re.escape(self.ifmarks['endif'])

    rx_if = re.compile(
        r'(?P<iftrue>' + rxp_iftrue + r')|(?P<iffalse>' + rxp_iffalse + r')'
    )

    rx_ifelseendif = re.compile(
        r'(?P<iftrue>' + rxp_iftrue + r')|(?P<iffalse>' + rxp_iffalse + r')'
        + '(?P<block_if>.*?)'
        + r'((?P<else>' + rxp_else + r')(?P<block_else>.*))?'
        + r'(?P<endif>' + rxp_endif + r')'
    )

    # replace all if/else/endif, in reverse order starting from the last match,
    # to get nested conditions hopefully right.
    while True:
        matches = rx_if.findall(content)

        if not len(matches):
            # done.
            return content

        m_if = matches[-1]
        pos = m_if.start()
        m = rx_ifelseendif.match(content, pos)
        if m is None:
            raise ValueError(
                "Invalid if[/else]/endif construct near if “{content[pos:pos+64]}”"
            )
        if m.group('iftrue'): 
            content = content[:pos] + m.block_if + content[m.end():]
        if m.group('iffalse'): 
            content = content[:pos] + m.block_else + content[m.end():]



# ------------------------------------------------------------------------------



................


class DocumentTemplate:
    def __init__(self, template_name, template_config, llm_run_info):
        super().__init__()

        self.template_name = template_name
        self.template_config = template_config
        self.llm_run_info = llm_run_info

        self.template_info_file = \
            llm_run_info.get_template_name(self.template_name, self.llm_run_info)
        template_info = llm_run_info.load_file(self.template_info_file, 'template_info')

        self.template_info = template_info
        self.template_engine = template_info.get('template_engine',
                                                 'llm.main.template.SimpleStringTemplate')
        self.template_engine_config = template_info.get('template_engine_config', {})
        self.default_config = template_info.get('default_config', {})

        cls = importclass(self.template_engine, default_classname='TemplateEngineClass')

        self.template = cls(
            self.template_info_file,
            **self.template_engine_config
        )
        # default_config=, **self.engine_config)
        
    def render_template(self, document, local_config, **kwargs):

        logger.debug("rendering template ‘%s’, config is %r", self.template_name, config)

        metadata = self.document.metadata
        if metadata is None:
            metadata = {}
        else:
            metadata = {k: v for (k, v) in metadata.items() if k != "config"}


        merged_config = configmerger.recursive_assign_defaults(
            [
                local_config,
                self.template_config,
                {
                    'metadata': metadata,
                },
                self.default_config,
            ]
        )


        return self.template.render_template(rendered_content, **kwargs)
