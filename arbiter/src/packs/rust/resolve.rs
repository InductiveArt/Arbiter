use std::collections::HashMap;
use std::collections::HashSet;
use std::path::Path;
use std::path::PathBuf;

use crate::core::definition::Def;
use crate::core::edge::Edge;
use crate::core::repository::Repo;
use crate::packs::rust::manifest::declared_crates;
use crate::packs::rust::module_files::module_files;
use crate::packs::rust::reading::Reading;

// Each use read to the definitions it reaches: in a module of its own crate,
// in a crate of the repository its manifest takes by path, or in the contract
// a module of generated code answers for.
pub fn edges(
    readings: &[Reading],
    mine: usize,
    defs: &[Def],
    root: &Path,
    repo: &Repo,
) -> Vec<Edge> {
    let crates = repository_crates(root, repo);
    let unreached = HashMap::new();
    let mut defined: HashMap<(&Path, &str), Vec<usize>> = HashMap::new();
    for (index, def) in defs.iter().enumerate() {
        if def.pack == mine {
            defined
                .entry((def.file.as_path(), def.name.as_str()))
                .or_default()
                .push(index);
        }
    }
    let contracts = contracts(readings);
    let contracted = contracted(defs);
    let mut edges = Vec::new();
    for reading in readings {
        let unit = repo.unit_of(&reading.file);
        let reached = crates.get(&unit).unwrap_or(&unreached);
        for used in &reading.uses {
            for candidate in module_files(
                &reading.file,
                &used.module,
                &repo.units[unit].path,
                &reading.imports,
                reached,
            ) {
                let own = defined
                    .get(&(candidate.as_path(), used.name.as_str()))
                    .into_iter()
                    .flatten()
                    .copied();
                let contract = contracts
                    .get(candidate.as_path())
                    .into_iter()
                    .flat_map(|package| {
                        contracted
                            .get(used.name.as_str())
                            .into_iter()
                            .flatten()
                            .copied()
                            .filter(move |def| {
                                defs[*def]
                                    .file
                                    .parent()
                                    .is_some_and(|folder| folder.ends_with(package))
                            })
                    });
                for def in own.chain(contract) {
                    edges.push(Edge {
                        def,
                        file: reading.file.clone(),
                        line: used.line,
                        unit,
                    });
                }
            }
        }
    }
    edges
}

// The crates each unit reaches inside this repository by name: its own, and
// each its manifest takes by path. A crate taken from a registry is a
// release, compiled from no source here.
fn repository_crates(root: &Path, repo: &Repo) -> HashMap<usize, HashMap<String, PathBuf>> {
    let declared = declared_crates(root, repo);
    let homes: HashMap<&str, &PathBuf> = declared
        .iter()
        .filter_map(|(unit, entry)| Some((entry.own.as_deref()?, &repo.units[*unit].path)))
        .collect();
    declared
        .iter()
        .map(|(unit, entry)| {
            let named =
                entry.own.iter().chain(&entry.paths).filter_map(|name| {
                    Some((name.clone(), homes.get(name.as_str())?.to_path_buf()))
                });
            (*unit, named.collect())
        })
        .collect()
}

// A module taking in what a generator wrote answers for the contract it was
// generated from, as a generated module does in every language: the
// generated file is named for the contract's package, and the package's
// definitions sit in the folder its name spells.
fn contracts(readings: &[Reading]) -> HashMap<&Path, PathBuf> {
    let generated: HashSet<&Path> = readings
        .iter()
        .filter(|reading| reading.generated)
        .map(|reading| reading.file.as_path())
        .collect();
    readings
        .iter()
        .filter(|reading| !reading.generated)
        .flat_map(|reading| {
            reading
                .includes
                .iter()
                .filter(|taken| generated.contains(taken.as_path()))
                .filter_map(|taken| {
                    let named = taken.file_name()?.to_string_lossy();
                    let package = named.strip_suffix(".rs")?;
                    Some((reading.file.as_path(), package.split('.').collect()))
                })
        })
        .collect()
}

// Each name a contract defines, with its definitions.
fn contracted(defs: &[Def]) -> HashMap<&str, Vec<usize>> {
    let mut contracted: HashMap<&str, Vec<usize>> = HashMap::new();
    for (index, def) in defs.iter().enumerate() {
        if def
            .file
            .extension()
            .is_some_and(|extension| extension == "proto")
        {
            contracted.entry(def.name.as_str()).or_default().push(index);
        }
    }
    contracted
}
