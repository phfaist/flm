#
# Support for \begin{cells} ... \end{cells} -- better than LaTeX' {tabular}.
#

import re

import logging
logger = logging.getLogger(__name__)

from pylatexenc import macrospec
from pylatexenc.latexnodes import (
    LatexArgumentSpec,
    LatexWalkerParseError,
    ParsedArgumentsInfo,
    ParsingStateDelta,
)
from pylatexenc.latexnodes import nodes as latexnodes_nodes
from pylatexenc.latexnodes import parsers as latexnodes_parsers

from ..flmenvironment import (
    FLMArgumentSpec,
)
from ..flmspecinfo import (
    FLMEnvironmentSpecBase,
    FLMMacroSpecError
)

from ._base import Feature




class _NotSpecified:
    pass

# ------------------


class LatexTabularRowSeparatorSpec(macrospec.MacroSpec):
    def __init__(self):
        super().__init__(macroname='\\', arguments_spec_list=[])

class LatexTabularColumnSeparatorSpec(macrospec.SpecialsSpec):
    def __init__(self):
        super().__init__(specials_chars='&', arguments_spec_list=[])


_macro_args = {
    'mergespec': LatexArgumentSpec(
        latexnodes_parsers.LatexCharsGroupParser(enable_groups=False),
        argname='mergespec'
    ),
    'styles': LatexArgumentSpec(
        latexnodes_parsers.LatexCharsGroupParser(
            delimiters=('<', '>'),
            enable_groups=False,
            optional=True,
        ),
        argname='styles'
    ),
    'styles_mapping': LatexArgumentSpec(
        latexnodes_parsers.LatexCharsGroupParser(
            delimiters=('<', '>'),
            enable_groups=False,
            optional=True,
        ),
        argname='styles_mapping'
    ),
    'cellcontents': FLMArgumentSpec(
        '{',
        argname='cellcontents',
        is_block_level=True,
    ),
}

class MergeMacroSpec(macrospec.MacroSpec):
    def __init__(self, macroname='merge',):
        super().__init__(macroname, arguments_spec_list=[
            _macro_args['mergespec'],
        ])

_macro_args['placement'] = LatexArgumentSpec(
    latexnodes_parsers.LatexDelimitedGroupParser(
        delimiters=('[',']',),
        optional=True,
    ),
    argname='placement',
    parsing_state_delta=macrospec.ParsingStateDeltaExtendLatexContextDb(
        extend_latex_context=dict(
            macros=[ MergeMacroSpec() ]
        )
    ),
)

_macro_args['placement_mapping'] = LatexArgumentSpec(
    latexnodes_parsers.LatexDelimitedGroupParser(
        delimiters=('[',']',),
        optional=True,
    ),
    argname='placement_mapping',
    parsing_state_delta=macrospec.ParsingStateDeltaExtendLatexContextDb(
        extend_latex_context=dict(
            macros=[ MergeMacroSpec() ]
        )
    ),
)

class CellMacro(FLMMacroSpecError):
    def __init__(self, macroname='cell',):
        super().__init__(macroname=macroname, arguments_spec_list=[
            _macro_args['styles'],
            _macro_args['placement'],
            _macro_args['cellcontents'],
        ])

_macro_args['celldata_contents'] = LatexArgumentSpec(
    '{',
    argname='celldata_contents',
    parsing_state_delta=macrospec.ParsingStateDeltaExtendLatexContextDb(
        extend_latex_context=dict(
            macros=[ CellMacro(), LatexTabularRowSeparatorSpec() ],
            specials=[ LatexTabularColumnSeparatorSpec() ],
        )
    ),
)

class CelldataMacroSpec(macrospec.MacroSpec):
    def __init__(self, macroname='celldata',):
        super().__init__(macroname, arguments_spec_list=[
            _macro_args['styles_mapping'],
            _macro_args['placement_mapping'],
            _macro_args['celldata_contents'],
        ])


