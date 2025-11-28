use dispatch::Queue;

/// Shared UI style constants for consistent layout.
pub mod style {
    /// Fixed NSTableView row height.
    pub const ROW_HEIGHT: f64 = 64.0;
    /// Horizontal inset applied to row content within the table.
    pub const ROW_HORIZONTAL_INSET: f64 = 12.0;
    /// Padding above and below each row's card to create breathing room.
    pub const ROW_VERTICAL_PADDING: f64 = 6.0;
    /// Visual spacing between rows (equals the combined vertical padding).
    pub const ROW_STACK_SPACING: f64 = ROW_VERTICAL_PADDING * 2.0;
    /// Interior padding before the icon within the row card.
    pub const ROW_INTERNAL_PADDING: f64 = 14.0;
    /// Gap between the icon and the text column.
    pub const ROW_ICON_TEXT_PADDING: f64 = 12.0;
    /// Width reserved for the trailing type label.
    pub const ROW_TYPE_LABEL_WIDTH: f64 = 90.0;
    /// Trailing padding after the type label.
    pub const ROW_TRAILING_PADDING: f64 = 14.0;
    /// Default icon size inside a row card.
    pub const ROW_ICON_SIZE: f64 = 44.0;
    /// Row title text field height.
    pub const ROW_TITLE_HEIGHT: f64 = 22.0;
    /// Row subtitle text field height.
    pub const ROW_SUBTITLE_HEIGHT: f64 = 18.0;
    /// Spacing between title and subtitle text.
    pub const ROW_TEXT_SPACING: f64 = 4.0;
    /// Corner radius for row cards.
    pub const ROW_CORNER_RADIUS: f64 = 12.0;
    /// Border width for row cards.
    pub const ROW_BORDER_WIDTH: f64 = 1.0;
    /// Selection background opacity applied to the accent color.
    pub const ROW_SELECTION_BG_ALPHA: f64 = 0.22;
    /// Selection border opacity applied to the accent color.
    pub const ROW_SELECTION_BORDER_ALPHA: f64 = 0.4;
    /// Corner radius for the clipboard preview panel.
    pub const PREVIEW_CORNER_RADIUS: f64 = 18.0;
}

/// Run work on the main thread. AppKit calls must be dispatched here.
pub fn run_on_main<F>(task: F)
where
    F: FnOnce() + Send + 'static,
{
    Queue::main().exec_async(task);
}

/// Calculate the next table row index with wrap-around semantics.
pub fn wrapped_row(current_row: isize, num_rows: isize, down: bool) -> isize {
    if num_rows <= 0 {
        return -1;
    }

    if down {
        if current_row < 0 || current_row >= num_rows - 1 {
            0
        } else {
            current_row + 1
        }
    } else if current_row <= 0 {
        num_rows - 1
    } else {
        current_row - 1
    }
}

#[cfg(test)]
mod tests {
    use super::wrapped_row;

    #[test]
    fn wraps_downwards() {
        assert_eq!(wrapped_row(-1, 3, true), 0);
        assert_eq!(wrapped_row(0, 3, true), 1);
        assert_eq!(wrapped_row(2, 3, true), 0);
    }

    #[test]
    fn wraps_upwards() {
        assert_eq!(wrapped_row(0, 3, false), 2);
        assert_eq!(wrapped_row(1, 3, false), 0);
    }

    #[test]
    fn handles_empty() {
        assert_eq!(wrapped_row(0, 0, true), -1);
    }
}
