use dispatch::Queue;

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