# ------------------



class CellIndexRangeModel:
    def __init__(self, start, end):
        super().__init__()
        # !! in this internal representation, the start index starts at zero and
        # !! the end index points one past the end !!
        self.start = start
        self.end = end

    _fields = ('start', 'end',)

    def __repr__(self):
        if self.end == self.start + 1:
            return f'{self.start+1}'
        return f'[{self.start+1}-{self.end}]'


class CellPlacementModel:
    def __init__(self, row_range, col_range):
        super().__init__()
        self.row_range = row_range
        self.col_range = col_range

    _fields = ('row_range', 'col_range',)

    def __repr__(self):
        return f'{repr(self.row_range)},{repr(self.col_range)}'


class CellModel:
    def __init__(self, placement, styles, content_nodes):
        super().__init__()
        self.placement = placement
        self.styles = styles
        self.content_nodes = content_nodes

    _fields = ('placement', 'styles', 'content_nodes',)

    def __repr__(self):
        return (
            f"<Cell @{repr(self.placement)} <{' '.join(self.styles)}> "
            f"(‘{_splfysidews(self.content_nodes.latex_verbatim())}’)>"
        )

def _splfysidews(s):
    # simplify white space on the sides
    return re.sub(r'(^\s+|\s+$)', ' ', s)

# ------------------


class CellPlacementsMappingModel:
    def __init__(self, row_placements, col_placements):
        super().__init__()
        self.row_placements = row_placements
        self.col_placements = col_placements

    _fields = ('row_placements', 'col_placements',)

    def __repr__(self):
        return (
            f"{self.__class__.__name__}("
            + ''.join(f'\n    {k}={repr(getattr(self, k))}' for k in self._fields)
            + f"\n)"
        )

    def _get_index_range(self, placements, j, current):

        if len(placements) == 0:
            return CellIndexRangeModel(current, current+1)

        # don't include last placement range, we might need to deal with an open
        # range
        if j < (len(placements)-1):
            return placements[j]

        placement = placements[-1]
        if placement.end is None:
            idx = placement.start + (j - len(placements) + 1)
            return CellIndexRangeModel(idx, idx+1)
        return placement

    def get_row_index_range(self, row_j, current_row=None):
        return self._get_index_range(self.row_placements, row_j, current_row)

    def get_col_index_range(self, col_j, current_col=None):
        return self._get_index_range(self.col_placements, col_j, current_col)

    def start_row_col(self, current_row=None, current_col=None):
        return (self.get_row_index_range(0, current_row).start,
                self.get_col_index_range(0, current_col).start)

# ------------------



