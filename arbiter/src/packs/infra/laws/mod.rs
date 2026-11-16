mod comment_overreach;
mod unpinned_image;

use crate::domain::finding::Finding;
use crate::packs::infra::reading::Reading;

// Every law the infra pack holds over its readings, in the order they are told.
pub fn instances(readings: &[Reading]) -> Vec<Finding> {
    let mut findings = unpinned_image::instances(readings);
    findings.extend(comment_overreach::instances(readings));
    findings
}
