FLM Environment: ``flm.flmenvironment``
=======================================

.. automodule:: flm.flmenvironment


Pylatexenc specializations
--------------------------

.. autoclass:: FLMParsingState
   :show-inheritance:
   :members:
   :undoc-members:

.. autoclass:: FLMParsingStateDeltaSetBlockLevel
   :show-inheritance:
   :members:
   :undoc-members:

.. autofunction:: FLMArgumentSpec

.. autoclass:: FLMLatexWalker
   :show-inheritance:
   :members:
   :undoc-members:


Finalizing nodes and handling text blocks
-----------------------------------------

A “nodes finalizer” is meant to process a compiled node or compiled node list to
add some relevant meta-information and normalize content appropriately.  In
particular, we auto-detect whether or not the node list is in block-level mode
or not (if not already specified) and we handle space characters (remove them
between block-level components and surrounding text; keep one space between
inline components).

.. autoclass:: NodesFinalizer
   :show-inheritance:
   :members:
   :undoc-members:



The main `FLMEnvironment` class
-------------------------------

.. autoclass:: FLMEnvironment
   :show-inheritance:
   :members:
   :undoc-members:



Constructing a “standard” environment
-------------------------------------

Convenience functions for constructing a standard environment with appropriate
definitions and handlers.


.. autofunction:: standard_parsing_state

.. autofunction:: make_standard_environment


.. autoclass:: FLMLatexWalkerMathContextParsingStateEventHandler
   :show-inheritance:
   :members:

.. autofunction:: standard_environment_get_located_error_message


Helpers to create macro/environment/specials nodes
--------------------------------------------------

It is sometimes useful, in a render function, to create new “virtual” nodes in
order to call fragment renderer routines.  For instance, an enumeration list
will likely create nodes for the enumerator tag with automatic numbering.

For this purpose, you should use the latex walker' methods
:py:meth:`pylatexenc.latexwalker.LatexWalker.make_node()` and
:py:meth:`pylatexenc.latexwalker.LatexWalker.make_nodelist()`.  (Recall that the
latex walker instance is accessible as `node.latex_walker`.)  As a convenience,
we provide the following methods to create nodes that represent an “invocable”
instance, e.g., a LaTeX macro possibly given with specific arguments.

.. autofunction:: make_invocable_arguments

.. autofunction:: make_invocable_node_instance
