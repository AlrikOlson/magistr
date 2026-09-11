use sha2::{Digest, Sha256};

/// A file snapshot whose bytes survive classification and hashing unchanged.
#[derive(Clone, Debug)]
pub enum FileContent {
    Text(String),
    Binary(Vec<u8>),
}

impl FileContent {
    /// Decode a snapshot without deriving its type from its filename.
    #[must_use]
    pub fn decode(bytes: Vec<u8>) -> Self {
        if bytes.contains(&0) {
            return Self::Binary(bytes);
        }
        match String::from_utf8(bytes) {
            Ok(text) => Self::Text(text),
            Err(error) => Self::Binary(error.into_bytes()),
        }
    }

    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        match self {
            Self::Text(text) => text.as_bytes(),
            Self::Binary(bytes) => bytes,
        }
    }

    /// SHA-256 of the original bytes, independent of the content variant.
    #[must_use]
    pub fn content_hash(&self) -> String {
        format!("{:x}", Sha256::digest(self.as_bytes()))
    }
}

impl PartialEq for FileContent {
    fn eq(&self, other: &Self) -> bool {
        self.as_bytes() == other.as_bytes()
    }
}

impl Eq for FileContent {}

#[cfg(test)]
#[path = "file_content.tests.rs"]
mod tests;
