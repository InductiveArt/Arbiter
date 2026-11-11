use std::env;
use std::fs;
use std::path::Path;
use std::process::ExitCode;

use arbiter::commands;
use arbiter::core::claims;
use arbiter::core::pack;
use arbiter::core::pack::Def;
use arbiter::core::pack::Edge;
use arbiter::core::pack::Finding;
use arbiter::core::pack::Suppression;
use arbiter::examine;
use arbiter::laws::findings::drawn;
use arbiter::laws::findings::integrity;
use arbiter::laws::findings::winnow;
use arbiter::laws::rhyme;
use arbiter::ledger;
use arbiter::packs::graphed;
use arbiter::pipeline::outcomes;
use arbiter::pipeline::verdict;
use arbiter::records::brief;

fn main() -> ExitCode {
    // A closed pipe ends the read, not the program: piping a report into
    // `head` is a normal way to read it, never a panic.
    #[cfg(unix)]
    unsafe {
        unsafe extern "C" {
            fn signal(number: i32, handler: usize) -> usize;
        }
        // SIGPIPE is 13 and the default disposition is 0 on every unix here.
        signal(13, 0);
    }
    let arguments: Vec<String> = env::args().skip(1).collect();
    if arguments.first().map(String::as_str) == Some("fix") {
        eprintln!("fix became a mode: arbiter gate --fix");
        return ExitCode::from(2);
    }
    if let Some(command @ "gate") = arguments.first().map(String::as_str) {
        let cold = arguments.iter().any(|argument| argument == "--cold");
        let fixing = arguments.iter().any(|argument| argument == "--fix");
        let briefless = arguments.iter().any(|argument| argument == "--no-brief");
        let asked = arguments
            .iter()
            .skip(1)
            .find(|argument| !argument.starts_with("--"))
            .cloned()
            .unwrap_or_else(|| ".".to_string());
        let gaps = outcomes::preflight(Path::new(&asked));
        if !gaps.is_empty() {
            for gap in &gaps {
                eprintln!("{command}: {gap}");
            }
            eprintln!(
                "{command}: a machine gap is no finding; run the bootstrap, then judge again"
            );
            return ExitCode::from(2);
        }
        let Some(run) = verdict::run(&asked, fixing, cold, briefless) else {
            eprintln!("{command}: no folder at {asked}");
            return ExitCode::from(2);
        };
        println!("{command}: {} units, {} files{}", run.units, run.files, {
            let foreign = &run.graph.repo.foreign;
            if foreign.is_empty() {
                String::new()
            } else {
                let named: Vec<String> = foreign
                    .iter()
                    .map(|place| place.to_string_lossy().to_string())
                    .collect();
                format!(" (foreign: {})", named.join(", "))
            }
        });
        for (check, (passed, failed, warm, seconds)) in verdict::tallied(&run.outcomes) {
            let heat = if warm > 0 {
                format!(" ({warm} warm)")
            } else {
                String::new()
            };
            if failed == 0 {
                println!("  {check:15} ok     {passed} units{heat}   {seconds:.1}s");
            } else {
                println!(
                    "  {check:15} failed {failed} of {}{heat}   {seconds:.1}s",
                    passed + failed
                );
            }
        }
        let mut counts: std::collections::BTreeMap<&str, usize> = Default::default();
        for finding in &run.findings {
            *counts.entry(finding.check).or_insert(0) += 1;
        }
        for (check, count) in &counts {
            println!("  {check:15} {count} findings");
        }
        match run.added {
            None => println!(
                "  {:15} {} counted, no reference recorded",
                "suppressions", run.counted
            ),
            Some(added) => println!(
                "  {:15} {} counted, {added} added since the signature",
                "suppressions", run.counted
            ),
        }
        match run.third_added {
            None => println!(
                "  {:15} {} locked, no reference recorded",
                "third parties", run.third
            ),
            Some(added) => println!(
                "  {:15} {} locked, {added} added since the signature",
                "third parties", run.third
            ),
        }
        match run.claims_vanished {
            None => println!(
                "  {:15} {} spoken, no reference recorded",
                "claims", run.claims
            ),
            Some(0) => println!("  {:15} {} spoken, none vanished", "claims", run.claims),
            Some(vanished) => println!(
                "  {:15} {} spoken, {vanished} vanished since the signature",
                "claims", run.claims
            ),
        }
        match run.needs_added {
            None => println!(
                "  {:15} {} stated, no reference recorded",
                "needs", run.needs_stated
            ),
            Some(added) => println!(
                "  {:15} {} stated, {added} added since the signature",
                "needs", run.needs_stated
            ),
        }
        match run.brief {
            verdict::BriefLine::Skipped => println!("  {:15} skipped", "brief"),
            verdict::BriefLine::Unwritten => println!("  {:15} no brief written", "brief"),
            verdict::BriefLine::Read {
                cases,
                tested,
                planned,
                manual,
                unanswered,
                unwired,
            } => println!(
                "  {:15} {cases} cases — {tested} tested, {planned} planned, \
                 {manual} manual, {unanswered} unanswered; {unwired} claims unwired",
                "brief"
            ),
        }
        for note in &run.notes {
            println!("{command}: {note}");
        }
        if run.outcomes.is_empty() && run.files == 0 {
            println!("{command}: nothing to judge here");
            return ExitCode::from(2);
        }
        let place = verdict::written(&run);
        let _ = arbiter::court::write(&run);
        let failed = run
            .outcomes
            .iter()
            .filter(|outcome| !outcome.passed)
            .count();
        if failed == 0 && run.findings.is_empty() {
            println!(
                "{command}: clean — {} checks, no findings",
                run.outcomes.len()
            );
            return ExitCode::SUCCESS;
        }
        let shown = place
            .and_then(|path| path.strip_prefix(&run.root).ok().map(Path::to_path_buf))
            .map(|path| path.to_string_lossy().to_string())
            .unwrap_or_else(|| "the report could not be written".to_string());
        println!(
            "{command}: {failed} checks failed, {} findings — {shown}",
            run.findings.len()
        );
        return ExitCode::FAILURE;
    }
    if arguments.first().map(String::as_str) == Some("find") {
        return found(&arguments[1..]);
    }
    if arguments.first().map(String::as_str) == Some("brief") {
        let (case, asked) = match (arguments.get(1), arguments.get(2)) {
            (Some(first), second) if !Path::new(first).is_dir() => (
                Some(first.clone()),
                second.cloned().unwrap_or_else(|| ".".to_string()),
            ),
            (first, _) => (None, first.cloned().unwrap_or_else(|| ".".to_string())),
        };
        let Some(found) = graphed(&asked) else {
            eprintln!("brief: no folder at {asked}");
            return ExitCode::from(2);
        };
        let Some(reading) = brief::read(&found) else {
            return unwritten(&found);
        };
        if let Some(case) = case {
            return slice(&case, &reading);
        }
        println!(
            "brief: {} cases — {} tested, {} planned, {} manual, {} unanswered; \
             {} claims, {} unwired",
            reading.cases.len(),
            reading
                .needs
                .iter()
                .filter(|need| need.state == brief::State::Tested)
                .count(),
            reading
                .needs
                .iter()
                .filter(|need| need.state == brief::State::Planned)
                .count(),
            reading
                .needs
                .iter()
                .filter(|need| need.state == brief::State::Manual)
                .count(),
            reading
                .needs
                .iter()
                .filter(|need| need.state == brief::State::Unanswered)
                .count(),
            reading.claims,
            reading.unwired.len(),
        );
        let mut case = String::new();
        for need in &reading.needs {
            if need.case != case {
                println!();
                println!("{}", need.case);
                case = need.case.clone();
            }
            let state = match need.state {
                brief::State::Tested => "tested",
                brief::State::Planned => "planned",
                brief::State::Manual => "manual",
                brief::State::Unanswered => "unanswered",
            };
            println!("  {state:10} {}", need.need);
        }
        if !reading.unwired.is_empty() {
            println!();
            println!("unwired claims, each with the binding to paste:");
            for (file, line, identity) in &reading.unwired {
                println!("  {}:{line} \"{identity}\"", file.to_string_lossy());
            }
        }
        return ExitCode::SUCCESS;
    }
    if arguments.first().map(String::as_str) == Some("examine") {
        let asked = arguments.get(1).cloned().unwrap_or_else(|| ".".to_string());
        let Some(found) = graphed(&asked) else {
            eprintln!("examine: no folder at {asked}");
            return ExitCode::from(2);
        };
        match examine::run(&found) {
            Ok(testimonies) => {
                let mut refuted = false;
                for testimony in &testimonies {
                    println!("case {} — sequence {}", testimony.case, examine::SEQUENCE);
                    for step in &testimony.steps {
                        let mark = if step.pass { "✓" } else { "✗" };
                        println!("  {mark} {} — {}", step.act, step.saw);
                    }
                    let verdict = if testimony.passed {
                        "verified"
                    } else {
                        "refuted"
                    };
                    for need in &testimony.needs {
                        println!("  {verdict}: {need}");
                    }
                    refuted |= !testimony.passed;
                }
                println!(
                    "examine: receipts — {}",
                    found
                        .root
                        .join(".arbiter")
                        .join("last")
                        .join("receipts.json")
                        .display()
                );
                return ExitCode::from(u8::from(refuted));
            }
            Err(reason) => {
                eprintln!("examine: {reason}");
                return ExitCode::from(2);
            }
        }
    }
    if arguments.first().map(String::as_str) == Some("adopt") {
        let asked = arguments.get(1).cloned().unwrap_or_else(|| ".".to_string());
        let root = std::path::PathBuf::from(&asked);
        if !root.is_dir() {
            eprintln!("adopt: no folder at {asked}");
            return ExitCode::from(2);
        }
        let (told, stale) = adopted(&root);
        for line in told {
            println!("adopt: {line}");
        }
        if stale {
            eprintln!("adopt: a stale binary is a machine gap; update arbiter, then adopt again");
            return ExitCode::from(2);
        }
        return ExitCode::SUCCESS;
    }
    if arguments.first().map(String::as_str) == Some("rules") {
        return match arguments.get(1) {
            None => {
                matrix();
                ExitCode::SUCCESS
            }
            Some(rule) => ruled(rule),
        };
    }
    if arguments.first().map(String::as_str) == Some("ledger") {
        if arguments.get(1).map(String::as_str) == Some("sign") {
            let asked = arguments.get(2).cloned().unwrap_or_else(|| ".".to_string());
            let Some(found) = graphed(&asked) else {
                eprintln!("sign: no folder at {asked}");
                return ExitCode::from(2);
            };
            let blankets = ledger::blankets(&found);
            if !blankets.is_empty() {
                eprintln!(
                    "sign: {} blanket suppressions are findings, never records",
                    blankets.len()
                );
                for suppression in &blankets {
                    eprintln!("  {}", shown(suppression));
                }
                return ExitCode::FAILURE;
            }
            let counted = ledger::counted(&found);
            let packages = ledger::packages(&found);
            let spoken = claims::identities(&found);
            let demanded: Vec<String> = brief::read(&found)
                .map(|reading| {
                    reading
                        .needs
                        .iter()
                        .map(|need| format!("{} | {}", need.case, need.need))
                        .collect()
                })
                .unwrap_or_default();
            match ledger::record(&found.root, &counted, &packages, &spoken, &demanded) {
                Ok(path) => {
                    println!(
                        "sign: {} suppressions, {} third parties, {} claims and {} needs recorded in {}",
                        counted.len(),
                        packages.len(),
                        spoken.len(),
                        demanded.len(),
                        path.to_string_lossy()
                    );
                    for pack in &found.packs {
                        for line in pack.record(&found.root, &found.repo) {
                            println!("sign: {line}");
                        }
                    }
                    return ExitCode::SUCCESS;
                }
                Err(error) => {
                    eprintln!("sign: the reference could not be written: {error}");
                    return ExitCode::from(2);
                }
            }
        }
        let asked = arguments.get(1).cloned().unwrap_or_else(|| ".".to_string());
        let Some(found) = graphed(&asked) else {
            eprintln!("ledger: no folder at {asked}");
            return ExitCode::from(2);
        };
        return ledgered(&found);
    }
    if let Some(command) = arguments.first() {
        eprintln!("arbiter: no command named {command}");
    }
    taught();
    ExitCode::from(2)
}

