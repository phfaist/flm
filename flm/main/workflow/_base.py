import logging
logger = logging.getLogger(__name__)


from .._util import abbrev_value_str


class RenderWorkflow:
    r"""
    Abstract base class for render workflows.

    A workflow orchestrates the full rendering pipeline: it selects the
    fragment renderer, renders document fragments (with optional endnotes),
    and performs any post-processing (e.g. template wrapping, running
    external tools).

    Subclasses override static configuration hooks and the
    :meth:`postprocess_rendered_document` method to implement
    format-specific behavior.

    .. attribute:: binary_output

        If ``True``, the workflow produces binary (non-text) output.
        Defaults to ``False``.
    """

    binary_output = False

    @staticmethod
    def get_workflow_default_config(flm_run_info, config) -> dict:
        r"""
        Return workflow-specific default configuration entries that will be
        merged into the run configuration.

        :param flm_run_info: The run-info dictionary.
        :param config: The current configuration dictionary.
        :returns: A dictionary of default config values (empty by default).
        """
        return {}


    @staticmethod
    def get_fragment_renderer_name(outputformat, flm_run_info, run_config) -> str|None:
        r"""
        Optionally override which fragment renderer is used for this workflow.

        Return a renderer name string to force a specific renderer, or
        ``None`` to let the output format determine the renderer (the
        default).

        :param outputformat: The requested output format string.
        :param flm_run_info: The run-info dictionary.
        :param run_config: The current run configuration.
        :returns: A renderer name string, or ``None``.
        """
        return None

    @staticmethod
    def get_default_main_config(flm_run_info, run_config) -> None|dict:
        r"""Return workflow-specific overrides for the main FLM configuration,
        or ``None`` to use defaults."""
        return None


    @staticmethod
    def requires_temporary_directory_output(flm_run_info, run_config) -> bool:
        r"""Return ``True`` if this workflow needs a temporary output directory
        (e.g. for multi-file LaTeX builds).  Defaults to ``False``."""
        return False


    # ---


    TypeWorkflowConfigDict : type = dict


    def __init__(
        self,
        workflow_config,
        flm_run_info,
        fragment_renderer_information,
        fragment_renderer
    ):
        r"""
        :param workflow_config: A dictionary of workflow-specific settings.
            Each key is also set as an instance attribute.
        :param flm_run_info: The run-info dictionary containing
            ``'main_config'`` and other run metadata.
        :param fragment_renderer_information: A
            ``FragmentRendererInformation`` instance describing the selected
            renderer.
        :param fragment_renderer: The
            :py:class:`~flm.fragmentrenderer.FragmentRenderer` instance used
            to produce the output.
        """

        self.config = workflow_config
        self.flm_run_info = flm_run_info
        self.fragment_renderer_information = fragment_renderer_information
        self.fragment_renderer = fragment_renderer

        self.main_config = self.flm_run_info['main_config']

        for k, v in self.config.items():
            setattr(self, k, v)

        logger.debug("Initialized workflow ‘%s’ with config %s", self.__class__.__name__,
                     abbrev_value_str(workflow_config, maxstrlen=512))


    def render_document(self, document, content_parts_infos=None, **kwargs):
        r"""Render a document and post-process the result.

        Calls :py:meth:`render_document_fragments` to obtain the rendered
        content and render context, then passes the result through
        :py:meth:`postprocess_rendered_document`.

        :param document: The :py:class:`~flm.flmdocument.FLMDocument` to
            render.
        :param content_parts_infos: Optional content-parts metadata.
        :returns: The final post-processed output.
        """

        rendered_content, render_context = self.render_document_fragments(document)

        final_content = self.postprocess_rendered_document(
            rendered_content, document, render_context
        )

        return final_content


    def render_document_fragments(self, document):
        r"""Render the document fragments via
        :py:meth:`~flm.flmdocument.FLMDocument.render`.

        :param document: The :py:class:`~flm.flmdocument.FLMDocument`.
        :returns: A tuple ``(rendered_result, render_context)``.
        """

        # Render the main document
        rendered_result, render_context = document.render(self.fragment_renderer)

        return rendered_result, render_context


    def render_document_fragment_callback(
            self, fragment, render_context,
            content_parts_infos,
            **kwargs
    ):
        r"""Render callback suitable for passing to
        :py:meth:`~flm.flmenvironment.FLMEnvironment.make_document`.

        Renders the main *fragment*, any additional content parts, and
        (if enabled) endnotes.

        :param fragment: The main :py:class:`~flm.flmfragment.FLMFragment`.
        :param render_context: The active render context.
        :param content_parts_infos: Dict with optional ``'parts'`` key
            listing additional fragment parts to render.
        :returns: The rendered output string.
        """

        rendered_result = fragment.render(render_context)

        #environment = fragment.environment

        # Render content parts, if applicable
        doc_parts = content_parts_infos.get('parts', None)
        if not doc_parts: doc_parts = []
        for doc_part_info in doc_parts:

            fragment_part = doc_part_info['fragment']
            if fragment_part is None:
                continue

            rendered_result += fragment_part.render(render_context)


        # Render endnotes
        if ( getattr(self, 'render_endnotes', True)
             and render_context.supports_feature('endnotes') ):
            endnotes_mgr = render_context.feature_render_manager('endnotes')
            endnotes_result = endnotes_mgr.render_endnotes()
            rendered_result = render_context.fragment_renderer.render_join_blocks([
                rendered_result,
                endnotes_result,
            ], render_context)

        return rendered_result


    def postprocess_rendered_document(self, rendered_content, document, render_context):
        r"""Post-process the rendered document output.

        Subclasses override this to apply template wrapping, run external
        tools, or perform other transformations.  The default implementation
        returns *rendered_content* unchanged.

        :param rendered_content: The raw rendered output string.
        :param document: The :py:class:`~flm.flmdocument.FLMDocument`.
        :param render_context: The render context from the render pass.
        :returns: The final output (string or bytes if
            :py:attr:`binary_output` is ``True``).
        """
        return rendered_content


