import logging
logger = logging.getLogger(__name__)

import importlib


def import_class(fullname, *, default_classnames=None, default_prefix=None):

    try_modname_classname_list = []

    if '.' not in fullname:
        if default_classnames is None or len(default_classnames) == 0:
            raise ValueError(f"Missing class name: ‘{fullname}’")

        if default_prefix is not None:
            prefixedfullname = f"{default_prefix}.{fullname}"
            for default_classname in default_classnames:
                try_modname_classname_list.append(
                    (prefixedfullname, default_classname)
                )

        for default_classname in default_classnames:
            try_modname_classname_list.append(
                (fullname, default_classname)
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
            logger.debug(f"Found ‘{classname}’ in module ‘{modname}’")
            return mod, classobj

        except AttributeError as e:
            logger.debug(f"No class ‘{classname}’ in module ‘{modname}’")
            continue
        # ### This doesn't work as intended because it also catches
        # ### ModuleNotFoundError's raised by a module that is found, but in
        # ### which there is an invalid import statement.  Beurk!
        # except ModuleNotFoundError as e: ...
        except ImportError as e:
            if e.name == modname and isinstance(e, ModuleNotFoundError):
                logger.debug(f"Could not find module ‘{modname}’: {str(e)}")
                continue
            else:
                logger.warning(f"Failed to import module ‘{modname}’: {str(e)}", exc_info=True)
                continue

    raise ValueError(f"Failed to locate import ‘{fullname}’")