// The one query door: a noun names a report, a rule names one law's instances,
// bare is every tool-free finding. A find reads and never writes.
fn found(rest: &[String]) -> ExitCode {
    let folder = |at: usize| rest.get(at).cloned().unwrap_or_else(|| ".".to_string());
    match rest.first().map(String::as_str) {
        Some("fallout") => {
            let Some(name) = rest.get(1) else {
                eprintln!("find fallout: name the symbol to look up");
                return ExitCode::from(2);
            };
            let asked = folder(2);
            let Some(found) = graphed(&asked) else {
                eprintln!("find: no folder at {asked}");
                return ExitCode::from(2);
            };
            fallout(name, &found.defs, &found.edges);
            ExitCode::SUCCESS
        }
        Some("deps") => {
            let Some(name) = rest.get(1) else {
                let Some(found) = graphed(".") else {
                    eprintln!("find: no folder at .");
                    return ExitCode::from(2);
                };
                return drawn_graph(&found);
            };
            let asked = folder(2);
            let Some(found) = graphed(&asked) else {
                eprintln!("find: no folder at {asked}");
                return ExitCode::from(2);
            };
            deps(name, &found)
        }
        Some("rhymes") => {
            let asked = folder(1);
            let Some(found) = graphed(&asked) else {
                eprintln!("find: no folder at {asked}");
                return ExitCode::from(2);
            };
            rhymed(&found)
        }
        Some("tangles") => {
            let scope = rest.get(1).cloned();
            let Some(found) = graphed(".") else {
                eprintln!("find: no folder at .");
                return ExitCode::from(2);
            };
            tangled(scope.as_deref(), &found)
        }
        Some("plan") => {
            let Some(found) = graphed(".") else {
                eprintln!("find: no folder at .");
                return ExitCode::from(2);
            };
            planned(rest.get(1).map(String::as_str), &found)
        }
        Some("impact") => {
            let Some(name) = rest.get(1) else {
                eprintln!("find impact: name the service to take down");
                return ExitCode::from(2);
            };
            let Some(found) = graphed(".") else {
                eprintln!("find: no folder at .");
                return ExitCode::from(2);
            };
            fallen(name, &found)
        }
        first => {
            let (rule, asked) = match first {
                None => (None, ".".to_string()),
                Some(word) if pack::findable(word) => (Some(word.to_string()), folder(1)),
                Some(word) if pack::drillable(word) => {
                    eprintln!(
                        "find: {word} fires through the gate's outcomes; \
                         run arbiter gate . and read .arbiter/last/report.md"
                    );
                    return ExitCode::from(2);
                }
                Some(word) if Path::new(word).is_dir() => (None, word.to_string()),
                Some(word) => {
                    eprintln!(
                        "find: nothing named {word}; find takes a rule from arbiter rules, \
                         fallout <name>, deps [unit], plan [name], impact <service>, rhymes or tangles"
                    );
                    return ExitCode::from(2);
                }
            };
            let Some(found) = graphed(&asked) else {
                eprintln!("find: no folder at {asked}");
                return ExitCode::from(2);
            };
            let commissioned = brief::commissioned(&found);
            let (winnowed, _) = winnow(&found.defs, &found.edges, &found.repo, &commissioned);
            let mut all = winnowed;
            all.extend(integrity(&found));
            all.extend(ledger::blanket_findings(&found));
            all.extend(ledger::broken_record(&found.root));
            all.extend(claims::vanished(&found));
            all.extend(brief::findings(&found));
            all.sort_by(|a, b| (&a.file, a.line).cmp(&(&b.file, b.line)));
            emit(&all, rule.as_deref());
            ExitCode::SUCCESS
        }
    }
}

