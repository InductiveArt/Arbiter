use task_runner::context::task_context::TaskContext;
use task_runner::declaration::pull::In;
use task_runner::declaration::task::Task;
use task_runner::record::task_failure::TaskFailure;

use crate::core::graph_data::GraphData;
use crate::ledger;
use crate::ledger::LedgerAccount;

pub fn account_ledger_task() -> Task {
    Task::new(
        "account_ledger",
        |context: &mut TaskContext, graph: In<GraphData>| account_ledger(context, &graph),
    )
    .line(
        "the ledger's account of the graph: every suppression counted, every third party the \
         lockfiles resolve, and how many of each the signed reference does not hold",
    )
}

fn account_ledger(
    context: &mut TaskContext,
    graph: &GraphData,
) -> Result<LedgerAccount, TaskFailure> {
    let graph = graph
        .clone()
        .into_graph(crate::packs::all())
        .map_err(TaskFailure::from)?;
    let account = ledger::account(&graph);
    context.measure("suppressions", account.counted.len() as f64);
    context.measure("third parties", account.third as f64);
    Ok(account)
}
