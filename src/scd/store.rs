use std::collections::HashMap;
use std::path::Path;

/// Maps thing names to their compiled SynthDef bytes.
/// Thing name = filename stem (e.g. "space-crackle" from "space-crackle.scd").
pub struct ScdStore {
    defs: HashMap<String, Vec<u8>>,
}

impl ScdStore {
    /// Create an empty store with no loaded SynthDefs.
    pub fn empty() -> Self {
        Self {
            defs: HashMap::new(),
        }
    }

    /// Read all .scd files from the given directory.
    /// Returns empty store (not Err) if directory does not exist.
    /// Non-.scd files are silently ignored.
    pub fn load_dir(dir: &Path) -> Result<Self, std::io::Error> {
        let mut defs = HashMap::new();
        if !dir.exists() {
            return Ok(Self { defs });
        }
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("scd") {
                if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                    let bytes = std::fs::read(&path)?;
                    defs.insert(stem.to_string(), bytes);
                }
            }
        }
        Ok(Self { defs })
    }

    /// Get SynthDef bytes for a thing name. Returns None if not loaded.
    pub fn get(&self, thing_name: &str) -> Option<&[u8]> {
        self.defs.get(thing_name).map(|v| v.as_slice())
    }

    /// Returns all loaded thing names.
    pub fn thing_names(&self) -> impl Iterator<Item = &str> {
        self.defs.keys().map(|s| s.as_str())
    }

    /// Iterate over all (thing_name, bytes) pairs.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &[u8])> {
        self.defs.iter().map(|(k, v)| (k.as_str(), v.as_slice()))
    }

    /// Number of loaded SynthDefs.
    pub fn len(&self) -> usize {
        self.defs.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    /// Helper: create a temp dir with .scd files for testing
    fn setup_scd_dir(files: &[(&str, &[u8])]) -> tempfile::TempDir {
        let dir = tempfile::tempdir().expect("create temp dir");
        for (name, content) in files {
            fs::write(dir.path().join(name), content).expect("write test file");
        }
        dir
    }

    #[test]
    fn load_dir_with_two_scd_files_returns_two_entries() {
        let dir = setup_scd_dir(&[
            ("space-crackle.scd", b"SCgf-fake-crackle"),
            ("bass-drone.scd", b"SCgf-fake-drone"),
        ]);
        let store = ScdStore::load_dir(dir.path()).unwrap();
        assert_eq!(store.len(), 2);
    }

    #[test]
    fn get_returns_file_content_for_loaded_thing() {
        let dir = setup_scd_dir(&[("space-crackle.scd", b"SCgf-fake-crackle")]);
        let store = ScdStore::load_dir(dir.path()).unwrap();
        assert_eq!(store.get("space-crackle"), Some(b"SCgf-fake-crackle".as_slice()));
    }

    #[test]
    fn get_returns_none_for_missing_thing() {
        let dir = setup_scd_dir(&[("space-crackle.scd", b"SCgf-fake-crackle")]);
        let store = ScdStore::load_dir(dir.path()).unwrap();
        assert_eq!(store.get("missing-thing"), None);
    }

    #[test]
    fn load_dir_on_nonexistent_path_returns_ok_with_zero_entries() {
        let store = ScdStore::load_dir(Path::new("/tmp/nonexistent_scd_dir_xyzzy_12345")).unwrap();
        assert_eq!(store.len(), 0);
    }

    #[test]
    fn load_dir_ignores_non_scd_files() {
        let dir = setup_scd_dir(&[
            ("space-crackle.scd", b"SCgf-fake-crackle"),
            ("notes.txt", b"some notes"),
            ("bass-drone.scsyndef", b"SCgf-binary"),
        ]);
        let store = ScdStore::load_dir(dir.path()).unwrap();
        assert_eq!(store.len(), 1);
        assert!(store.get("space-crackle").is_some());
        assert!(store.get("notes").is_none());
        assert!(store.get("bass-drone").is_none());
    }

    #[test]
    fn thing_names_returns_loaded_names() {
        let dir = setup_scd_dir(&[
            ("space-crackle.scd", b"SCgf-fake-crackle"),
            ("bass-drone.scd", b"SCgf-fake-drone"),
        ]);
        let store = ScdStore::load_dir(dir.path()).unwrap();
        let mut names: Vec<&str> = store.thing_names().collect();
        names.sort();
        assert_eq!(names, vec!["bass-drone", "space-crackle"]);
    }
}
