//! Daemon settings, resolved from the `clipboard` section of crownos-config.

use crownos_config::schema::Clipboard;

const MIB: usize = 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    pub history_size: usize,
    pub max_entry_bytes: usize,
    pub persist_primary: bool,
}

impl Config {
    pub fn load() -> Self {
        Self::from(&crownos_config::load::<Clipboard>(Clipboard::SECTION))
    }
}

impl From<&Clipboard> for Config {
    fn from(section: &Clipboard) -> Self {
        Self {
            history_size: section.history_size as usize,
            max_entry_bytes: (section.max_entry_mib as usize).saturating_mul(MIB),
            persist_primary: section.persist_primary,
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self::from(&Clipboard::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_follow_the_config_section() {
        let config = Config::default();
        assert_eq!(config.history_size, 50);
        assert_eq!(config.max_entry_bytes, 16 * MIB);
        assert!(!config.persist_primary);
    }
}
