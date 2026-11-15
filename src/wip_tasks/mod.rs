mod gitignore;
pub mod group_files_by_unit;
pub mod list_files;

use crate::wip::task::Task;
use crate::wip_tasks::group_files_by_unit::group_files_by_unit;
use crate::wip_tasks::list_files::list_files;

// Every task the gate is made of, listed one by one so a search for its name
// lands on its file.
pub fn listed() -> Vec<Task> {
    vec![list_files(), group_files_by_unit()]
}