// The signature's account: everything signing records, counted against what
// stands today, with each delta listed where one exists.
fn ledgered(found: &arbiter::core::graph::Graph) -> ExitCode {
    let counted = ledger::counted(found);
    let reference = ledger::reference(&found.root);
    match &reference {
        None => {
            println!("{:14} {} counted", "suppressions", counted.len());
            for suppression in &counted {
                println!("  {}", shown(suppression));
            }
        }
        Some(reference) => {
            let extra = ledger::added(&counted, reference);
            println!(
                "{:14} {} counted, {} added since the signature",
                "suppressions",
                counted.len(),
                extra.len()
            );
            for suppression in &extra {
                println!("  {}", shown(suppression));
            }
        }
    }
    let packages = ledger::packages(found);
    match ledger::recorded_packages(&found.root) {
        None => println!("{:14} {} locked", "third parties", packages.len()),
        Some(recorded) => {
            let fresh: Vec<&String> = packages
                .iter()
                .filter(|name| !recorded.contains(*name))
                .collect();
            println!(
                "{:14} {} locked, {} added since the signature",
                "third parties",
                packages.len(),
                fresh.len()
            );
            for name in fresh {
                println!("  {name}");
            }
        }
    }
    let spoken = claims::identities(found).len();
    match claims::recorded(&found.root) {
        None => println!("{:14} {} spoken", "claims", spoken),
        Some(_) => {
            let lost = claims::gone(found);
            println!(
                "{:14} {} spoken, {} vanished since the signature",
                "claims",
                spoken,
                lost.len()
            );
            for claim in &lost {
                println!("  '{claim}'");
            }
        }
    }
    let blankets = ledger::blankets(found);
    if !blankets.is_empty() {
        println!(
            "{:14} {} refused, never recorded",
            "blankets",
            blankets.len()
        );
        for suppression in &blankets {
            println!("  {}", shown(suppression));
        }
    }
    if reference.is_none() {
        println!("no reference recorded; arbiter ledger sign records today's");
    }
    ExitCode::SUCCESS
}

