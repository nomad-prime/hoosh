pub mod attachment;
pub mod paste_detector;
pub mod textarea;
pub mod wrapping;

pub use attachment::TextAttachment;
pub use paste_detector::{PasteClassification, PasteDetector};
pub use textarea::{TextArea, TextAreaState};
pub use wrapping::wrap_ranges;
