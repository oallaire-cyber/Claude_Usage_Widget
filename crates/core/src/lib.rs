//! Pure logic shared by `cuw-bridge` and the tray app. No I/O lives here: the clock (`now`) and file
//! contents are always passed in, so everything is unit-testable.

pub mod bridge_state;
pub mod merge;
pub mod notify;
pub mod pace;
pub mod sources;
pub mod statusline;
pub mod text;
pub mod time;
pub mod tray;
pub mod view;
pub mod window;