fn rhymed(found: &arbiter::core::graph::Graph) -> ExitCode {
    let groups = rhyme::rhymes(found);
    if groups.is_empty() {
        println!("rhymes: nothing repeats at function grain");
        return ExitCode::SUCCESS;
    }
    let plural = if groups.len() == 1 { "" } else { "s" };
    println!(
        "rhymes: {} shape{plural} sounding in more than one place",
        groups.len()
    );
    for group in groups {
        let kind = if group.perfect { "perfect" } else { "slant" };
        println!();
        println!(
            "{kind}, {} places, {} leaves:",
            group.places.len(),
            group.leaves
        );
        for place in group.places {
            let title = if place.title.is_empty() {
                String::new()
            } else {
                format!(" {}", place.title)
            };
            println!("  {}:{}{}", place.file.to_string_lossy(), place.line, title);
        }
    }
    ExitCode::SUCCESS
}

// The bare call teaches: the whole grammar, each command with its one line.
fn taught() {
    println!(
        "arbiter {} judges a repository: the resolved graph, each unit's pinned tools, the brief.",
        commands::VERSION
    );
    println!();
    let width = commands::COMMANDS
        .iter()
        .map(|(call, _)| call.len())
        .max()
        .unwrap_or(0);
    for (call, line) in commands::COMMANDS {
        println!("  {call:width$}  {line}");
    }
    println!();
    println!(
        "gate, examine, adopt and ledger sign act; find, ledger, brief and rules read and never write."
    );
    println!(
        "A command takes the folder last; missing, it judges here. \
         Exit: 0 clean, 1 findings, 2 no judgment — a broken machine, \
         a bad ask, or nothing to judge."
    );
}

// With no brief written, the whole tree is the unwired world: the claims an
// agent reads to draft the first cases.
fn unwritten(found: &arbiter::core::graph::Graph) -> ExitCode {
    let (claimed, silent) = claims::heard(found);
    let behaviors: usize = claimed.values().map(Vec::len).sum();
    println!("brief: no brief written; {behaviors} claims unwired");
    for (unit, list) in &claimed {
        let shown = if unit.as_os_str().is_empty() {
            ".".to_string()
        } else {
            unit.to_string_lossy().to_string()
        };
        println!();
        println!("{shown} - {}", list.len());
        let mut last = None;
        let mut told: Vec<String> = Vec::new();
        for one in list {
            if last != Some(&one.file) {
                println!("  {}", one.file.to_string_lossy());
                last = Some(&one.file);
                told.clear();
            }
            for (depth, frame) in one.claim.path.iter().enumerate() {
                if told.len() > depth && told[depth] == *frame {
                    continue;
                }
                told.truncate(depth);
                told.push(frame.clone());
                println!("{}{frame}", "  ".repeat(depth + 2));
            }
            told.truncate(one.claim.path.len());
            println!(
                "{}{}",
                "  ".repeat(one.claim.path.len() + 2),
                one.claim.sentence
            );
        }
    }
    if !silent.is_empty() {
        let names: Vec<String> = silent
            .iter()
            .map(|unit| {
                if unit.as_os_str().is_empty() {
                    ".".to_string()
                } else {
                    unit.to_string_lossy().to_string()
                }
            })
            .collect();
        println!();
        println!("claims nothing: {}", names.join(", "));
    }
    ExitCode::SUCCESS
}

// One case whole: its needs, their states, and what each binds.
fn slice(case: &str, reading: &arbiter::records::brief::Reading) -> ExitCode {
    let needs: Vec<&arbiter::records::brief::Need> = reading
        .needs
        .iter()
        .filter(|need| need.case == case)
        .collect();
    if needs.is_empty() {
        eprintln!("brief: no case named {case}");
        return ExitCode::from(2);
    }
    println!("{case} — {} needs", needs.len());
    for need in needs {
        let state = match need.state {
            brief::State::Tested => "tested",
            brief::State::Planned => "planned",
            brief::State::Manual => "manual",
            brief::State::Unanswered => "unanswered",
        };
        println!();
        println!("  {state:10} {}", need.need);
        for entry in &need.entries {
            println!("             {entry}");
        }
    }
    ExitCode::SUCCESS
}

