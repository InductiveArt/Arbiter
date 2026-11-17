use task_runner::context::task_context::TaskContext;
use task_runner::declaration::pull::In;
use task_runner::declaration::task::Task;
use task_runner::record::task_failure::TaskFailure;

use crate::core::graph_data::GraphData;
use crate::domain::law::LawFindings;
use crate::domain::law::LawName;
use crate::tasks::laws;

pub fn find_law_instances_task() -> Task {
    Task::new(
        "find_law_instances",
        |context: &mut TaskContext, (graph, law): (In<GraphData>, In<LawName>)| {
            find_law_instances(context, &graph, &law)
        },
    )
    .line(
        "one law held over the graph, its languages given back their readings: the instances \
         of one of the core's laws, or of one language's laws",
    )
}

fn find_law_instances(
    context: &mut TaskContext,
    data: &GraphData,
    law: &LawName,
) -> Result<LawFindings, TaskFailure> {
    let graph = data
        .clone()
        .into_graph(crate::packs::all())
        .map_err(TaskFailure::from)?;
    let findings = match laws::all().iter().find(|core| core.task == law.name) {
        Some(core) => (core.run)(&graph),
        None => match graph.packs.iter().find(|pack| pack.label() == law.name) {
            Some(language) => language.integrity(&graph.root, &graph.repo),
            None => return Err(TaskFailure::from(format!("no law is named {}", law.name))),
        },
    };
    context.measure("findings", findings.len() as f64);
    Ok(LawFindings {
        law: law.name.clone(),
        findings,
    })
}
