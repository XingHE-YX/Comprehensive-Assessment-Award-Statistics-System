mod attachments;

pub use attachments::{
    AttachmentStorage, MAX_ATTACHMENT_BYTES, MAX_ATTACHMENTS_PER_SUBMISSION, StorageError,
    StoredAttachment, UploadInput,
};
