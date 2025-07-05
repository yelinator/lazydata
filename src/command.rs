use crate::layout::query_editor::Mode;
use tui_textarea::{CursorMove, Scrolling};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Command {
    Quit,
    ToggleFocus,
    ExecuteQuery,

    DataTablePreviousTab,
    DataTableNextTab,
    DataTableNextRow,
    DataTablePreviousRow,
    DataTableNextHistoryRow,
    DataTablePreviousHistoryRow,
    DataTableScrollRight,
    DataTableScrollLeft,
    DataTableNextColor,
    DataTablePreviousColor,
    DataTableNextPage,
    DataTablePreviousPage,
    DataTableJumpToFirstRow,
    DataTableJumpToLastRow,
    DataTableNextColumn,
    DataTablePreviousColumn,
    DataTableAdjustColumnWidthIncrease,
    DataTableAdjustColumnWidthDecrease,
    DataTableCopySelectedCell,
    DataTableCopySelectedRow,
    DataTableCopyQueryToEditor,
    DataTableRunSelectedHistoryQuery,
    DataTableSetTabIndex(usize),

    SidebarToggleSelected,
    SidebarKeyLeft,
    SidebarKeyRight,
    SidebarKeyDown,
    SidebarKeyUp,
    SidebarDeselect,
    SidebarSelectFirst,
    SidebarSelectLast,
    SidebarScrollDown(u16),
    SidebarScrollUp(u16),

    EditorInputChar(char),
    EditorInputBackspace,
    EditorInputDelete,
    EditorInputEnter,
    EditorMoveCursor(CursorMove),
    EditorDeleteLineByEnd,
    EditorCancelSelection,
    EditorPaste,
    EditorUndo,
    EditorRedo,
    EditorDeleteNextChar,
    EditorSetMode(Mode),
    EditorScrollRelative(i16, i16),
    EditorScroll(Scrolling),
    EditorStartSelection,
    EditorCopySelection,
    EditorCutSelection,
    EditorPerformPendingOperator,

    NoOp,
}
#[derive(Debug, Clone, Copy)]
pub enum CommandCategory {
    Global,
    Editor,
    DataTable,
    Sidebar,
}

impl std::fmt::Display for CommandCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CommandCategory::Global => write!(f, "Global"),
            CommandCategory::Editor => write!(f, "Editor"),
            CommandCategory::DataTable => write!(f, "DataTable"),
            CommandCategory::Sidebar => write!(f, "Sidebar"),
        }
    }
}

impl CommandCategory {
    #[allow(dead_code)]
    pub const fn help_command_categories() -> [Self; 4] {
        [Self::Global, Self::Editor, Self::DataTable, Self::Sidebar]
    }
}
