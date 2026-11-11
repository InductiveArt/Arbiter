use arbiter::packs::graphed;
use arbiter::records::brief::read;
use arbiter::records::brief::State;

const CLAIMS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/claims");

#[test]
fn the_brief_states_each_need_and_roars_at_the_unwired() {
    let found = graphed(CLAIMS).expect("the fixture folder exists");
    let reading = read(&found).expect("the fixture writes a brief");
    assert_eq!(reading.cases.len(), 4);
    assert_eq!(reading.claims, 8);
    let states: Vec<String> = reading
        .needs
        .iter()
        .map(|need| {
            let state = match need.state {
                State::Tested => "tested",
                State::Planned => "planned",
                State::Manual => "manual",
                State::Unanswered => "unanswered",
            };
            format!("{} | {} | {state}", need.case, need.need)
        })
        .collect();
    assert_eq!(
        states,
        [
            "docking | arrows dock and crowd politely | tested",
            "docking | a full rim refuses the next arrow | planned",
            "lattice | the lattice holds its points | tested",
            "lattice | the lattice survives a snapshot restore | manual",
            "lattice | a binding gone stale | unanswered",
            "lattice | a need with nothing at all | unanswered",
            "spacing | a hand-typed binding folds its spaces | tested",
        ]
    );
    let told: Vec<String> = reading
        .findings
        .iter()
        .map(|finding| format!("{} {}", finding.name, finding.reason))
        .collect();
    assert_eq!(
        told,
        [
            "stale-binding the case 'lattice' binds '. | lattice | a sentence nobody speaks', \
             which the tree does not speak",
            "unanswered-need the need 'a need with nothing at all' of case 'lattice' binds \
             no claims and is neither planned nor manual",
            "empty-case the case 'hollow' declares no needs; a case with no needs proves nothing",
            "unwired-claim the claim '. |  | a bare claim stands outside any suite' answers no need \
             in the brief",
            "unwired-claim the claim '. |  | doubles %s into place' answers no need in the brief",
            "unwired-claim the claim '. |  | a helper still names the door' answers no need in the brief",
            "unwired-claim the claim '. |  | an empty lattice spans nothing' answers no need in the brief",
        ]
    );
}

#[test]
fn a_binding_typed_with_one_space_wires_a_claim_printed_with_two() {
    let found = graphed(CLAIMS).expect("the fixture folder exists");
    let reading = read(&found).expect("the fixture writes a brief");
    let need = reading
        .needs
        .iter()
        .find(|need| need.case == "spacing")
        .expect("the fixture declares the spacing case");
    assert!(need.state == State::Tested);
}

#[test]
fn a_brief_finding_points_at_its_own_line() {
    let found = graphed(CLAIMS).expect("the fixture folder exists");
    let reading = read(&found).expect("the fixture writes a brief");
    let at = |name: &str| {
        reading
            .findings
            .iter()
            .find(|finding| finding.name == name)
            .map(|finding| finding.line)
    };
    assert_eq!(at("stale-binding"), Some(26));
    assert_eq!(at("unanswered-need"), Some(29));
    assert_eq!(at("empty-case"), Some(32));
}

