use cocoa::base::{id, nil, YES};
use cocoa::foundation::NSString;
use dispatch::Queue;
use objc::{class, msg_send, sel, sel_impl};

/// Shared UI style constants for consistent layout.
pub mod style {
    /// Fixed NSTableView row height.
    pub const ROW_HEIGHT: f64 = 78.0;
    /// Horizontal inset applied to row content within the table.
    pub const ROW_HORIZONTAL_INSET: f64 = 18.0;
    /// Padding above and below each row's card to create breathing room.
    pub const ROW_VERTICAL_PADDING: f64 = 6.0;
    /// Visual spacing between rows (equals the combined vertical padding).
    pub const ROW_STACK_SPACING: f64 = ROW_VERTICAL_PADDING * 2.0;
    /// Interior padding before the icon within the row card.
    pub const ROW_INTERNAL_PADDING: f64 = 18.0;
    /// Gap between the icon and the text column.
    pub const ROW_ICON_TEXT_PADDING: f64 = 14.0;
    /// Width reserved for the trailing type label.
    pub const ROW_TYPE_LABEL_WIDTH: f64 = 96.0;
    /// Trailing padding after the type label.
    pub const ROW_TRAILING_PADDING: f64 = 16.0;
    /// Default icon size inside a row card.
    pub const ROW_ICON_SIZE: f64 = 44.0;
    /// Row title text field height.
    pub const ROW_TITLE_HEIGHT: f64 = 24.0;
    /// Row subtitle text field height.
    pub const ROW_SUBTITLE_HEIGHT: f64 = 18.0;
    /// Spacing between title and subtitle text.
    pub const ROW_TEXT_SPACING: f64 = 4.0;
    /// Top inset for content to align text/icon closer to the top of the row card.
    pub const ROW_CONTENT_TOP_INSET: f64 = 2.0;
    /// Corner radius for row cards.
    pub const ROW_CORNER_RADIUS: f64 = 14.0;
    /// Border width for row cards.
    pub const ROW_BORDER_WIDTH: f64 = 1.0;
    /// Selection background opacity applied to the accent color.
    pub const ROW_SELECTION_BG_ALPHA: f64 = 0.28;
    /// Selection border opacity applied to the accent color.
    pub const ROW_SELECTION_BORDER_ALPHA: f64 = 0.55;
    /// Corner radius for the clipboard preview panel.
    pub const PREVIEW_CORNER_RADIUS: f64 = 20.0;
    /// Horizontal inset for primary content (search bar, table, preview).
    pub const CONTENT_SIDE_INSET: f64 = 24.0;
    /// Extra margin inside the list container to avoid clipping shadows.
    pub const LIST_EXTRA_MARGIN: f64 = 12.0;
    /// Minimum width allotted to the list when both list and preview are visible.
    pub const LIST_MIN_WIDTH: f64 = 360.0;
    /// Minimum width allotted to the preview panel.
    pub const PREVIEW_MIN_WIDTH: f64 = 280.0;
    /// Ratio used to split available width between list and preview.
    pub const LIST_WIDTH_RATIO: f64 = 0.56;
    /// Gap between list and preview.
    pub const PREVIEW_GAP: f64 = 16.0;
    /// Extra gutter reserved inside the list for scrollbars and breathing room.
    pub const LIST_SCROLL_GUTTER: f64 = 32.0;
    /// Padding inside the clipboard preview card.
    pub const PREVIEW_CONTENT_INSET: f64 = 20.0;
    /// Height of the search bar container.
    pub const SEARCH_BAR_HEIGHT: f64 = 68.0;
    /// Top spacing between the window frame and the search bar.
    pub const SEARCH_BAR_TOP_MARGIN: f64 = 22.0;
    /// Gap between the search bar and the top of the results/preview split.
    pub const SEARCH_RESULTS_GAP: f64 = 14.0;
    /// Vertical offset between the top search bar and table start.
    pub const TABLE_TOP_OFFSET: f64 = SEARCH_BAR_HEIGHT + SEARCH_BAR_TOP_MARGIN;
    /// Vertical offset where the results/preview layout begins.
    pub const RESULTS_TOP_OFFSET: f64 = TABLE_TOP_OFFSET + SEARCH_RESULTS_GAP;
    /// Footer height beneath the table.
    pub const TABLE_FOOTER_HEIGHT: f64 = 30.0;
}

/// Run work on the main thread. AppKit calls must be dispatched here.
pub fn run_on_main<F>(task: F)
where
    F: FnOnce() + Send + 'static,
{
    Queue::main().exec_async(task);
}

