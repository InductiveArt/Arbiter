use std::path::Path;

use crate::core::graph::Graph;
use crate::core::task::Node;
use crate::domain::check_outcome::CheckOutcome;
use crate::domain::finding::Finding;
use crate::domain::repairs::Repairs;
use crate::ledger::LedgerAccount;

// One gate run as its report is written from: the graph it read, how each
// check ended in each unit, what the gate told along the way, every finding
// in the order of its file and line, the ledger's account, and the steps the
// run took with their cost.
pub struct GateRun {
    pub graph: Graph,
    pub outcomes: Vec<CheckOutcome>,
    pub notes: Vec<String>,
    pub findings: Vec<Finding>,
    pub ledger: LedgerAccount,
    pub steps: Vec<Node>,
}

impl GateRun {
    pub fn root(&self) -> &Path {
        &self.graph.root
    }

    pub fn units(&self) -> usize {
        self.graph.repo.units.len()
    }

    pub fn files(&self) -> usize {
        self.graph.repo.files.len()
    }

    pub fn failed(&self) -> usize {
        self.outcomes
            .iter()
            .filter(|outcome| !outcome.passed)
            .count()
    }

    // No check ran and no file was there to read: nothing was gated.
    pub fn gated_nothing(&self) -> bool {
        self.outcomes.is_empty() && self.files() == 0
    }

    pub fn clean(&self) -> bool {
        self.failed() == 0 && self.findings.is_empty()
    }

    // The run after a fixing run's repairs: their notes after its own, and
    // each repair that did not succeed standing for its check in the same
    // unit, in place of the check's outcome or after the unit's last one.
    pub fn repaired(mut self, repairs: Repairs) -> GateRun {
        self.notes.extend(repairs.notes);
        for failed in repairs.failed {
            let same = self
                .outcomes
                .iter()
                .position(|outcome| outcome.check == failed.check && outcome.unit == failed.unit);
            match same {
                Some(at) => self.outcomes[at] = failed,
                None => {
                    let at = self
                        .outcomes
                        .iter()
                        .rposition(|outcome| outcome.unit == failed.unit)
                        .map_or(self.outcomes.len(), |at| at + 1);
                    self.outcomes.insert(at, failed);
                }
            }
        }
        self
    }
}
