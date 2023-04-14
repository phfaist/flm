import re
import string

import logging
logger = logging.getLogger(__name__)

import yaml
#import json

from .configmerger import ConfigMerger
configmerger = ConfigMerger()

from ._util import abbrev_value_str



_emptydict = {}

class _ProxyDictVarConfig:
    def __init__(self, config, ifmarks, none_as_empty_string=True):
        self.config = config
        self.ifmarks = ifmarks

        self.none_as_empty_string = none_as_empty_string

    def __getitem__(self, key):
        
        if key.startswith('if:'):
            boolkey = key[3:]
            value = self.get_config_value(boolkey)
            if value:
                return self.ifmarks['iftrue']
            else:
                return self.ifmarks['iffalse']
        elif key == 'else':
            return self.ifmarks['else']
        elif key == 'endif':
            return self.ifmarks['endif']
            
        return self.get_config_value(key)

    def get_config_value(self, key):
        parts = key.split('.')
        value = self.config
        for part in parts:
            value = value.get(part, _emptydict)
        if value is _emptydict:
            value = None
        if value is None and self.none_as_empty_string:
            return ''
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


class OnlyContentTemplate:

    def __init__(self, template_info_path, template_info_file, flm_run_info):
        super().__init__()
        self.template_info_path = template_info_path # base folder
        self.template_info_file = template_info_file # the .yaml file

    def render_template(self, config, **kwargs):
        return config['content']


class SimpleStringTemplate:

    def __init__(self, template_info_path, template_info_file, flm_run_info,
                 *, template_content_extension='.html', template_content_filename=None):
        super().__init__()
        self.template_info_path = template_info_path # base folder
        self.template_info_file = template_info_file # the .yaml file

        # the template content
        if template_content_filename is not None:
            template_content_file = template_content_filename
        else:
            template_content_file = (
                re.sub(r'\.(ya?ml|json.*)$', '', self.template_info_file, flags=re.IGNORECASE)
                + template_content_extension
            )

        self.template_content = flm_run_info['resource_accessor'].read_file(
            template_info_path, template_content_file, 'template_content'
        )

        self.ifmarks = dict(_default_ifmarks)

    def render_template(self, config, **kwargs):
        
        tpl = _StrTemplate(self.template_content)

        content = tpl.substitute(_ProxyDictVarConfig(config, self.ifmarks))
        
        content = replace_ifmarks(content, self.ifmarks)

        return content
        


def replace_ifmarks(content, ifmarks):

    # cheap if/else/endif mechanism

    rxp_iftrue = re.escape(ifmarks['iftrue'])
    rxp_iffalse = re.escape(ifmarks['iffalse'])
    rxp_else = re.escape(ifmarks['else'])
    rxp_endif = re.escape(ifmarks['endif'])

    rx_if = re.compile(
        r'((?P<iftrue>' + rxp_iftrue + r')|(?P<iffalse>' + rxp_iffalse + r'))',
        flags=re.DOTALL
    )

    rx_ifelseendif = re.compile(
        r'((?P<iftrue>' + rxp_iftrue + r')|(?P<iffalse>' + rxp_iffalse + r'))'
        + '(?P<block_if>.*?)'
        + r'((?P<else>' + rxp_else + r')(?P<block_else>.*?))?'
        + r'(?P<endif>' + rxp_endif + r')',
        flags=re.DOTALL
    )

    #logger.debug('rx pattern = ‘%s’', rx_ifelseendif.pattern)


    # replace all if/else/endif, in reverse order starting from the last match,
    # to get nested conditions hopefully right.
    while True:
        matches = list(rx_if.finditer(content))

        if not len(matches):
            # done.
            return content

        m_if = matches[-1]
        pos = m_if.start()
        m = rx_ifelseendif.match(content, pos)
        if m is None:
            raise ValueError(
                f"Invalid if[/else]/endif construct near if “{content[pos:pos+256]}”"
            )
        assert( m.start() == pos )
        
        #logger.debug("Substituting if/else/endif match -> %r", m.groupdict())

        block_if = m.group('block_if')
        if not block_if:
            block_if = ''

        block_else = m.group('block_else')
        if not block_else:
            block_else = ''

        pos_end = m.end()

        if m.group('iftrue'): 
            content = content[:pos] + block_if + content[pos_end:]
        elif m.group('iffalse'): 
            content = content[:pos] + block_else + content[pos_end:]



# ------------------------------------------------------------------------------




class DocumentTemplate:
    def __init__(self, template_name, template_prefix, template_config, flm_run_info):
        super().__init__()

        self.template_name = template_name
        self.template_prefix = template_prefix
        self.template_config = template_config
        self.flm_run_info = flm_run_info

        resource_accessor = self.flm_run_info['resource_accessor']

        self.template_info_path, self.template_info_file = \
            resource_accessor.get_template_info_file_name(
                self.template_prefix,
                self.template_name,
                self.flm_run_info
            )
        template_info_yaml = resource_accessor.read_file(
            self.template_info_path, self.template_info_file, 'template_info'
        )
        self.template_info = yaml.safe_load(template_info_yaml)

        self.template_engine = self.template_info.get('template_engine',
                                                 'flm.main.template.SimpleStringTemplate')
        self.template_engine_config = self.template_info.get('template_engine_config', {})
        self.default_config = self.template_info.get('default_config', {})

        _, cls = resource_accessor.import_class(
            self.template_engine,
            default_classnames=[ 'TemplateEngineClass' ],
        )

        self.template = cls(
            self.template_info_path,
            self.template_info_file,
            flm_run_info,
            **self.template_engine_config
        )
        # default_config=, **self.engine_config)
        
    def render_template(self, local_configs, **kwargs):

        merged_config = configmerger.recursive_assign_defaults(
            [
                *local_configs,
                self.template_config,
                self.default_config,
            ]
        )

        logger.debug("rendering template ‘%s’, config is %s",
                     self.template_name,
                     abbrev_value_str(merged_config))

        return self.template.render_template(merged_config, **kwargs)
