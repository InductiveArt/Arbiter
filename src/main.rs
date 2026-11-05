use std::env;
use std::fs;
use std::path::Path;
use std::process::ExitCode;

use arbiter::core::graph::graph;
use arbiter::core::pack;
use arbiter::core::pack::Def;
use arbiter::core::pack::Edge;
use arbiter::core::pack::Finding;
use arbiter::core::pack::Suppression;
use arbiter::judge::brief;
use arbiter::judge::claims;
use arbiter::judge::findings::integrity;
use arbiter::judge::findings::winnow;
use arbiter::judge::ledger;
use arbiter::judge::outcomes;
use arbiter::judge::rhyme;
use arbiter::judge::verdict;
use arbiter::verbs;

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
        println!("{command}: {} units, {} files", run.units, run.files);
        for (check, (passed, failed, warm, seconds)) in verdict::tallied(&run.outcomes) {
            let heat = if warm > 0 {
                format!(" ({warm} warm)")
            } else {
                String::new()
            };
            if failed == 0 {
                println!("  {check:10} ok     {passed} units{heat}   {seconds:.1}s");
            } else {
                println!(
                    "  {check:10} failed {failed} of {}{heat}   {seconds:.1}s",
                    passed + failed
                );
            }
        }
        let mut counts: std::collections::BTreeMap<&str, usize> = Default::default();
        for finding in &run.findings {
            *counts.entry(finding.check).or_insert(0) += 1;
        }
        for (check, count) in &counts {
            println!("  {check:10} {count} findings");
        }
        match run.added {
            None => println!(
                "  ledger     {} suppressions, no reference recorded",
                run.counted
            ),
            Some(added) => println!(
                "  ledger     {} suppressions, {added} added since accept",
                run.counted
            ),
        }
        match run.third_added {
            None => println!(
                "  deps       {} third parties, no reference recorded",
                run.third
            ),
            Some(added) => println!(
                "  deps       {} third parties, {added} added since accept",
                run.third
            ),
        }
        match run.claims_removed {
            None => println!("  claims     {} spoken, no reference recorded", run.claims),
            Some(0) => println!("  claims     {} spoken, none removed", run.claims),
            Some(removed) => println!(
                "  claims     {} spoken, {removed} removed since accept",
                run.claims
            ),
        }
        match run.brief {
            verdict::BriefLine::Skipped => println!("  brief      skipped"),
            verdict::BriefLine::Unwritten => println!("  brief      no brief written"),
            verdict::BriefLine::Read {
                cases,
                held,
                planned,
                manual,
                unanswered,
                unwired,
            } => println!(
                "  brief      {cases} cases — {held} held, {planned} planned, \
                 {manual} manual, {unanswered} unanswered; {unwired} claims unwired"
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
    if arguments.first().map(String::as_str) == Some("blast") {
        let Some(name) = arguments.get(1) else {
            eprintln!("blast: name the symbol to look up");
            return ExitCode::from(2);
        };
        let asked = arguments.get(2).cloned().unwrap_or_else(|| ".".to_string());
        let Some(found) = graph(&asked) else {
            eprintln!("blast: no folder at {asked}");
            return ExitCode::from(2);
        };
        blast(name, &found.defs, &found.edges);
        return ExitCode::SUCCESS;
    }
    if arguments.first().map(String::as_str) == Some("deps") {
        let Some(name) = arguments.get(1) else {
            eprintln!("deps: name the unit to describe");
            return ExitCode::from(2);
        };
        let asked = arguments.get(2).cloned().unwrap_or_else(|| ".".to_string());
        let Some(found) = graph(&asked) else {
            eprintln!("deps: no folder at {asked}");
            return ExitCode::from(2);
        };
        return deps(name, &found);
    }
    if arguments.first().map(String::as_str) == Some("findings") {
        let asked = arguments.get(1).cloned().unwrap_or_else(|| ".".to_string());
        let Some(found) = graph(&asked) else {
            eprintln!("findings: no folder at {asked}");
            return ExitCode::from(2);
        };
        let mut all = winnow(&found.defs, &found.edges, &found.repo);
        all.extend(integrity(&found));
        all.extend(ledger::blanket_findings(&found));
        all.extend(claims::vanished(&found));
        all.extend(brief::findings(&found));
        all.sort_by(|a, b| (&a.file, a.line).cmp(&(&b.file, b.line)));
        emit(&all, &arguments[2.min(arguments.len())..]);
        return ExitCode::SUCCESS;
    }
    if arguments.first().map(String::as_str) == Some("ledger") {
        let asked = arguments.get(1).cloned().unwrap_or_else(|| ".".to_string());
        let Some(found) = graph(&asked) else {
            eprintln!("ledger: no folder at {asked}");
            return ExitCode::from(2);
        };
        let counted = ledger::counted(&found);
        let plural = if counted.len() == 1 { "" } else { "s" };
        println!("ledger: {} suppression{plural}", counted.len());
        for suppression in &counted {
            println!("  {}", shown(suppression));
        }
        match ledger::reference(&found.root) {
            None => println!("ledger: no reference recorded; arbiter accept records today's"),
            Some(reference) => {
                let extra = ledger::added(&counted, &reference);
                if extra.is_empty() {
                    println!("ledger: {} recorded, none added", reference.len());
                } else {
                    println!(
                        "ledger: {} recorded, {} added:",
                        reference.len(),
                        extra.len()
                    );
                    for suppression in &extra {
                        println!("  {}", shown(suppression));
                    }
                }
            }
        }
        let blankets = ledger::blankets(&found);
        if blankets.is_empty() {
            return ExitCode::SUCCESS;
        }
        let plural = if blankets.len() == 1 { "" } else { "s" };
        println!(
            "ledger: {} blanket suppression{plural}, quieting more than one named rule:",
            blankets.len()
        );
        for suppression in &blankets {
            println!("  {}", shown(suppression));
        }
        return ExitCode::FAILURE;
    }
    if arguments.first().map(String::as_str) == Some("brief") {
        let (case, asked) = match (arguments.get(1), arguments.get(2)) {
            (Some(first), second) if !Path::new(first).is_dir() => (
                Some(first.clone()),
                second.cloned().unwrap_or_else(|| ".".to_string()),
            ),
            (first, _) => (None, first.cloned().unwrap_or_else(|| ".".to_string())),
        };
        let Some(found) = graph(&asked) else {
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
            "brief: {} cases — {} held, {} planned, {} manual, {} unanswered; \
             {} claims, {} unwired",
            reading.cases,
            reading
                .needs
                .iter()
                .filter(|need| need.state == brief::State::Held)
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
                brief::State::Held => "held",
                brief::State::Planned => "planned",
                brief::State::Manual => "manual",
                brief::State::Unanswered => "unanswered",
            };
            println!("  {state:10} {}", need.need);
        }
        if !reading.unwired.is_empty() {
            println!();
            println!("unwired claims:");
            for (file, line, sentence) in &reading.unwired {
                println!("  {}:{line} {sentence}", file.to_string_lossy());
            }
        }
        return ExitCode::SUCCESS;
    }
    if arguments.first().map(String::as_str) == Some("rhyme") {
        let asked = arguments.get(1).cloned().unwrap_or_else(|| ".".to_string());
        let Some(found) = graph(&asked) else {
            eprintln!("rhyme: no folder at {asked}");
            return ExitCode::from(2);
        };
        let groups = rhyme::rhymes(&found);
        if groups.is_empty() {
            println!("rhyme: nothing repeats at function grain");
            return ExitCode::SUCCESS;
        }
        let plural = if groups.len() == 1 { "" } else { "s" };
        println!(
            "rhyme: {} shape{plural} sounding in more than one place",
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
        return ExitCode::SUCCESS;
    }
    if arguments.first().map(String::as_str) == Some("warm") {
        let asked = arguments.get(1).cloned().unwrap_or_else(|| ".".to_string());
        let Some(found) = graph(&asked) else {
            eprintln!("warm: no folder at {asked}");
            return ExitCode::from(2);
        };
        let defs_units: Vec<usize> = found.defs.iter().map(|def| def.unit).collect();
        let warmth = arbiter::core::warm::warmth(
            &found.root,
            &found.repo,
            &found.packs,
            &found.edges,
            &defs_units,
        );
        let mut counted = (0, 0);
        for (index, unit) in found.repo.units.iter().enumerate() {
            let Some(Some(key)) = warmth.keys.get(index) else {
                continue;
            };
            let state = if warmth.stored(index, &unit.path).is_some() {
                counted.0 += 1;
                "warm"
            } else {
                counted.1 += 1;
                "cold"
            };
            println!("  {state}  {:12.12}  {}", key, unit.path.display());
        }
        println!(
            "warm: {} warm, {} cold; the store is .arbiter/warm, cleared by deleting it",
            counted.0, counted.1
        );
        return ExitCode::SUCCESS;
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
    if arguments.first().map(String::as_str) == Some("accept") {
        let asked = arguments.get(1).cloned().unwrap_or_else(|| ".".to_string());
        let Some(found) = graph(&asked) else {
            eprintln!("accept: no folder at {asked}");
            return ExitCode::from(2);
        };
        let blankets = ledger::blankets(&found);
        if !blankets.is_empty() {
            eprintln!(
                "accept: {} blanket suppressions are findings, never records",
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
        match ledger::record(&found.root, &counted, &packages, &spoken) {
            Ok(path) => {
                println!(
                    "accept: {} suppressions, {} third parties and {} claims recorded in {}",
                    counted.len(),
                    packages.len(),
                    spoken.len(),
                    path.to_string_lossy()
                );
                for pack in &found.packs {
                    for line in pack.record(&found.root, &found.repo) {
                        println!("accept: {line}");
                    }
                }
                return ExitCode::SUCCESS;
            }
            Err(error) => {
                eprintln!("accept: the reference could not be written: {error}");
                return ExitCode::from(2);
            }
        }
    }
    if let Some(verb) = arguments.first() {
        eprintln!("arbiter: no verb named {verb}");
    }
    taught();
    ExitCode::from(2)
}

// The bare call teaches: the whole grammar, each verb with its one line.
fn taught() {
    println!(
        "arbiter {} judges a repository: the resolved graph, each unit's pinned tools, the brief.",
        verbs::VERSION
    );
    println!();
    let width = verbs::VERBS
        .iter()
        .map(|(call, _)| call.len())
        .max()
        .unwrap_or(0);
    for (call, line) in verbs::VERBS {
        println!("  {call:width$}  {line}");
    }
    println!();
    println!(
        "A verb takes the folder last; missing, it judges here. \
         Exit: 0 clean, 1 findings, 2 broken machine."
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
fn slice(case: &str, reading: &arbiter::judge::brief::Reading) -> ExitCode {
    let needs: Vec<&arbiter::judge::brief::Need> = reading
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
            brief::State::Held => "held",
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

fn emit(findings: &[Finding], files: &[String]) {
    for finding in findings {
        let shown = finding.file.to_string_lossy();
        if !files.is_empty() && !files.iter().any(|file| *file == shown) {
            continue;
        }
        println!(
            "{}",
            serde_json::json!({
                "check": finding.check,
                "file": shown,
                "line": finding.line,
                "symbol": finding.name,
                "reason": finding.reason,
            })
        );
    }
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
        eprintln!("deps: no unit named {name}");
        return ExitCode::from(2);
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

fn blast(name: &str, defs: &[Def], edges: &[Edge]) {
    let matched: Vec<(usize, &Def)> = defs
        .iter()
        .enumerate()
        .filter(|(_, def)| def.name == name)
        .collect();
    if matched.is_empty() {
        println!("blast: no public symbol named {name}");
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

// The matrix of rule against language. A cell answers held or never; an open
// cell is work the matrix keeps visible, never a pass.
fn matrix() {
    let packs = arbiter::packs::all();
    let width = pack::RULES
        .iter()
        .map(|(rule, _)| rule.len())
        .max()
        .unwrap_or(0);
    print!("{:width$}", "rule");
    for pack in &packs {
        print!("  {:10}", pack.label());
    }
    println!();
    for (rule, _) in pack::RULES {
        print!("{rule:width$}");
        for pack in &packs {
            let answer = pack
                .rules()
                .iter()
                .find(|(name, _)| name == rule)
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
    println!();
    println!("each rule's law:");
    for (rule, law) in pack::RULES {
        println!("  {rule}: {law}");
    }
    println!();
    println!("core judges every language through the one graph:");
    for (name, law) in pack::CORE {
        println!("  {name}: {law}");
    }
    println!();
    println!(
        "arbiter rules <rule> prints one rule's stance in every language, with each never's why."
    );
}

// One rule whole: its law, then each pack's stance - held, open, or never
// with its why. A core judgment has no stances; its law is the answer.
fn ruled(asked: &str) -> ExitCode {
    if let Some((rule, law)) = pack::RULES.iter().find(|(rule, _)| *rule == asked) {
        println!("{rule}: {law}");
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
        println!("{name}: {law}");
        println!();
        println!("  core: judged through the one graph, every language at once");
        return ExitCode::SUCCESS;
    }
    eprintln!("rules: no rule named {asked}; arbiter rules prints the matrix");
    ExitCode::from(2)
}

// The agent layer, installed where agents will look. The judge's own files
// (rules, skill, hook) follow the binary and are refreshed when they drift,
// never backwards: a file stamped by a newer arbiter is left, and the caller
// is told to update. settings.json is the repository's and is written once.
fn adopted(root: &Path) -> (Vec<String>, bool) {
    let mut told = Vec::new();
    let mut stale = false;
    let owned: [(&str, &str); 3] = [
        (
            ".claude/agent-rules.md",
            include_str!("../share/agent-rules.md"),
        ),
        (
            ".claude/skills/arbiter-quality/SKILL.md",
            include_str!("../share/skills/arbiter-quality/SKILL.md"),
        ),
        (
            ".claude/hooks/arbiter-stop.sh",
            include_str!("../share/hooks/arbiter-stop.sh"),
        ),
    ];
    for (place, content) in owned {
        let path = root.join(place);
        let expected = verbs::stamped(place, content);
        match fs::read_to_string(&path) {
            Ok(present) if present == expected => {
                told.push(format!("kept {place}"));
                continue;
            }
            Ok(present) if verbs::ahead(&present) => {
                let (major, minor, patch) = verbs::written_by(&present).unwrap_or((0, 0, 0));
                told.push(format!(
                    "left {place}: written by arbiter {major}.{minor}.{patch}, and this \
                     binary is {} — update the binary, not the file",
                    verbs::VERSION
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
