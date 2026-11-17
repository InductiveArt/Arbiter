use task_runner::context::task_context::TaskContext;
use task_runner::declaration::pull::In;
use task_runner::declaration::task::Task;
use task_runner::record::task_failure::TaskFailure;

use crate::core::graph;
use crate::core::graph_data::GraphData;
use crate::core::units;
use crate::domain::folder::Folder;
use crate::domain::found_units::FoundUnits;

pub fn read_graph_task() -> Task {
    Task::new(
        "read_graph",
        |context: &mut TaskContext, (folder, found): (In<Folder>, In<FoundUnits>)| {
            read_graph(context, &folder, &found)
        },
    )
    .line(
        "the units found read into the graph: every file a language handles read for what it \
         defines and uses, each use resolved to the definitions it reaches, written out with \
         each language's readings",
    )
}

fn read_graph(
    context: &mut TaskContext,
    folder: &Folder,
    found: &FoundUnits,
) -> Result<GraphData, TaskFailure> {
    let packs = crate::packs::all();
    let repo = units::repo(found.clone(), &packs);
    let graph = graph::read(folder.path.clone(), repo, packs);
    context.measure("files", graph.repo.files.len() as f64);
    context.measure("definitions", graph.defs.len() as f64);
    context.measure("edges", graph.edges.len() as f64);
    GraphData::of(&graph).map_err(TaskFailure::from)
}
