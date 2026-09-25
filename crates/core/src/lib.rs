//! Pure logic shared by `cuw-bridge` and the tray app. No I/O lives here: the clock (`now`) and file
//! contents are always passed in, so everything is unit-testable.

pub mod bridge_state;
pub mod statusline;
pub mod time;
