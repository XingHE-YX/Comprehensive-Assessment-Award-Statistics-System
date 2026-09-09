use std::path::Path;

use crate::storage::{MAX_ATTACHMENT_BYTES, MAX_ATTACHMENTS_PER_SUBMISSION, UploadInput};

use super::ValidationErrors;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedUpload {
    pub original_name: String,
    pub mime_type: String,
    pub file_size: i64,
}

pub fn validate_upload(file: &UploadInput) -> Result<ValidatedUpload, ValidationErrors> {
    let mut errors = ValidationErrors::new();
    if file.bytes.is_empty() || file.bytes.len() as u64 > MAX_ATTACHMENT_BYTES {
        errors.add("attachments", "每个附件必须大于 0 且不超过 10 MiB");
    }

    let mime = file.mime_type.trim().to_ascii_lowercase();
    if !matches!(
        mime.as_str(),
        "image/jpeg" | "image/png" | "application/pdf"
    ) {
        errors.add("attachments", "附件仅支持 JPG、PNG 或 PDF 格式");
    }

    if file.original_name.is_empty()
        || file.original_name.len() > 255
        || file.original_name.contains('/')
        || file.original_name.contains('\\')
        || file.original_name == "."
        || file.original_name == ".."
        || file.original_name.chars().any(char::is_control)
    {
        errors.add("attachments", "附件文件名不合法");
    } else {
        let extension = Path::new(&file.original_name)
            .extension()
            .and_then(|value| value.to_str())
            .map(str::to_ascii_lowercase);
        let extension_matches = match mime.as_str() {
            "image/jpeg" => matches!(extension.as_deref(), Some("jpg" | "jpeg")),
            "image/png" => extension.as_deref() == Some("png"),
            "application/pdf" => extension.as_deref() == Some("pdf"),
            _ => false,
        };
        if !extension_matches {
            errors.add("attachments", "附件扩展名与文件类型不匹配");
        }
    }

    if errors.is_empty() {
        Ok(ValidatedUpload {
            original_name: file.original_name.clone(),
            mime_type: mime,
            file_size: file.bytes.len() as i64,
        })
    } else {
        Err(errors)
    }
}

pub fn validate_uploads_with_existing(
    uploads: &[UploadInput],
    existing_count: usize,
) -> Result<Vec<ValidatedUpload>, ValidationErrors> {
    let mut errors = ValidationErrors::new();
    if uploads.is_empty() && existing_count == 0 {
        errors.add("attachments", "成果申报至少需要上传 1 个证明材料");
    }
    if uploads.len().saturating_add(existing_count) > MAX_ATTACHMENTS_PER_SUBMISSION {
        errors.add("attachments", "每项成果最多上传 10 个证明材料");
    }

    let mut validated = Vec::with_capacity(uploads.len());
    for upload in uploads {
        match validate_upload(upload) {
            Ok(file) => validated.push(file),
            Err(file_errors) => errors.extend(file_errors),
        }
    }
    if errors.is_empty() {
        Ok(validated)
    } else {
        Err(errors)
    }
}