fn shown(suppression: &Suppression) -> String {
    format!(
        "{}:{} {}",
        suppression.file.to_string_lossy(),
        suppression.line,
        suppression.text
    )
}

fn emit(findings: &[Finding], rule: Option<&str>) {
    for finding in findings {
        if rule.is_some_and(|rule| finding.name != rule) {
            continue;
        }
        let shown = finding.file.to_string_lossy();
        println!(
            "{}",
            serde_json::json!({
                "check": finding.check,
                "file": shown,
                "line": finding.line,
                "rule": finding.name,
                "reason": finding.reason,
            })
        );
    }
}

// One plan's bring-up as a ladder - rung by rung, nothing running - and its
// channels; bare, every plan the record holds. The view of the desired side.
fn planned(asked: Option<&str>, found: &arbiter::core::graph::Graph) -> ExitCode {
    let reading = match arbiter::records::plans::read(&found.root) {
        None => {
            println!("plans: none written");
            return ExitCode::SUCCESS;
        }
        Some(Err(broken)) => {
            eprintln!("find plan: {}", broken.reason);
            return ExitCode::from(2);
        }
        Some(Ok(reading)) => reading,
    };
    let mut shown = 0;
    for plan in &reading.plans {
        if asked.is_some_and(|name| name != plan.name) {
            continue;
        }
        shown += 1;
        println!("plan {}", plan.name);
        let spoken = |name: &str| -> String {
            let count = reading
                .services
                .iter()
                .find(|service| service.name == name)
                .map_or(1, |service| service.replicas.max(1));
            if count > 1 {
                format!("{name} x{count}")
            } else {
                name.to_string()
            }
        };
        let rungs = arbiter::records::plans::ladder(&reading, plan);
        for (rung, services) in rungs.iter().enumerate() {
            let step: Vec<String> = services
                .iter()
                .map(|(service, machine)| format!("{} on {machine}", spoken(service)))
                .collect();
            println!("  up   {}  {}", rung + 1, step.join(", "));
        }
        for (rung, services) in rungs.iter().enumerate().rev() {
            let step: Vec<String> = services
                .iter()
                .map(|(service, _)| spoken(service))
                .collect();
            println!("  down {}  {}", rungs.len() - rung, step.join(", "));
        }
        for service in &reading.services {
            for (domain, port, to) in &service.serves {
                println!("  door {domain}:{port} -> {to}  via {}", service.name);
            }
        }
        for service in &reading.services {
            for (to, contract) in &service.channels {
                match contract {
                    Some(contract) => {
                        println!("  channel {} -> {to}  {contract}", service.name);
                    }
                    None => println!("  channel {} -> {to}", service.name),
                }
            }
        }
    }
    if shown == 0 {
        match asked {
            Some(name) => {
                eprintln!("find plan: no plan named {name}");
                return ExitCode::from(2);
            }
            None => println!("plans: services declared, no plan written yet"),
        }
    }
    ExitCode::SUCCESS
}

// Every sibling cycle the facts hold, suppressed or not, largest first: the
// map an agent reads before representing - a measure, never findings.
fn tangled(scope: Option<&str>, found: &arbiter::core::graph::Graph) -> ExitCode {
    let groups = arbiter::laws::findings::tangles(found);
    let mut shown = 0;
    for (place, members) in &groups {
        if scope.is_some_and(|asked| !(place == asked || place.starts_with(&format!("{asked}/")))) {
            continue;
        }
        println!("{place}: {}", members.join(" <-> "));
        shown += 1;
    }
    if shown == 0 {
        println!("tangles: none");
    }
    ExitCode::SUCCESS
}

// The whole resolved unit graph as one mermaid fence, declared against real:
// an edge the declarations refuse is drawn dashed and named undeclared. A
// measure, never findings, so the fence pastes into any page the host draws.
fn drawn_graph(found: &arbiter::core::graph::Graph) -> ExitCode {
    let edges = drawn(found);
    let mut ids: std::collections::BTreeMap<&str, usize> = Default::default();
    for (user, owner, _) in &edges {
        let next = ids.len();
        ids.entry(user.as_str()).or_insert(next);
        let next = ids.len();
        ids.entry(owner.as_str()).or_insert(next);
    }
    println!("```mermaid");
    println!("flowchart TD");
    for (name, id) in &ids {
        println!("  u{id}[\"{name}\"]");
    }
    for (user, owner, breaks) in &edges {
        let from = ids[user.as_str()];
        let to = ids[owner.as_str()];
        if *breaks {
            println!("  u{from} -. undeclared .-> u{to}");
        } else {
            println!("  u{from} --> u{to}");
        }
    }
    if edges.is_empty() {
        println!("  %% no cross-unit edges resolved");
    }
    println!("  %% test units are not drawn");
    println!("```");
    ExitCode::SUCCESS
}

