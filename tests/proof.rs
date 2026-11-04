use arbiter::core::graph::graph;
use arbiter::judge::proof::proofs;

const PROOF: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/proof");

#[test]
fn each_language_claims_its_sentences_under_their_frames() {
    let found = graph(PROOF).expect("the fixture folder exists");
    let (claimed, unproven) = proofs(&found);
    assert!(unproven.is_empty());
    let shown: Vec<String> = claimed
        .values()
        .flatten()
        .map(|proof| {
            format!(
                "{} | {} | {}",
                proof.file.to_string_lossy(),
                proof.path.join(" / "),
                proof.sentence
            )
        })
        .collect();
    assert_eq!(
        shown,
        [
            "dock.test.ts | docking | snaps the arrow to the nearest rim",
            "dock.test.ts | docking / a crowded rim | spreads the arrows a pitch apart",
            "dock.test.ts |  | a bare claim stands outside any suite",
            "lib.rs |  | doubling two gives four",
            "test_lattice.py | TestLattice | holds a point per cell",
            "test_lattice.py |  | an empty lattice spans nothing",
        ]
    );
}
