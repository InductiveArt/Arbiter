use std::time::Instant;

// One run's pipeline, timed end to end: every step leaves its seconds, the
// graph laws included. The trace is evidence - it changes no judgment.
pub struct Trace {
    clock: Instant,
    last: f64,
    steps: Vec<Step>,
}

pub struct Step {
    pub name: &'static str,
    pub seconds: f64,
}

impl Trace {
    pub fn start() -> Self {
        Trace {
            clock: Instant::now(),
            last: 0.0,
            steps: Vec::new(),
        }
    }

    // Marks the end of a step: everything since the last mark was it.
    pub fn step(&mut self, name: &'static str) {
        let now = self.clock.elapsed().as_secs_f64();
        self.steps.push(Step {
            name,
            seconds: now - self.last,
        });
        self.last = now;
    }

    // Records a step measured elsewhere, such as the engine's own phases or
    // a sum the outcomes already carry.
    pub fn note(&mut self, name: &'static str, seconds: f64) {
        self.steps.push(Step { name, seconds });
    }

    pub fn steps(self) -> Vec<Step> {
        self.steps
    }
}