/// Quick fade-in helper for lightweight transitions.
pub unsafe fn fade_in_view(view: id, duration: f64) {
    if view.is_null() {
        return;
    }
    let _: () = msg_send![view, setAlphaValue: 0.0f64];
    let _: () = msg_send![class!(NSAnimationContext), beginGrouping];
    let ctx: id = msg_send![class!(NSAnimationContext), currentContext];
    let _: () = msg_send![ctx, setDuration: duration];
    let animator: id = msg_send![view, animator];
    let _: () = msg_send![animator, setAlphaValue: 1.0f64];
    let _: () = msg_send![class!(NSAnimationContext), endGrouping];
}

#[allow(dead_code)]
/// Quick bounce scale for a lively snap. Uses Core Animation; no layout cost.
pub unsafe fn bounce_view(view: id, duration: f64) {
    if view.is_null() {
        return;
    }

    // Ensure backing layer exists
    let layer: id = msg_send![view, layer];
    let layer = if layer == nil {
        let _: () = msg_send![view, setWantsLayer: YES];
        msg_send![view, layer]
    } else {
        layer
    };
    if layer == nil {
        return;
    }

    // Reset to identity before animating
    let identity = CATransform3D::identity();
    let _: () = msg_send![layer, setTransform: identity];

    // Create scale animation
    let anim_key = nsstring("transform.scale");
    let animation: id = msg_send![class!(CABasicAnimation), animationWithKeyPath: anim_key];
    if animation == nil {
        return;
    }
    let from_val: id = msg_send![class!(NSNumber), numberWithDouble: 0.94f64];
    let to_val: id = msg_send![class!(NSNumber), numberWithDouble: 1.0f64];
    let _: () = msg_send![animation, setFromValue: from_val];
    let _: () = msg_send![animation, setToValue: to_val];
    let _: () = msg_send![animation, setDuration: duration];
    let timing: id =
        msg_send![class!(CAMediaTimingFunction), functionWithName: nsstring("easeOut")];
    let _: () = msg_send![animation, setTimingFunction: timing];
    let _: () = msg_send![layer, addAnimation: animation forKey: nsstring("bounce")];
}

/// Springier bounce for more visible movement without feeling heavy.
pub unsafe fn bounce_spring_view(view: id, from_scale: f64, to_scale: f64, duration_cap: f64) {
    if view.is_null() {
        return;
    }
    let layer: id = msg_send![view, layer];
    let layer = if layer == nil {
        let _: () = msg_send![view, setWantsLayer: YES];
        msg_send![view, layer]
    } else {
        layer
    };
    if layer == nil {
        return;
    }

    let identity = CATransform3D::identity();
    let _: () = msg_send![layer, setTransform: identity];

    let anim_key = nsstring("transform.scale");
    let animation: id = msg_send![class!(CASpringAnimation), animationWithKeyPath: anim_key];
    if animation == nil {
        return;
    }

    let _: () = msg_send![animation, setFromValue: nsnumber(from_scale)];
    let _: () = msg_send![animation, setToValue: nsnumber(to_scale)];
    let _: () = msg_send![animation, setDamping: 16.0f64];
    let _: () = msg_send![animation, setMass: 1.0f64];
    let _: () = msg_send![animation, setStiffness: 180.0f64];
    let _: () = msg_send![animation, setInitialVelocity: 10.0f64];

    let mut settling: f64 = msg_send![animation, settlingDuration];
    if settling.is_nan() || settling <= 0.0 {
        settling = 0.38;
    }
    let duration = settling.min(duration_cap);
    let _: () = msg_send![animation, setDuration: duration];

    let _: () = msg_send![layer, addAnimation: animation forKey: nsstring("spring-bounce")];
}

unsafe fn nsstring(s: &str) -> id {
    NSString::alloc(nil).init_str(s)
}

unsafe fn nsnumber(v: f64) -> id {
    msg_send![class!(NSNumber), numberWithDouble: v]
}

// CATransform3D from QuartzCore (redeclared to avoid extra deps)
#[repr(C)]
#[derive(Clone, Copy)]
struct CATransform3D {
    m11: f64,
    m12: f64,
    m13: f64,
    m14: f64,
    m21: f64,
    m22: f64,
    m23: f64,
    m24: f64,
    m31: f64,
    m32: f64,
    m33: f64,
    m34: f64,
    m41: f64,
    m42: f64,
    m43: f64,
    m44: f64,
}

impl CATransform3D {
    const fn identity() -> Self {
        Self {
            m11: 1.0,
            m12: 0.0,
            m13: 0.0,
            m14: 0.0,
            m21: 0.0,
            m22: 1.0,
            m23: 0.0,
            m24: 0.0,
            m31: 0.0,
            m32: 0.0,
            m33: 1.0,
            m34: 0.0,
            m41: 0.0,
            m42: 0.0,
            m43: 0.0,
            m44: 1.0,
        }
    }
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
