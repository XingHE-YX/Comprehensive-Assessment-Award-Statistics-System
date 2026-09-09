//! Fixed, reviewable workbook schema. No credentials or storage paths are exported.
mod xlsx;

use std::collections::BTreeMap;

use crate::domain::{AcademicYear, Attachment, StudentDeclaration, Submission};

pub use xlsx::export_xlsx;

pub struct ExportData {
    pub years: Vec<AcademicYear>,
    pub submissions: Vec<Submission>,
    pub declarations: Vec<StudentDeclaration>,
    pub attachments: BTreeMap<i64, Vec<Attachment>>,
}
