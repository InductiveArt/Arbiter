use task_runner::context::task_context::TaskContext;
use task_runner::declaration::pull::In;
use task_runner::declaration::task::Task;
use task_runner::record::task_failure::TaskFailure;

use crate::core::cone::cones;
use crate::core::graph_data::GraphData;
use crate::domain::unit_cone::UnitCone;

pub fn digest_unit_cones_task() -> Task {
    Task::new(
        "digest_unit_cones",
        |context: &mut TaskContext, graph: In<GraphData>| {
            let digested = cones(&graph.repo, &graph.defs, &graph.edges);
            context.measure("units", digested.len() as f64);
            Ok::<Vec<UnitCone>, TaskFailure>(digested)
        },
    )
    .line(
        "each unit's input cone as one digest: its own files, the files of every unit it stands \
         on through the graph, and every lockfile, each file by its path and content",
    )
}
