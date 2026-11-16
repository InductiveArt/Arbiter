use crate::core::pack::Pack;
use crate::domain::ecosystem::Config;
use crate::domain::ecosystem::Ecosystem;

// Every ecosystem the languages carry a record for, as the record declares
// it, with the config files its tools are pointed at, in the order of their
// names.
pub fn ecosystems(packs: &[Box<dyn Pack>]) -> Result<Vec<Ecosystem>, String> {
    let mut found = Vec::new();
    for pack in packs {
        let Some(record) = pack.adapter() else {
            continue;
        };
        let mut ecosystem: Ecosystem = toml::from_str(record).map_err(|error| {
            let message: Vec<&str> = error.message().lines().collect();
            format!(
                "the built-in record packs/{}/ecosystem.toml does not read: {}",
                pack.label(),
                message.join(", ")
            )
        })?;
        ecosystem.configs = pack
            .configs()
            .iter()
            .map(|(name, content)| Config {
                name: name.to_string(),
                content: content.to_string(),
            })
            .collect();
        found.push(ecosystem);
    }
    found.sort_by(|one, other| one.name.cmp(&other.name));
    Ok(found)
}
