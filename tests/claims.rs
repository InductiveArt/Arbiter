use arbiter::judge::claims::heard;
use arbiter::packs::graphed;

const CLAIMS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/claims");

#[test]
fn each_language_claims_its_sentences_under_their_frames() {
    let found = graphed(CLAIMS).expect("the fixture folder exists");
    let (claimed, silent) = heard(&found);
    assert!(silent.is_empty());
    let shown: Vec<String> = claimed
        .values()
        .flatten()
        .map(|one| {
            format!(
                "{} | {} | {}",
                one.file.to_string_lossy(),
                one.claim.path.join(" / "),
                one.claim.sentence
            )
        })
        .collect();
    assert_eq!(
        shown,
        [
            "dock.test.ts | docking | snaps the arrow to the nearest rim",
            "dock.test.ts | docking / a crowded rim | spreads the arrows a pitch apart",
            "dock.test.ts |  | a bare claim stands outside any suite",
            "dock.test.ts |  | doubles %s into place",
            "dock.test.ts |  | a helper still names the door",
            "lib.rs |  | doubling two gives four",
            "test_lattice.py | TestLattice | holds a point per cell",
            "test_lattice.py |  | an empty lattice spans nothing",
        ]
    );
}

#[test]
fn a_claim_that_vanished_without_signature_is_a_finding() {
    let found = graphed(CLAIMS).expect("the fixture folder exists");
    let gone = arbiter::judge::claims::vanished(&found);
    assert_eq!(gone.len(), 1);
    assert!(gone[0].reason.contains("vanished;"));
    assert!(gone[0]
        .reason
        .contains("leaves only with the user's signature"));
}
