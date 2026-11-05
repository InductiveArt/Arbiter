use arbiter::core::graph::graph;
use arbiter::judge::brief::{read, State};

const CLAIMS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/claims");

#[test]
fn the_brief_states_each_need_and_roars_at_the_unwired() {
    let found = graph(CLAIMS).expect("the fixture folder exists");
    let reading = read(&found).expect("the fixture writes a brief");
    assert_eq!(reading.cases, 3);
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
            "empty-case the case 'hollow' declares no needs; a parcel with no spec proves nothing",
            "unwired-claim the claim 'a bare claim stands outside any suite' answers no need \
             in the brief",
            "unwired-claim the claim 'doubles %s into place' answers no need in the brief",
            "unwired-claim the claim 'a helper still names the door' answers no need in the brief",
            "unwired-claim the claim 'doubling two gives four' answers no need in the brief",
            "unwired-claim the claim 'an empty lattice spans nothing' answers no need in the brief",
        ]
    );
}
