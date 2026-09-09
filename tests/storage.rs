use std::path::Path;

use tempfile::tempdir;
use tokio::io::AsyncReadExt;
use zongce_web::storage::{
    AttachmentStorage, MAX_ATTACHMENT_BYTES, MAX_ATTACHMENTS_PER_SUBMISSION, StorageError,
    UploadInput,
};

#[tokio::test]
async fn saves_with_random_name_and_reopens_inside_submission_directory() {
    let dir = tempdir().expect("temp dir");
    let storage = AttachmentStorage::new(dir.path());
    let saved = storage
        .save(
            "2025-2026学年",
            "ZC2026-000001",
            UploadInput::new("证明材料.pdf", "application/pdf", b"proof".to_vec()),
        )
        .await
        .expect("save");
    assert_eq!(saved.original_name, "证明材料.pdf");
    assert_eq!(saved.mime_type, "application/pdf");
    assert_eq!(saved.file_size, 5);
    assert!(!saved.stored_name.contains('/'));
    assert!(!saved.stored_name.contains("\\"));
    assert!(Path::new(&saved.stored_name).file_name().is_some());

    // Reconstruct storage to simulate a restart; legacy display-name directories
    // remain readable without the in-memory path cache.
    let reopened = AttachmentStorage::new(dir.path());
    let mut file = reopened.open(&saved.stored_name).await.expect("open");
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes).await.expect("read");
    assert_eq!(bytes, b"proof");
}

#[tokio::test]
async fn rejects_unsafe_names_unsupported_types_and_oversized_files() {
    let dir = tempdir().expect("temp dir");
    let storage = AttachmentStorage::new(dir.path());
    for name in ["../escape.pdf", "nested/file.pdf", "..\\escape.pdf"] {
        let result = storage
            .save(
                "year",
                "ZC2026-000001",
                UploadInput::new(name, "application/pdf", b"x".to_vec()),
            )
            .await;
        assert!(
            result.is_err(),
            "unsafe original name must be rejected: {name}"
        );
    }
    assert!(
        storage
            .save(
                "year",
                "ZC2026-000001",
                UploadInput::new("x.exe", "application/octet-stream", b"x".to_vec())
            )
            .await
            .is_err()
    );
    assert!(
        storage
            .save(
                "year",
                "ZC2026-000001",
                UploadInput::new(
                    "large.pdf",
                    "application/pdf",
                    vec![0; MAX_ATTACHMENT_BYTES as usize + 1]
                ),
            )
            .await
            .is_err()
    );
    assert!(storage.open("../escape.pdf").await.is_err());
    assert!(storage.open("arbitrary.pdf").await.is_err());
}

#[tokio::test]
async fn enforces_ten_attachment_limit_and_rolls_back_batch_on_failure() {
    let dir = tempdir().expect("temp dir");
    let storage = AttachmentStorage::new(dir.path());
    let uploads = (0..=MAX_ATTACHMENTS_PER_SUBMISSION)
        .map(|index| UploadInput::new(format!("{index}.pdf"), "application/pdf", b"x".to_vec()))
        .collect();
    assert!(matches!(
        storage.save_many("year", "submission", 0, uploads).await,
        Err(StorageError::TooManyAttachments)
    ));

    let uploads = vec![
        UploadInput::new("ok.pdf", "application/pdf", b"x".to_vec()),
        UploadInput::new("bad.exe", "application/octet-stream", b"x".to_vec()),
    ];
    assert!(
        storage
            .save_many("year", "submission", 0, uploads)
            .await
            .is_err()
    );
    let mut entries = tokio::fs::read_dir(dir.path().join("year").join("submission"))
        .await
        .expect("directory");
    assert!(entries.next_entry().await.expect("read").is_none());
}

#[test]
fn exposes_contract_limits_for_route_validation() {
    assert_eq!(MAX_ATTACHMENT_BYTES, 10 * 1024 * 1024);
    assert_eq!(MAX_ATTACHMENTS_PER_SUBMISSION, 10);
}