// One unit's whole dependency picture, every language in the one format: the
// written edges beside the observed ones, and the third parties its locks name.
fn deps(name: &str, found: &arbiter::core::graph::Graph) -> ExitCode {
    let shown_unit = |unit: usize| {
        let path = &found.repo.units[unit].path;
        if path.as_os_str().is_empty() {
            ".".to_string()
        } else {
            path.to_string_lossy().to_string()
        }
    };
    let Some(unit) = (0..found.repo.units.len()).find(|&unit| shown_unit(unit) == *name) else {
        return scoped(name, found);
    };
    let mut uses = std::collections::BTreeSet::new();
    let mut users = std::collections::BTreeSet::new();
    for edge in &found.edges {
        let owner = found.defs[edge.def].unit;
        if owner == edge.unit {
            continue;
        }
        if edge.unit == unit {
            uses.insert(shown_unit(owner));
        }
        if owner == unit {
            users.insert(shown_unit(edge.unit));
        }
    }
    println!("deps: {name}");
    let listed = |set: &std::collections::BTreeSet<String>| {
        if set.is_empty() {
            "nothing".to_string()
        } else {
            set.iter().cloned().collect::<Vec<_>>().join(", ")
        }
    };
    println!("  uses: {}", listed(&uses));
    println!("  used by: {}", listed(&users));
    let declared = fs::read_to_string(found.root.join(".arbiter/units.toml"))
        .ok()
        .and_then(|text| text.parse::<toml::Value>().ok());
    let entry = declared
        .as_ref()
        .and_then(|parsed| parsed.get("units"))
        .and_then(|units| units.get(name));
    let names = |value: Option<&toml::Value>| -> Option<Vec<String>> {
        Some(
            value?
                .as_array()?
                .iter()
                .filter_map(toml::Value::as_str)
                .map(str::to_string)
                .collect(),
        )
    };
    match names(entry.and_then(|entry| entry.get("deps"))) {
        Some(deps) => println!("  declares: {}", deps.join(", ")),
        None => println!("  declares: nothing"),
    }
    match names(entry.and_then(|entry| entry.get("visibility"))) {
        Some(grants) => println!("  grants: {}", grants.join(", ")),
        None => println!("  grants: everyone; no visibility is declared"),
    }
    for pack in &found.packs {
        let locks = pack.lockfiles();
        if locks.is_empty() {
            continue;
        }
        for path in &found.repo.lockfiles {
            let Some(name) = path.file_name().map(|name| name.to_string_lossy()) else {
                continue;
            };
            if !locks.contains(&name.as_ref()) {
                continue;
            }
            if found.repo.unit_of(path) != unit {
                continue;
            }
            let Ok(text) = fs::read_to_string(found.root.join(path)) else {
                continue;
            };
            let locked = pack.locked(&name, &text);
            println!(
                "  third parties ({}): {} locked",
                pack.label(),
                locked.len()
            );
            for (name, version) in locked {
                println!("    {name} {version}");
            }
        }
    }
    ExitCode::SUCCESS
}

// A folder or file inside a unit: the same read at its own grain — each file
// it reaches beyond itself, and each outside file standing on it.
fn scoped(asked: &str, found: &arbiter::core::graph::Graph) -> ExitCode {
    let scope = Path::new(asked.trim_start_matches("./"))
        .components()
        .collect::<std::path::PathBuf>();
    if scope.as_os_str().is_empty() || !found.root.join(&scope).exists() {
        eprintln!("deps: no unit or path named {asked}");
        return ExitCode::from(2);
    }
    let within = |file: &Path| file.starts_with(&scope);
    let mut uses = std::collections::BTreeSet::new();
    let mut users = std::collections::BTreeSet::new();
    for edge in &found.edges {
        let owner = &found.defs[edge.def].file;
        if within(&edge.file) && !within(owner) {
            uses.insert(owner.to_string_lossy().to_string());
        }
        if within(owner) && !within(&edge.file) {
            users.insert(edge.file.to_string_lossy().to_string());
        }
    }
    println!("deps: {}", scope.display());
    for (title, set) in [("uses", &uses), ("used by", &users)] {
        if set.is_empty() {
            println!("  {title}: nothing");
            continue;
        }
        println!("  {title}:");
        for file in set {
            println!("    {file}");
        }
    }
    ExitCode::SUCCESS
}

