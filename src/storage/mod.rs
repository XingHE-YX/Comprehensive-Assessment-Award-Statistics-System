mod attachments;

pub use attachments::{
    AttachmentStorage, MAX_ATTACHMENT_BYTES, MAX_ATTACHMENTS_PER_SUBMISSION, StorageError,
    StoredAttachment, UploadInput,
};

/// Immutable, safe directory for new uploads; display names can change freely.
pub fn academic_year_directory(academic_year_id: i64) -> String {
    format!("year-{academic_year_id}")
}
