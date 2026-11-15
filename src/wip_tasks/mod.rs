mod gitignore;
pub mod group_files_by_unit;
pub mod list_files;
pub mod read_markers;

use crate::wip::task::Task;
use crate::wip_tasks::group_files_by_unit::group_files_by_unit_task;
use crate::wip_tasks::list_files::list_files_task;
use crate::wip_tasks::read_markers::read_markers_task;

// Every task the gate is made of, listed one by one so a search for its name
// lands on its file.
pub fn listed() -> Vec<Task> {
    vec![
        list_files_task(),
        read_markers_task(),
        group_files_by_unit_task(),
    ]
}
