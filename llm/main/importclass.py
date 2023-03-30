import logging
logger = logging.getLogger(__name__)

import importlib


def import_class(fullname, *, default_classnames=None, default_prefix=None):

    try_modname_classname_list = []

    if '.' not in fullname:
        if default_classnames is None or len(default_classnames) == 0:
            raise ValueError(f"Missing class name: ‘{fullname}’")
        usefullname = fullname
        if default_prefix is not None:
            usefullname = f"{default_prefix}.{fullname}"
        for default_classname in default_classnames:
            try_modname_classname_list.append(
                (usefullname, default_classname)
            )
    else:
        modname, classname = fullname.rsplit('.', maxsplit=1)
        if default_classnames is not None:
            for default_classname in default_classnames:
                try_modname_classname_list.append( (fullname, default_classname) )
        try_modname_classname_list.append( (modname, classname) )

    for modname, classname in try_modname_classname_list:
        try:
            mod = importlib.import_module(modname)
            classobj = getattr(mod, classname)
            return mod, classobj
        except AttributeError as e:
            logger.debug(f"No class ‘{classname}’ in module ‘{modname}’")
            continue
        except ModuleNotFoundError as e:
            logger.debug(f"Could not find module ‘{modname}’: {str(e)}")
            continue
        except ImportError as e:
            logger.warning(f"Failed to import module ‘{modname}’: {str(e)}", exc_info=True)
            continue

    raise ValueError(f"Failed to locate import ‘{fullname}’")