#[test]
fn a_case_trial_declares_the_moves_and_the_probe() {
    use std::fs;
    let root = std::env::temp_dir().join(format!("arbiter-trial-read-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(root.join(".arbiter")).expect("the folder is writable");
    fs::write(root.join("app.py"), "value = 1\n").expect("the folder is writable");
    fs::write(
        root.join(".arbiter/brief.toml"),
        concat!(
            "[[case]]\nname = \"marks\"\nstory = \"kept\"\n\n",
            "[case.trial]\nstore = \"sqlite\"\nvolume = \"data\"\npath = \"marks.db\"\n",
            "create = { method = \"PUT\", route = \"/m/a\", body = \"{}\" }\n",
            "update = { method = \"PUT\", route = \"/m/a\", body = \"{}\" }\n",
            "delete = { method = \"DELETE\", route = \"/m/a\" }\n",
            "read = { method = \"GET\", route = \"/m/b\", answer = \"{}\" }\n",
            "probe = \"SELECT 1\"\ncreated = \"1\"\nupdated = \"2\"\nseed = \"INSERT\"\n",
            "proves = [\"kept across restarts\"]\n\n",
            "[[case.need]]\nneed = \"kept across restarts\"\nplanned = true\n",
        ),
    )
    .expect("the folder is writable");

    let found = graphed(&root.to_string_lossy()).expect("the folder exists");
    let reading = read(&found).expect("the brief reads");
    assert_eq!(reading.trials.len(), 1, "the case carries one trial");
    let trial = &reading.trials[0];
    assert_eq!(trial.case, "marks");
    assert_eq!(trial.store, "sqlite");
    assert_eq!(trial.volume, "data");
    assert_eq!(trial.create.method, "PUT");
    assert_eq!(trial.delete.body, "", "a bodyless move stays empty");
    assert_eq!(trial.answer, "{}", "the read move names its exact answer");
    assert_eq!(trial.probe, "SELECT 1");
    assert_eq!(
        trial.proves,
        ["kept across restarts"],
        "the mandate is read"
    );
    assert_eq!(
        trial.sequence, "persistence",
        "an unnamed sequence is the one in the law-text"
    );
    assert!(
        !reading
            .findings
            .iter()
            .any(|told| told.name == "unmandated-trial" || told.name == "stale-mandate"),
        "a mandated trial raises nothing"
    );

    let _ = fs::remove_dir_all(&root);
}

fn briefed(tag: &str, brief: &str) -> (std::path::PathBuf, Vec<String>) {
    use std::fs;
    let root = std::env::temp_dir().join(format!("arbiter-{tag}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(root.join(".arbiter")).expect("the folder is writable");
    fs::write(root.join("app.py"), "value = 1\n").expect("the folder is writable");
    fs::write(root.join(".arbiter/brief.toml"), brief).expect("the folder is writable");
    let found = graphed(&root.to_string_lossy()).expect("the folder exists");
    let reading = read(&found).expect("the brief reads");
    let told = reading
        .findings
        .iter()
        .map(|finding| format!("{} | {}", finding.name, finding.reason))
        .collect();
    (root, told)
}

#[test]
fn an_orphan_case_is_named() {
    let (root, told) = briefed(
        "orphan",
        concat!(
            "[[case]]\nname = \"roots\"\n[[case.need]]\nneed = \"the ground holds\"\nmanual = \"seen\"\n\n",
            "[[case]]\nname = \"leaves\"\nserves = \"the sky holds\"\n",
            "[[case.need]]\nneed = \"leaves turn\"\nmanual = \"seen\"\n",
        ),
    );
    assert!(
        told.iter().any(|line| line.starts_with("orphan-case |")
            && line.contains("serves 'the sky holds', which no other case states")),
        "told: {told:?}"
    );
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn a_misplaced_case_is_named() {
    let (root, told) = briefed(
        "misplaced",
        concat!(
            "[[case]]\nname = \"roots\"\nunit = \"ghost/unit\"\n",
            "[[case.need]]\nneed = \"the ground holds\"\nmanual = \"seen\"\n",
        ),
    );
    assert!(
        told.iter().any(|line| line.starts_with("misplaced-case |")
            && line.contains("placed in unit ghost/unit, which this repository does not build")),
        "told: {told:?}"
    );
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn a_trial_proving_an_unstated_need_is_named() {
    let (root, told) = briefed(
        "mandate",
        concat!(
            "[[case]]\nname = \"marks\"\n\n",
            "[case.trial]\nstore = \"sqlite\"\nvolume = \"data\"\npath = \"m.db\"\n",
            "create = { method = \"PUT\", route = \"/m\" }\nupdate = { method = \"PUT\", route = \"/m\" }\n",
            "delete = { method = \"DELETE\", route = \"/m\" }\nread = { method = \"GET\", route = \"/m\" }\n",
            "probe = \"SELECT 1\"\ncreated = \"1\"\nupdated = \"2\"\nseed = \"INSERT\"\n",
            "proves = [\"a ghost sentence\"]\n\n",
            "[[case.need]]\nneed = \"kept\"\nmanual = \"seen\"\n",
        ),
    );
    assert!(
        told.iter().any(|line| line.starts_with("stale-mandate |")
            && line.contains("proves 'a ghost sentence', which the case does not state")),
        "told: {told:?}"
    );
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn a_trial_proving_nothing_is_named() {
    let (root, told) = briefed(
        "unmandated",
        concat!(
            "[[case]]\nname = \"marks\"\n\n",
            "[case.trial]\nstore = \"sqlite\"\nvolume = \"data\"\npath = \"m.db\"\n",
            "create = { method = \"PUT\", route = \"/m\" }\nupdate = { method = \"PUT\", route = \"/m\" }\n",
            "delete = { method = \"DELETE\", route = \"/m\" }\nread = { method = \"GET\", route = \"/m\" }\n",
            "probe = \"SELECT 1\"\ncreated = \"1\"\nupdated = \"2\"\nseed = \"INSERT\"\n\n",
            "[[case.need]]\nneed = \"kept\"\nmanual = \"seen\"\n",
        ),
    );
    assert!(
        told.iter()
            .any(|line| line.starts_with("unmandated-trial |") && line.contains("proves nothing")),
        "told: {told:?}"
    );
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn the_signature_records_the_stated_needs() {
    use std::fs;
    let root = std::env::temp_dir().join(format!("arbiter-demand-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("the folder is writable");
    let needs = vec!["marks | kept".to_string()];
    arbiter::ledger::record(&root, &[], &[], &[], &needs).expect("the reference writes");
    assert_eq!(
        arbiter::ledger::recorded_needs(&root).expect("the reference reads"),
        ["marks | kept"],
        "a stated need stands under the signature"
    );
    let _ = fs::remove_dir_all(&root);
}
