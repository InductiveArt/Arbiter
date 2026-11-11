use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

use crate::core::graph::Graph;

pub struct Place {
    pub file: PathBuf,
    pub line: usize,
    pub ending: usize,
    pub title: String,
}

// One shape sounding in several places: perfect when every place reads the
// same, slant when the structure matches under different words.
pub struct Group {
    pub leaves: usize,
    pub perfect: bool,
    pub places: Vec<Place>,
}

pub fn rhymes(found: &Graph) -> Vec<Group> {
    let mut by_bones: BTreeMap<String, Vec<(Place, String, usize)>> = BTreeMap::new();
    for file in &found.repo.files {
        let name = file
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_default();
        let Some(index) = found.packs.iter().position(|pack| pack.handles(&name)) else {
            continue;
        };
        if found.packs[index].generated(&name) {
            continue;
        }
        // A unit on a test path is a fixture: data for some test, not code to judge.
        let unit = found.repo.unit_of(file);
        let home = found.repo.units[unit].path.clone();
        if found.repo.is_test(unit, &home) {
            continue;
        }
        let Ok(source) = fs::read_to_string(found.root.join(file)) else {
            continue;
        };
        for shape in found.packs[index].shapes(&source) {
            by_bones.entry(shape.bones).or_default().push((
                Place {
                    file: file.clone(),
                    line: shape.line,
                    ending: shape.ending,
                    title: shape.title,
                },
                shape.flesh,
                shape.leaves,
            ));
        }
    }
    let mut groups: Vec<Group> = by_bones
        .into_values()
        .filter(|members| members.len() > 1)
        .map(|mut members| {
            members.sort_by(|a, b| (&a.0.file, a.0.line).cmp(&(&b.0.file, b.0.line)));
            let perfect = members.iter().all(|(_, flesh, _)| *flesh == members[0].1);
            let leaves = members[0].2;
            Group {
                leaves,
                perfect,
                places: members.into_iter().map(|(place, _, _)| place).collect(),
            }
        })
        .collect();
    groups.sort_by_key(|group| std::cmp::Reverse((group.leaves, group.places.len())));
    // A shape already inside a reported place everywhere it sounds says nothing new.
    let mut kept: Vec<Group> = Vec::new();
    for group in groups {
        let free = group.places.iter().any(|place| {
            !kept.iter().any(|higher| {
                higher.places.iter().any(|outer| {
                    outer.file == place.file
                        && outer.line <= place.line
                        && place.ending <= outer.ending
                })
            })
        });
        if free {
            kept.push(group);
        }
    }
    kept
}
