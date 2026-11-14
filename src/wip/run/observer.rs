use crate::wip::job::JobId;
use crate::wip::run::report::Report;
use crate::wip::trace::Outcome;
use crate::wip::trace::Step;

// What a run tells as it goes: an attempt out on its slot, a record landing,
// whether an attempt's, a replay's or one that never ran, a task's final
// outcome, and the whole report once the run ended.
pub enum Event<'a> {
    Started { job: &'a JobId, slot: usize },
    Ended { step: &'a Step },
    Settled { path: &'a str, outcome: &'a Outcome },
    Finished { report: &'a Report },
}

// Hears each event as it happens.
pub trait Observer {
    fn event(&mut self, event: Event<'_>);
}

// Hears everything and says nothing.
pub struct Quiet;

impl Observer for Quiet {
    fn event(&mut self, _event: Event<'_>) {}
}
