import importlib

from collections.abc import Mapping

from urllib.parse import urlparse
from urllib.request import urlopen

import yaml

import logging
logger = logging.getLogger(__name__)



# marker
class ListProperty:
    pass



# $defaults
class PresetDefaults:
    def __init__(self, defaults_additional_sources=None):
        if defaults_additional_sources is not None:
            self.defaults_additional_sources = list(defaults_additional_sources)
        else:
            self.defaults_additional_sources = []

    def process_list_item(self, configmerger,
                          presetarg, list_result, list_obj, j, list_obj_remaining,
                          property_path):
        logger.debug(f"defaults! {list_result=} {list_obj=} {j=} {list_obj_remaining=}")

        defaults_sources = list_obj_remaining + [
            defaults_source.fetch_defaults(property_path)
            for defaults_source in self.defaults_additional_sources
        ]

        defaults = configmerger.recursive_assign_defaults_list(
            defaults_sources,
            property_path
        )
        list_result.extend( defaults )


# $merge-config
class PresetMergeConfig:
    def process_list_item(self, configmerger,
                          presetarg, list_result, list_obj, j, list_obj_remaining,
                          property_path):
        logger.debug(f"merge-config, process_list_item, {list_result=}, {repr(list_obj)=} "
                     f"{j=} {repr(list_obj_remaining)=}")
        featurename = presetarg.get('name', None)
        if featurename is None:
            raise ValueError(
                "no name given, expected ‘$merge-config: {name: <name>, config: ...}’"
            )
        featurespecj0 = next(
            (j0 for j0 in range(len(list_result))
             if list_result[j0].get('name','') == featurename) ,
            None
        )
        if featurespecj0 is None:
            logger.error(f"$merge-config could not find item named ‘{featurename}’ in list %r",
                         list_result)
            raise ValueError(
                f"$merge-config -- could not find item named ‘{featurename}’"
            )

        newconfig = presetarg.get('config', {})

        list_result[featurespecj0]['config'] = \
            configmerger.recursive_assign_defaults_dict(
                [newconfig, list_result[featurespecj0]['config']],
                property_path
            )


class PresetRemoveItem:
    def process_list_item(self, configmerger,
                          presetarg, list_result, list_obj, j, list_obj_remaining,
                          property_path):
        featurename = presetarg
        if featurename is None:
            raise ValueError(
                "no name given, expected ‘$remove-item: <name>’"
            )
        featurespecj0 = next(
            (j0 for j0 in range(len(list_result))
             if list_result[j0].get('name','') == featurename) ,
            None
        )
        if featurespecj0 is None:
            logger.error(f"$remove-item could not find item named ‘{featurename}’ in list %r",
                         list_result)
            raise ValueError(
                f"$remove-item -- could not find item named ‘{featurename}’"
            )
        list_result[featurespecj0:featurespecj0+1] = [] # remove desired item


class PresetImport:
    def _fetch_import(self, remote):
        u = urlparse(remote)

        if not u.scheme or u.scheme == 'file':
            with open(u.path) as f:
                return yaml.safe_load(f)

        if u.scheme == 'pkg':
            modname, *modargs = u.path.split('/')
            mod = importlib.import_module(modname)
            if len(modargs) == 0:
                modargs = [ 'flm_default_import_config' ]
            try:
                obj = mod
                for part in modargs:
                    obj = getattr(obj, part)
                if callable(obj):
                    obj = obj()
                return obj
            except AttributeError:
                raise ValueError("Invalid preset $import target: ‘{}’".format(remote))

        with urlopen(remote) as response:
            # also works for JSON, since YAML is a superset of JSON
            return yaml.safe_load( response.read() )

    def process_property(self, configmerger, presetarg, result, obj, remaining_obj_list,
                         property_path):
        import_targets = presetarg
        if isinstance(import_targets, str):
            import_targets = [ import_targets ]
        for import_target in import_targets:
            target_data = self._fetch_import(import_target)
            result.update(configmerger.recursive_assign_defaults_dict(
                [ result, obj, target_data ] + remaining_obj_list,
                property_path
            ))
        logger.debug(f"processed property $import -> {result=} {obj=}")
        

    def process_list_item(self, configmerger,
                          presetarg, list_result, list_obj, j, list_obj_remaining,
                          property_path):
        import_targets = presetarg
        if isinstance(import_targets, str):
            import_targets = [ import_targets ]

        for import_target in import_targets:
            target_data = self._fetch_import(import_target)
            if not isinstance(target_data, list):
                target_data = [ target_data ]

            # call to recursive_assign_defaults_list() is important so we can
            # process $<preset>'s in target data
            new_items = configmerger.recursive_assign_defaults_list(
                [ target_data ],
                property_path
            )

            list_result.extend( new_items )

        logger.debug(f"processed list item $import -> {list_result=}")


