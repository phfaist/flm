
from .configmerger import ConfigMerger

configmerger = ConfigMerger()


# ---



..........
llm_run_info = {
    'cwd': .....
    'template_path': ......
}



# ---


def load_features(features_config):

    features = []

    for featurename, featureconfig in features_config.items():

        if featureconfig is None or featureconfig is False:
            continue
        if featureconfig is True:
            featureconfig = {}
        
        FeatureClass = importclass(featurename, default_classname='FeatureClass')

        if hasattr(FeatureClass, 'default_config'):
            defaultconfig = dict(FeatureClass.feature_default_config)
            featureconfig = configmerger.recursive_assign_defaults(
                [featureconfig, defaultconfig]
            )

        features.append( FeatureClass(**featureconfig) )

    return features



def load_environment(environment_config):
    ...................................




def load_workflow(.........)



# everything goes through the config dictionary.

#
# config
# ======
#
# llm:
#   parsing:
#     enable_dollar_math: .... etc.
#   fragment:
#     is_block_level: .....
#     input_lineno_colno_offsets: .....
#   document_metadata:
#     ..... # HMMM NO NO NO NO...
#
#   workflow:
#     html:
#       ......
#     text:
#       ......
#


def run(config):
    
