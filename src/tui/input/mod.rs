pub mod attachment;
pub mod paste_detector;
pub mod wrapping;

pub use attachment::TextAttachment;
pub use paste_detector::{PasteClassification, PasteDetector};
pub use wrapping::{WrappedLine, WrappingCalculator};