class CellsModel:
    def __init__(self, **kwargs):
        super().__init__()

        # !! internally, rows and columns start at zero (let's not kid ourselves) !!

        self.current_row = 0
        self.current_col = 0

        self.row_names = {}
        self.column_names = {}


        if kwargs:
            # e.g., reconstructing from loaded data
            self.cells_size = kwargs.pop('cells_size')
            self.cells_data = kwargs.pop('cells_data')
            self.finalize()
            return

        self.cells_size = [None, None] # rows, columns.  None = TBD
        self.cells_data = [] # list of cells instruction.

        self.grid_data = None


    _fields = ('cells_size', 'cells_data',)

    def __repr__(self):
        pp_data = ''.join(['\n        '+repr(d) for d in self.cells_data])
        return (
            f"{self.__class__.__name__}(\n    cells_size={repr(self.cells_size)},\n    "
            f"cells_data=[{pp_data}\n    ])"
        )

    # ------------------------------------------------------

    def add_cell_node(self, cell_node, default_placement=None, default_styles=None):
        # parse the cell_node's arguments
        cell_node_args = ParsedArgumentsInfo(node=cell_node).get_all_arguments_info(
            ('styles', 'placement', 'cellcontents',),
        )

        if default_styles is None:
            default_styles = []

        if cell_node_args['styles'].was_provided():
            styles = (
                cell_node_args['styles'].get_content_as_chars().split(' ')
                + default_styles
            )
        else:
            styles = default_styles

        if cell_node_args['placement'].was_provided():
            placement_spec = cell_node_args['placement'].get_content_nodelist()
        else:
            placement_spec = default_placement

        cell_contents = cell_node_args['cellcontents'].get_content_nodelist()

        return self.add_cell(placement_spec, styles, cell_contents)

    def add_cell(self, placement_spec, styles, content_nodes):

        placement = self.parse_placement_spec(placement_spec)

        cell = CellModel(
            placement=placement,
            styles=styles,
            content_nodes=content_nodes,
        )
        self.cells_data.append( cell )

        # grow cell model if necessary
        if self.cells_size[0] is None or cell.placement.row_range.end >= self.cells_size[0]:
            self.cells_size[0] = cell.placement.row_range.end
        if self.cells_size[1] is None or cell.placement.col_range.end >= self.cells_size[1]:
            self.cells_size[1] = cell.placement.col_range.end

        self.move_to_col( cell.placement.col_range.end )

        return cell

    def move_to_col(self, col):
        self.current_col = col

    def move_next_row(self):
        self.current_row += 1
        self.current_col = 0 # also reset column


    def finalize(self):
        # self.grid_data[row][col]
        self.grid_data = [
            [ None for _ in range(self.cells_size[1]) ]
            for _ in range(self.cells_size[0])
        ]
        for cell in self.cells_data:
            is_topleft = True
            for rowidx in range(cell.placement.row_range.start,
                                cell.placement.row_range.end):
                for colidx in range(cell.placement.col_range.start,
                                    cell.placement.col_range.end):
                    if self.grid_data[rowidx][colidx] is not None:
                        existing_cell = self.grid_data[rowidx][colidx]['cell']
                        raise ValueError(
                            f"‘{repr(cell)}’ overlaps with ‘{repr(existing_cell)}’"
                        )
                    # mark this grid location as being occupied by this cell
                    self.grid_data[rowidx][colidx] = {
                        'cell': cell,
                        'is_topleft': is_topleft
                    }
                    is_topleft = False
        # done.

    # ------------------------------------------------------

    def add_celldata_node(self, celldata_node):
        # parse the cell_node's arguments
        celldata_node_args = ParsedArgumentsInfo(node=celldata_node).get_all_arguments_info(
            ('styles_mapping', 'placement_mapping', 'celldata_contents',),
        )

        styles_mapping = [
            styles_spec.split()
            for styles_spec in
                celldata_node_args['styles_mapping'].get_content_as_chars().split(',')
        ]

        placement_mapping_spec = celldata_node_args['placement_mapping'].get_content_nodelist()

        celldata_contents = celldata_node_args['celldata_contents'].get_content_nodelist()

        # split celldata_contents into individual cell contents.

        data_rows = celldata_contents.split_at_node(
            lambda node: (
                node.isNodeType(latexnodes_nodes.LatexMacroNode)
                and node.macroname == '\\'
            )
        )

        def split_columns_predicate_fn(node):
            #logger.debug("Maybe split at node ? %r", node)
            if node.isNodeType(latexnodes_nodes.LatexSpecialsNode) \
               and node.specials_chars == '&':
                return True
            return False
        
        data_content_nodes = [
            data_row.split_at_node( split_columns_predicate_fn )
            for data_row in data_rows
        ]

        logger.debug('data_content_nodes = %r', data_content_nodes)

        self.add_celldata(placement_mapping_spec, styles_mapping, data_content_nodes)


    def add_celldata(self, placement_mapping_spec, styles_mapping, data_content_nodes):

        placement_mapping = self.parse_placement_mapping_spec(
            placement_mapping_spec,
        )

        logger.debug("add_celldata, placement_mapping = %r", placement_mapping)

        self.current_row, self.current_col = placement_mapping.start_row_col(
            current_row=self.current_row, current_col=self.current_col
        )

        data_row_j = 0
        for data_row_data in data_content_nodes:

            data_col_j = 0
            for cell_content in data_row_data:
                
                row_range = placement_mapping.get_row_index_range(
                    data_row_j, current_row=self.current_row
                )
                col_range = placement_mapping.get_col_index_range(
                    data_col_j, current_col=self.current_col
                )
                placement = CellPlacementModel(
                    row_range=row_range,
                    col_range=col_range
                )

                if data_col_j < len(styles_mapping):
                    styles = styles_mapping[data_col_j]
                else:
                    styles = styles_mapping[-1]

                cell_content_nl = \
                    cell_content.latex_walker.filter_whitespace_comments_nodes(
                        cell_content
                    )

                logger.debug(
                    f"placing cell ‘{_splfysidews(cell_content_nl.latex_verbatim())}’ at "
                    f"default placement {placement}; {data_row_j=}, {data_col_j=}"
                )

                if len(cell_content_nl) == 1 \
                   and cell_content_nl[0].isNodeType(latexnodes_nodes.LatexMacroNode) \
                   and cell_content_nl[0].macroname == 'cell':
                    # custom \cell call overriding style and/or placement
                    cell = self.add_cell_node(cell_content_nl[0],
                                              default_placement=placement,
                                              default_styles=styles)
                    data_col_j += (
                        cell.placement.col_range.end - cell.placement.col_range.start
                    )
                elif len(cell_content_nl) == 0:
                    # no contents for this cell -- skip this cell but still
                    # update the current col
                    self.current_col = col_range.end
                    data_col_j += 1
                else:
                    # the node list is simple cell content
                    self.add_cell( placement, styles, cell_content, )
                    data_col_j += 1
            
            self.move_next_row()
            data_row_j += 1
            data_col_j = 0


    # --- parsers for cell indices, ranges, etc. -----------

    _rx_int = re.compile(r'^\d+$')

    def parse_cell_index_spec(self, index_spec, is_row=False, is_col=False,
                              default=_NotSpecified):

        if isinstance(index_spec, str):
            index_spec_s = index_spec
        else:
            # node list to get as string
            index_spec_s = index_spec.get_content_as_chars()

        index_spec_s = index_spec_s.strip()

        # is it empty or '.', which mean the current row/col ?
        if not index_spec_s or index_spec_s == '.':
            if default is not _NotSpecified:
                return default
            if is_row:
                return self.current_row
            if is_col:
                return self.current_col
            raise RuntimeError("Internal error: Neither is_row nor is_col is set!")

        # is it a simple integer?
        if self._rx_int.match(index_spec_s):
            return int(index_spec_s) - 1 # internal indices start at zero.

        # is it a row name alias?
        if is_row and index_spec_s in self.row_names:
            return self.row_names[index_spec_s]
        if is_col and index_spec_s in self.col_names:
            return self.col_names[index_spec_s]
            
        raise ValueError(
            f"Invalid cell index: ‘{index_spec_s}’, expected number or valid alias name"
        )

    def parse_cell_index_range_spec(self, range_spec_s, is_row=False, is_col=False,
                                    default=_NotSpecified,
                                    default_start=None, default_end=None):

        if ',' in range_spec_s:
            # split into individual range specifications
            parts = range_spec_s.split(',')
            include_array = []
            overall_start = None
            overall_end = None
            for part in parts:
                start, end = self.parse_cell_index_range_spec(
                    part, is_row=is_row, is_col=is_col,
                    default=default
                )
                for idx in range(start, end):
                    if idx >= len(include_array):
                        include_array += [ False for _ in range(end-len(include_array)+1) ]
                    include_array[idx] = True
                if overall_start is None or overall_start > start:
                    overall_start = start
                if overall_end is None or overall_end < end:
                    overall_end = end
            # check that the given ranges' union is a contiguous range
            for idx in range(overall_start, overall_end):
                if not include_array[idx]:
                    raise ValueError(
                        f"Specified range ‘{range_spec_s}’ is not contiguous"
                    )
            return overall_start, overall_end

        if '-' in range_spec_s:
            # range specified as "idx1-idx2"
            start_spec, end_spec = range_spec_s.split('-', 1)
            start = self.parse_cell_index_spec(
                start_spec, is_row=is_row, is_col=is_col, default=default_start
            )
            end_incl = self.parse_cell_index_spec(
                end_spec, is_row=is_row, is_col=is_col, default=None
            )
            if end_incl is None:
                end = default_end
            else:
                end = end_incl + 1
            return start, end

        if '+' in range_spec_s:
            # range specified as "idx1+N"
            start_spec, len_spec = range_spec_s.split('+', 1)
            start = self.parse_cell_index_spec(
                start_spec, is_row=is_row, is_col=is_col, default=default,
            )
            if not self._rx_int.match(len_spec):
                raise ValueError(
                    f"Invalid number in ‘START+N’ cell index range specification: ‘{len_spec}’"
                )
            range_len = int(len_spec)
            # '-1' because 'end' includes final cell index and len_spec
            # counts the number of cells to include
            end = start + range_len
            return start, end

        idx = self.parse_cell_index_spec(
            range_spec_s, is_row=is_row, is_col=is_col, default=default
        )
        return idx, idx+1


    def parse_placement_index_spec(self, placement_index_spec, is_row=False, is_col=False,
                                   default=_NotSpecified,
                                   default_start=0, default_end=_NotSpecified):
        # a single column/row index or a \merge macro

        if len(placement_index_spec) == 0:
            if is_row:
                return CellIndexRangeModel(start=self.current_row, end=self.current_row+1)
            elif is_col:
                return CellIndexRangeModel(start=self.current_col, end=self.current_col+1)
            else:
                raise RuntimeError("Internal error, neither is_row nor is_col was set here.")

        nl = placement_index_spec.latex_walker.filter_whitespace_comments_nodes(
            placement_index_spec
        )

        if len(nl) != 1:
            if is_row:
                PLACEHOLDER = 'ROW'
            elif is_col:
                PLACEHOLDER = 'COL'
            else:
                PLACEHOLDER = None
            raise LatexWalkerParseError(
                f"Bad cell index or merge range specification, expected ‘{PLACEHOLDER}’ or "
                f"‘\\merge{'{'}{PLACEHOLDER}-RANGE{'}'}’, got {repr(placement_index_spec)} "
                f"({len(nl)} nodes)",
                pos=nl.pos
            )

        node = nl[0]

        if node.isNodeType(latexnodes_nodes.LatexMacroNode) \
           and node.macroname == 'merge':
            # it's a \merge macro -->

            merge_node_args = ParsedArgumentsInfo(node=node).get_all_arguments_info(
                ('mergespec',) ,
            )

            range_spec_s = merge_node_args['mergespec'].get_content_as_chars()

            default_end_computed = default_end
            if default_end_computed is _NotSpecified:
                if is_row:
                    default_end_computed = self.cells_size[0]
                if is_col:
                    default_end_computed = self.cells_size[1]

            start, end = self.parse_cell_index_range_spec(
                range_spec_s, is_row=is_row, is_col=is_col,
                default_start=default_start, default_end=default_end_computed,
            )

            return CellIndexRangeModel(start=start, end=end)

        # it has to be a simple cell index
        idx = self.parse_cell_index_spec(
            nl.get_content_as_chars(), is_row=is_row, is_col=is_col,
            default=default,
        )
        return CellIndexRangeModel(start=idx, end=idx+1)


    def parse_placement_spec(self, placement_spec):

        if isinstance(placement_spec, CellPlacementModel):
            return placement_spec

        row_spec_nl, col_spec_nl = [], []

        if placement_spec is not None:

            placement_spec_split = placement_spec.split_at_chars(';', keep_empty=True)

            if len(placement_spec_split) == 2:

                row_spec_nl, col_spec_nl = placement_spec_split

            elif len(placement_spec_split) == 1:

                (col_spec_nl,) = placement_spec_split

            elif len(placement_spec_split) == 0:

                # all ok, keep defaults
                pass

            else:

                raise LatexWalkerParseError(
                    f"Bad cell placement specification, expected ‘ROW;COL’ or "
                    f"‘COL’, got ‘{_splfysidews(placement_spec.latex_verbatim())}’",
                    pos=placement_spec.pos
                )

        row_range = self.parse_placement_index_spec(row_spec_nl, is_row=True)
        col_range = self.parse_placement_index_spec(col_spec_nl, is_col=True)

        return CellPlacementModel(row_range=row_range, col_range=col_range)


    # ---

    def parse_placement_mapping_index_spec(self, placement_mapping_index_spec,
                                           index_end, is_row=False, is_col=False):

        if len(placement_mapping_index_spec) == 0:
            # signify add/current row/col
            return []

        parts = placement_mapping_index_spec.split_at_chars(',', keep_empty=True)

        # keep track of "current row/col" in the specification mapping.
        current_idx = 0

        index_placements = []
        for placement_part_spec in parts:
            # placement_part_spec is a LatexNodeList which should contain only a
            # single node (index specification, range specification, or a single
            # \merge{...}  specification)
            
            nl = placement_part_spec.latex_walker.filter_whitespace_comments_nodes(
                placement_part_spec
            )

            if len(nl) != 1:
                if is_row:
                    PLACEHOLDER = 'ROW-RANGE'
                elif is_col:
                    PLACEHOLDER = 'COL-RANGE'
                else:
                    PLACEHOLDER = None
                raise LatexWalkerParseError(
                    f"Bad cell index or range or merge range specification, "
                    f"expected ‘{PLACEHOLDER}’ or "
                    f"‘\\merge{'{'}{PLACEHOLDER}-RANGE{'}'}’, got {repr(placement_index_spec)} "
                    f"({len(nl)} nodes)",
                    pos=nl.pos
                )

            node = nl[0]

            if node.isNodeType(latexnodes_nodes.LatexMacroNode) \
               and node.macroname == 'merge':
                # it's a \merge macro -->

                merge_node_args = ParsedArgumentsInfo(node=node).get_all_arguments_info(
                    ('mergespec',) ,
                )

                range_spec_s = merge_node_args['mergespec'].get_content_as_chars()

                start, end = self.parse_cell_index_range_spec(
                    range_spec_s, is_row=is_row, is_col=is_col,
                    default=current_idx,
                    default_start=current_idx, default_end=None
                )

                index_placements.append( CellIndexRangeModel(start=start, end=end) )
                current_idx = end
                continue

            # not a merge macro, parse range
            iter_start, iter_end = self.parse_cell_index_range_spec(
                nl.get_content_as_chars(), is_row=is_row, is_col=is_col,
                default=current_idx, default_start=current_idx, default_end=None
            )

            if iter_end is None:
                index_placements.append( CellIndexRangeModel(start=iter_start, end=None) )
                current_idx = None
            else:
                # simply append each item in the range to our placement mapping
                for j in range(iter_start, iter_end):
                    index_placements.append( CellIndexRangeModel(start=j, end=j+1) )
                    current_idx = j+1
            
            continue

        return index_placements

    def parse_placement_mapping_spec(self, placement_mapping_spec):
        
        placement_mapping_spec_split = \
            placement_mapping_spec.split_at_chars(';', keep_empty=True)

        if len(placement_mapping_spec_split) == 2:

            row_mapping_spec, col_mapping_spec = placement_mapping_spec_split

        elif len(placement_mapping_spec_split) == 1:

            row_mapping_spec = []
            (col_mapping_spec,) = placement_mapping_spec_split

        elif len(placement_mapping_spec_split) == 0:

            row_mapping_spec, col_mapping_spec = [], []

        else:

            raise LatexWalkerParseError(
                f"Expected ‘ROWS;COLS’ or ‘COLS’ or ‘’ for placement argument, "
                f"got ‘{_splfysidews(placement_mapping_spec.latex_verbatim())}’",
                pos=placement_mapping_spec.pos
            )

        row_placements = self.parse_placement_mapping_index_spec(
            row_mapping_spec, index_end=None, is_row=True
        )
        col_placements = self.parse_placement_mapping_index_spec(
            col_mapping_spec, index_end=None, is_col=True
        )

        return CellPlacementsMappingModel(
            row_placements=row_placements,
            col_placements=col_placements,
        )