def get_default_presets():
    return {
        '$defaults': PresetDefaults(),
        '$merge-config': PresetMergeConfig(),
        '$remove-item': PresetRemoveItem(),
        '$import': PresetImport(),
    }



def _get_preset_keyvals(d):
    if not isinstance(d, dict):
        return []
    return [(k,v) for (k,v) in d.items() if isinstance(k,str) and k.startswith('$')]


class ConfigMerger:
    def __init__(self, presets=None):
        if presets is not None:
            self.presets = dict(presets)
        else:
            self.presets = get_default_presets()

    def recursive_assign_defaults(self, obj_list):
        return self.recursive_assign_defaults_dict(obj_list, [])

    def recursive_assign_defaults_dict(self, obj_list, property_path):

        #logger.debug(f"recursive_assign_defaults_dict({obj_list=}, {property_path=})")

        if len(obj_list) == 0:
            return {}

        result = {}

        for j, obj in enumerate(obj_list):
            remaining_obj_list = obj_list[j+1:]

            if not isinstance(obj, Mapping):
                logger.warning("Incompatible config merge, ignoring value %r for ‘%s’",
                               obj, ".".join(property_path))
                continue

            # process any "meta"/preset keys
            for presetname, presetarg in _get_preset_keyvals(obj):
                del obj[presetname]
                self.presets[presetname].process_property(
                    self, presetarg, result, obj, remaining_obj_list,
                    property_path
                )

            for k in obj:

                if k in result:
                    # nothing to copy, value is already in result
                    continue

                if isinstance(obj[k], dict):
                    # recurse into sub-properties
                    # see if there are any presets to process

                    sub_result = self.recursive_assign_defaults_dict(
                        [obj[k]] + [
                            (o.get(k,{}) if isinstance(o,dict) else {})
                            for o in remaining_obj_list
                        ],
                        property_path + [k]
                    )

                    #logger.debug(f"Assigning default for property ‘{k}’ → {repr(sub_result)}")
                    result[k] = sub_result

                elif isinstance(obj[k], list):

                    list_result = self.recursive_assign_defaults_list(
                        [obj[k]] + [
                            (o.get(k,None) if isinstance(o,dict) else None)
                            for o in remaining_obj_list
                        ],
                        property_path + [k]
                    )

                    #logger.debug(f"Assigning default for property ‘{k}’ → {repr(list_result)}")
                    result[k] = list_result

                else:
                    # simply copy the scalar value.
                    result[k] = obj[k]

        return result


    def recursive_assign_defaults_list(self, obj_list, property_path):

        # ignore None's in argument list
        obj_list = [ o for o in obj_list if o is not None ]

        if len(obj_list) == 0:
            return []

        obj, *remaining_obj_list = obj_list

        list_result = []

        j = 0
        while j < len(obj):
            item = obj[j]

            if isinstance(item, dict):
                item_presets = _get_preset_keyvals(item)

                if len(item_presets) == 1:
                    presetname, presetarg = item_presets[0]
                    self.presets[presetname].process_list_item(
                        self, presetarg, list_result, obj, j, remaining_obj_list,
                        property_path + [ ListProperty ]
                    )
                    #logger.debug(f"process_list_item, new list is "
                    #             f"{list_result=} ({j=} {obj=})")
                    j += 1
                    continue

                elif len(item_presets) > 1:
                    raise ValueError(
                        "You cannot specify multiple $<preset> keys in config "
                        "list items"
                    )

            list_result.append( item )
            j += 1

        return list_result


