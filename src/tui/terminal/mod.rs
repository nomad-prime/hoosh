mod custom_terminal;
mod lifecycle;
pub mod lifecycle_fullview;
pub mod lifecycle_inline;

pub use custom_terminal::Terminal;
pub use lifecycle::HooshTerminal;
pub use lifecycle::init_terminal;
pub use lifecycle::resize_terminal;
pub use lifecycle::restore_terminal;