# ------------------------------------------------------------------------------


class CellsEnvironment(FLMEnvironmentSpecBase):
    
    is_block_level = True

    allowed_in_standalone_mode = True
    
    body_contents_is_block_level = True


    def __init__(self, environmentname='cells'):
        super().__init__(
            environmentname=environmentname,
        )

    def make_body_parser(self, token, nodeargd, arg_parsing_state_delta):
        return macrospec.LatexEnvironmentBodyContentsParser(
            environmentname=token.arg,
            contents_parsing_state_delta=macrospec.ParsingStateDeltaExtendLatexContextDb(
                extend_latex_context=dict(
                    macros=[
                        CellMacro(),
                        CelldataMacroSpec(),
                        LatexTabularRowSeparatorSpec(),
                    ]
                )
            ),
            # not None; delta will be computed w.r.t. base parsing state, not
            # contents parsing state
            child_parsing_state_delta = ParsingStateDelta(),
        )


    def postprocess_parsed_node(self, node):

        # build the cells model here.

        cells_model = CellsModel()

        for n in node.nodelist:
            if n.isNodeType(latexnodes_nodes.LatexMacroNode):
                if n.macroname == 'cell':
                    cells_model.add_cell_node(n)
                    continue
                elif n.macroname == 'celldata':
                    cells_model.add_celldata_node(n)
                    continue
                elif n.macroname == '\\':
                    cells_model.move_next_row()
                    continue

            if n.isNodeType(latexnodes_nodes.LatexCommentNode):
                # okay, ignore comments
                continue

            if n.isNodeType(latexnodes_nodes.LatexCharsNode) \
               and len(n.chars.strip()) == 0:
                # okay, ignore pure whitespace
                continue

            raise LatexWalkerParseError(
                f"You cannot place ‘{_splfysidews(n.latex_verbatim())}’ here.  Expected: "
                f"\\cell, \\celldata, \\\\."
            )

        cells_model.finalize()

        node.flm_cells_model = cells_model


    def render(self, node, render_context):
        r"""
        Produce a final representation of the node, using the given
        `render_context`.
        """
        
        return render_context.fragment_renderer.render_cells(
            cells_model=node.flm_cells_model,
            render_context=render_context,
        )
    
    

# ------------------------------------------------------------------------------

class FeatureProvideCells(Feature):
    
    feature_name = 'cells'
    feature_title = 'Typesetting data tables'

    DocumentManager = None
    RenderManager = None

    def add_latex_context_definitions(self):
        return dict(
            environments=[
                CellsEnvironment(),
            ],
        )

    # ---

    def add_flm_doc_latex_context_definitions(self):
        r"""
        These definitions won't be used in the real world.  This method
        will only be queried by `flm.docgen` to generate comprehensive
        documentation that includes these commands.
        """
        return dict(
            macros=[
                CellMacro(),
                CelldataMacroSpec(),
                MergeMacroSpec(),
                LatexTabularRowSeparatorSpec(),
            ],
            specials=[ LatexTabularColumnSeparatorSpec() ]
        )


FeatureClass = FeatureProvideCells
