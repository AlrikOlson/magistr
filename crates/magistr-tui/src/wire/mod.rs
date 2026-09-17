//! Byte-stream plumbing shared by every parser that reads the terminal:
//! the escape-sequence walker the graphics probe and the input decoder
//! both step through.

pub mod sequence;

pub use sequence::next_sequence;
