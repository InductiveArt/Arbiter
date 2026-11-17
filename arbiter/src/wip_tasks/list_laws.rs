use task_runner::context::task_context::TaskContext;
use task_runner::declaration::task::Task;
use task_runner::record::task_failure::TaskFailure;

use crate::domain::law::LawName;
use crate::tasks::laws;

pub fn list_laws_task() -> Task {
    Task::new("list_laws", |context: &mut TaskContext, (): ()| {
        list_laws(context)
    })
    .line(
        "every law held over the graph: each of the core's laws, then each language's laws by \
         the language's name",
    )
}

fn list_laws(context: &mut TaskContext) -> Result<Vec<LawName>, TaskFailure> {
    let named: Vec<LawName> = laws::all()
        .iter()
        .map(|law| law.task.to_string())
        .chain(
            crate::packs::all()
                .iter()
                .map(|language| language.label().to_string()),
        )
        .map(|name| LawName { name })
        .collect();
    context.measure("laws", named.len() as f64);
    Ok(named)
}