// What falls when one service is down, and in what capacity: who cannot
// boot, who degrades, which doors go dark, what state turns unreachable,
// and which cases are placed over it.
fn fallen(name: &str, found: &arbiter::core::graph::Graph) -> ExitCode {
    let Some(Ok(reading)) = arbiter::records::plans::read(&found.root) else {
        eprintln!("find impact: no plans record; impact runs over the declared world");
        return ExitCode::from(2);
    };
    if !reading.services.iter().any(|service| service.name == *name) {
        eprintln!("find impact: no service named {name} in the plans");
        return ExitCode::from(2);
    }
    println!("if {name} is down:");
    let (boot, dials) = arbiter::records::plans::leaning_on(&reading, name);
    for service in &boot {
        println!("  cannot boot: {service} — needs it");
    }
    for (service, contract) in &dials {
        match contract {
            Some(contract) => println!("  degraded: {service} — dials it ({contract})"),
            None => println!("  degraded: {service} — dials it"),
        }
    }
    let direct: std::collections::BTreeSet<&String> = boot
        .iter()
        .chain(dials.iter().map(|(who, _)| who))
        .collect();
    let beyond: Vec<String> = arbiter::records::plans::impacted(&reading, name)
        .into_iter()
        .filter(|service| !direct.contains(service))
        .collect();
    if !beyond.is_empty() {
        println!("  and standing above those: {}", beyond.join(", "));
    }
    if direct.is_empty() && beyond.is_empty() {
        println!("  no service stands on it");
    }
    let mut doors = Vec::new();
    for service in &reading.services {
        for (domain, port, to) in &service.serves {
            if to == name || service.name == *name {
                doors.push(format!("{domain}:{port}"));
            }
        }
    }
    doors.sort();
    doors.dedup();
    if !doors.is_empty() {
        println!("  dark doors: {}", doors.join(", "));
    }
    let fallen_service = reading
        .services
        .iter()
        .find(|service| service.name == *name);
    if let Some(service) = fallen_service {
        let orphaned: Vec<&String> = service
            .volumes
            .iter()
            .filter(|volume| {
                !reading
                    .services
                    .iter()
                    .any(|other| other.name != *name && other.volumes.contains(volume))
            })
            .collect();
        if !orphaned.is_empty() {
            println!(
                "  unreachable state: {}",
                orphaned
                    .iter()
                    .map(|volume| volume.as_str())
                    .collect::<Vec<&str>>()
                    .join(", ")
            );
        }
    }
    let mut placed: Vec<String> = Vec::new();
    if let Some(brief) = brief::read(found) {
        for case in &brief.cases {
            let Some(unit) = &case.unit else { continue };
            let mut cone: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
            for service in reading
                .services
                .iter()
                .filter(|one| one.unit.as_deref() == Some(unit.as_str()))
            {
                cone.insert(service.name.clone());
                cone.extend(arbiter::records::plans::stands_on(&reading, &service.name));
            }
            if cone.contains(name) {
                placed.push(case.name.clone());
            }
        }
    }
    let mut cases: Vec<String> = Vec::new();
    for case in &placed {
        cases.push(format!("{case} — placed over it"));
    }
    cases.sort();
    cases.dedup();
    if cases.is_empty() {
        println!("  cases: none placed over it");
    } else {
        for case in cases {
            println!("  case: {case}");
        }
    }
    ExitCode::SUCCESS
}

fn fallout(name: &str, defs: &[Def], edges: &[Edge]) {
    let matched: Vec<(usize, &Def)> = defs
        .iter()
        .enumerate()
        .filter(|(_, def)| def.name == name)
        .collect();
    if matched.is_empty() {
        println!("fallout: no public symbol named {name}");
        return;
    }
    for (index, def) in matched {
        println!("{}:{} {}", def.file.to_string_lossy(), def.line, def.name);
        let mut users: Vec<String> = edges
            .iter()
            .filter(|edge| edge.def == index && edge.file != def.file)
            .map(|edge| format!("  used by {}:{}", edge.file.to_string_lossy(), edge.line))
            .collect();
        users.sort();
        users.dedup();
        if def.self_used {
            println!("  used by its own file");
        } else if users.is_empty() {
            println!("  used by nothing");
        }
        for user in users {
            println!("{user}");
        }
    }
}

// The matrix of rule against language, printed group by group. A cell answers
// held or never; an open cell is work the matrix keeps visible, never a pass.
// The tag column answers origin and power at a glance: a core row is judged
// on one graph, a record row waits for the user's record.
fn matrix() {
    let packs = arbiter::packs::all();
    let width = pack::RULES
        .iter()
        .chain(pack::CORE)
        .map(|(rule, _)| rule.len())
        .max()
        .unwrap_or(0);
    print!("  {:width$}  {:14}", "rule", "power");
    for pack in &packs {
        print!("  {:10}", pack.label());
    }
    println!();
    for (group, members) in pack::GROUPS {
        println!("{group}");
        for member in *members {
            if !pack::RULES.iter().any(|(rule, _)| rule == member) {
                println!("  {member:width$}  {}", tagged(member));
                continue;
            }
            print!("  {member:width$}  {:14}", tagged(member));
            for pack in &packs {
                let answer = pack
                    .rules()
                    .iter()
                    .find(|(name, _)| name == member)
                    .map(|(_, answer)| *answer);
                let cell = match answer {
                    Some("held") => "held",
                    Some(_) => "never",
                    None => "open",
                };
                print!("  {cell:10}");
            }
            println!();
        }
    }
    println!();
    println!("core is judged on one graph: every unit and import, all languages together.");
    println!(
        "facts rules are the bar from day one; a record rule waits for the user's \
         record, and the drill names its file."
    );
}

// The glance column: origin and power in one word each.
fn tagged(rule: &str) -> String {
    let core = pack::CORE.iter().any(|(name, _)| *name == rule);
    let record = matches!(pack::power(rule), pack::Power::Record(_));
    match (core, record) {
        (true, true) => "core, record".to_string(),
        (true, false) => "core, facts".to_string(),
        (false, true) => "record".to_string(),
        (false, false) => "facts".to_string(),
    }
}

// One rule whole: its law, then each pack's stance - held, open, or never
// with its why. A core judgment has no stances; its law is the answer. A
// group prints every law on its shelf.
fn ruled(asked: &str) -> ExitCode {
    if let Some((group, members)) = pack::GROUPS.iter().find(|(group, _)| *group == asked) {
        println!("{group}:");
        for member in *members {
            if let Some(law) = pack::law(member) {
                println!("  {member}: {law}");
            }
        }
        return ExitCode::SUCCESS;
    }
    if let Some((rule, law)) = pack::RULES.iter().find(|(rule, _)| *rule == asked) {
        println!("{rule}");
        labeled("law", law);
        labeled("power", &empowered(asked));
        labeled("judged", "by each language's pack, stance by stance");
        println!();
        for pack in arbiter::packs::all() {
            let answer = pack
                .rules()
                .iter()
                .find(|(name, _)| name == rule)
                .map(|(_, answer)| *answer);
            match answer {
                Some("held") => println!("  {:10} held", pack.label()),
                Some(told) => println!(
                    "  {:10} never — {}",
                    pack.label(),
                    told.strip_prefix("never: ").unwrap_or(told)
                ),
                None => println!("  {:10} open", pack.label()),
            }
        }
        return ExitCode::SUCCESS;
    }
    if let Some((name, law)) = pack::CORE.iter().find(|(name, _)| *name == asked) {
        println!("{name}");
        labeled("law", law);
        labeled("power", &empowered(asked));
        labeled(
            "judged",
            "on the one graph — every unit and import, all languages at once",
        );
        return ExitCode::SUCCESS;
    }
    eprintln!("rules: no rule or group named {asked}; arbiter rules prints the matrix");
    ExitCode::from(2)
}

