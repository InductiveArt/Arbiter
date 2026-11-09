use arbiter::judge::brief::read;
use arbiter::judge::brief::State;
use arbiter::packs::graphed;

const CLAIMS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/claims");

#[test]
fn the_brief_states_each_need_and_roars_at_the_unwired() {
    let found = graphed(CLAIMS).expect("the fixture folder exists");
    let reading = read(&found).expect("the fixture writes a brief");
    assert_eq!(reading.cases, 4);
    assert_eq!(reading.claims, 8);
    let states: Vec<String> = reading
        .needs
        .iter()
        .map(|need| {
            let state = match need.state {
                State::Held => "held",
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
            "docking | arrows dock and crowd politely | held",
            "docking | a full rim refuses the next arrow | planned",
            "lattice | the lattice holds its points | held",
            "lattice | the lattice survives a snapshot restore | manual",
            "lattice | a binding gone stale | unanswered",
            "lattice | a need with nothing at all | unanswered",
            "spacing | a hand-typed binding folds its spaces | held",
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
    assert!(need.state == State::Held);
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
            "probe = \"SELECT 1\"\ncreated = \"1\"\nupdated = \"2\"\nseed = \"INSERT\"\n\n",
            "[[case.need]]\nneed = \"kept across restarts\"\nkind = \"persistence\"\nplanned = true\n",
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
        reading.needs[0].kind.as_deref(),
        Some("persistence"),
        "the need claims its category"
    );

    let _ = fs::remove_dir_all(&root);
}
