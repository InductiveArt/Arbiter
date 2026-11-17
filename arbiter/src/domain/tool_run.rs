use std::path::Path;

use crate::domain::ecosystem::Config;
use crate::domain::ecosystem::Ecosystem;
use crate::domain::subprocess::Subprocess;

// How a tool is started from the words its record writes: the program, the
// arguments handed to it, and the configs of its ecosystem when a word points
// at them.
pub struct ToolRun {
    pub program: String,
    pub arguments: Vec<String>,
    pub configs: Vec<Config>,
}

impl ToolRun {
    // None when the words name no program.
    pub fn of(words: &[String], ecosystem: &Ecosystem) -> Option<ToolRun> {
        let (program, arguments) = words.split_first()?;
        let pointed = words.iter().any(|word| word.contains("{pack}"));
        Some(ToolRun {
            program: program.clone(),
            arguments: arguments.to_vec(),
            configs: if pointed {
                ecosystem.configs.clone()
            } else {
                Vec::new()
            },
        })
    }

    // The tool started in the folder under the name, over the cone when its
    // run stands for a fact about the cone's files.
    pub fn subprocess(&self, name: String, folder: &Path, cone: Option<&str>) -> Subprocess {
        Subprocess {
            name,
            program: self.program.clone(),
            arguments: self.arguments.clone(),
            folder: folder.to_path_buf(),
            configs: self.configs.clone(),
            cone: cone.map(str::to_string),
        }
    }
}