// Where the law detains its power: the facts alone, or a record the user
// wrote - a record-powered law waits until its record exists.
fn empowered(rule: &str) -> String {
    match pack::power(rule) {
        pack::Power::Facts => "the facts".to_string(),
        pack::Power::Record(file) => {
            format!("the user's record ({file}); without the record, the law waits")
        }
    }
}

// One labeled line of the drill, wrapped at a readable width with a hanging
// indent so the label column stays a column.
fn labeled(label: &str, text: &str) {
    let width = 68;
    let mut line = String::new();
    let mut first = true;
    for word in text.split_whitespace() {
        if !line.is_empty() && line.len() + 1 + word.len() > width {
            if first {
                println!("  {label:7}{line}");
                first = false;
            } else {
                println!("  {:7}{line}", "");
            }
            line.clear();
        }
        if !line.is_empty() {
            line.push(' ');
        }
        line.push_str(word);
    }
    if first {
        println!("  {label:7}{line}");
    } else if !line.is_empty() {
        println!("  {:7}{line}", "");
    }
}

// The agent layer, installed where agents will look. The judge's own files
// (rules, skill, hook) follow the binary and are refreshed when they drift,
// never backwards: a file stamped by a newer arbiter is left, and the caller
// is told to update. settings.json is the repository's and is written once.
fn adopted(root: &Path) -> (Vec<String>, bool) {
    let mut told = Vec::new();
    let mut stale = false;
    let owned: [(&str, &str); 6] = [
        (
            ".claude/agent-rules.md",
            include_str!("../share/agent-rules.md"),
        ),
        (
            ".claude/skills/arbiter-quality/SKILL.md",
            include_str!("../share/skills/arbiter-quality/SKILL.md"),
        ),
        (
            ".claude/skills/arbiter-method/SKILL.md",
            include_str!("../share/skills/arbiter-method/SKILL.md"),
        ),
        (
            ".claude/skills/arbiter-genesis/SKILL.md",
            include_str!("../share/skills/arbiter-genesis/SKILL.md"),
        ),
        (
            ".claude/skills/arbiter-arrival/SKILL.md",
            include_str!("../share/skills/arbiter-arrival/SKILL.md"),
        ),
        (
            ".claude/hooks/arbiter-stop.sh",
            include_str!("../share/hooks/arbiter-stop.sh"),
        ),
    ];
    for (place, content) in owned {
        let path = root.join(place);
        let expected = commands::stamped(place, content);
        match fs::read_to_string(&path) {
            Ok(present) if present == expected => {
                told.push(format!("kept {place}"));
                continue;
            }
            Ok(present) if commands::ahead(&present) => {
                let (major, minor, patch) = commands::written_by(&present).unwrap_or((0, 0, 0));
                told.push(format!(
                    "left {place}: written by arbiter {major}.{minor}.{patch}, and this \
                     binary is {} — update the binary, not the file",
                    commands::VERSION
                ));
                stale = true;
                continue;
            }
            Ok(_) => {
                if fs::write(&path, &expected).is_ok() {
                    told.push(format!("refreshed {place}"));
                }
                continue;
            }
            Err(_) => {}
        }
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        if fs::write(&path, &expected).is_ok() {
            told.push(format!("adopted {place}"));
        }
    }
    let settings = root.join(".claude/settings.json");
    if settings.exists() {
        told.push("kept .claude/settings.json".to_string());
    } else {
        if let Some(parent) = settings.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let content = "{\n  \"hooks\": {\n    \"Stop\": [\n      {\n        \"hooks\": [\n          {\n            \"type\": \"command\",\n            \"command\": \"bash \\\"$CLAUDE_PROJECT_DIR/.claude/hooks/arbiter-stop.sh\\\"\"\n          }\n        ]\n      }\n    ]\n  }\n}\n";
        if fs::write(&settings, content).is_ok() {
            told.push("adopted .claude/settings.json".to_string());
        }
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let hook = root.join(".claude/hooks/arbiter-stop.sh");
        let _ = fs::set_permissions(&hook, fs::Permissions::from_mode(0o755));
    }
    let claude = root.join("CLAUDE.md");
    let pointer = "@.claude/agent-rules.md";
    match fs::read_to_string(&claude) {
        Ok(text) if text.contains(pointer) => told.push("kept CLAUDE.md".to_string()),
        Ok(text) => {
            let _ = fs::write(&claude, format!("{}\n{pointer}\n", text.trim_end()));
            told.push("pointed CLAUDE.md at the rules".to_string());
        }
        Err(_) => {
            let _ = fs::write(&claude, format!("{pointer}\n"));
            told.push("wrote CLAUDE.md pointing at the rules".to_string());
        }
    }
    (told, stale)
}
