/// Buffer Manager — tracks scsynth buffer allocations for sample playback.
///
/// Maps sample file paths (relative to project root) to scsynth buffer IDs.
/// Buffer IDs start at 100 (0 is reserved for FFT in scsynth).

use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Manages buffer ID allocation and path-to-ID mapping.
#[derive(Debug)]
pub struct BufferManager {
    /// Map from sample path (relative to project root) to allocated buffer ID.
    buffers: HashMap<String, i32>,
    /// Next buffer ID to allocate. Starts at 100 (0 reserved for FFT).
    next_id: i32,
    /// Project root directory for resolving relative paths.
    project_root: PathBuf,
}

impl BufferManager {
    /// Create a new BufferManager with the given project root.
    pub fn new(project_root: PathBuf) -> Self {
        Self {
            buffers: HashMap::new(),
            next_id: 100,
            project_root,
        }
    }

    /// Allocate a buffer ID for a sample path. Returns the ID.
    /// If the path already has an allocated buffer, returns the existing ID.
    pub fn alloc(&mut self, sample_path: &str) -> i32 {
        if let Some(&id) = self.buffers.get(sample_path) {
            return id;
        }
        let id = self.next_id;
        self.next_id += 1;
        self.buffers.insert(sample_path.to_string(), id);
        id
    }

    /// Get the buffer ID for a sample path, if allocated.
    pub fn get(&self, sample_path: &str) -> Option<i32> {
        self.buffers.get(sample_path).copied()
    }

    /// Remove a buffer allocation, returning the freed ID.
    pub fn free(&mut self, sample_path: &str) -> Option<i32> {
        self.buffers.remove(sample_path)
    }

    /// Resolve a relative sample path to an absolute path using the project root.
    pub fn resolve_path(&self, sample_path: &str) -> PathBuf {
        let p = Path::new(sample_path);
        if p.is_absolute() {
            p.to_path_buf()
        } else {
            self.project_root.join(p)
        }
    }

    /// Check if a file change event path matches any loaded sample.
    /// Returns the sample key (relative path) if found.
    pub fn sample_for_path(&self, changed_path: &Path) -> Option<String> {
        for sample_path in self.buffers.keys() {
            let abs = self.resolve_path(sample_path);
            if changed_path == abs || changed_path.ends_with(sample_path) {
                return Some(sample_path.clone());
            }
        }
        None
    }

    /// Get all allocated sample paths.
    pub fn all_samples(&self) -> Vec<&str> {
        self.buffers.keys().map(|s| s.as_str()).collect()
    }

    /// Number of allocated buffers.
    pub fn len(&self) -> usize {
        self.buffers.len()
    }

    /// Check if no buffers are allocated.
    pub fn is_empty(&self) -> bool {
        self.buffers.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alloc_returns_sequential_ids() {
        let mut mgr = BufferManager::new(PathBuf::from("/project"));
        let id1 = mgr.alloc("samples/kick.wav");
        let id2 = mgr.alloc("samples/snare.wav");
        assert_eq!(id1, 100);
        assert_eq!(id2, 101);
    }

    #[test]
    fn alloc_same_path_returns_same_id() {
        let mut mgr = BufferManager::new(PathBuf::from("/project"));
        let id1 = mgr.alloc("samples/kick.wav");
        let id2 = mgr.alloc("samples/kick.wav");
        assert_eq!(id1, id2);
        assert_eq!(mgr.len(), 1);
    }

    #[test]
    fn get_returns_allocated_id() {
        let mut mgr = BufferManager::new(PathBuf::from("/project"));
        mgr.alloc("samples/kick.wav");
        assert_eq!(mgr.get("samples/kick.wav"), Some(100));
        assert_eq!(mgr.get("samples/nonexistent.wav"), None);
    }

    #[test]
    fn free_removes_buffer() {
        let mut mgr = BufferManager::new(PathBuf::from("/project"));
        mgr.alloc("samples/kick.wav");
        let freed = mgr.free("samples/kick.wav");
        assert_eq!(freed, Some(100));
        assert_eq!(mgr.get("samples/kick.wav"), None);
        assert_eq!(mgr.len(), 0);
    }

    #[test]
    fn resolve_relative_path() {
        let mgr = BufferManager::new(PathBuf::from("/home/user/project"));
        let abs = mgr.resolve_path("samples/kick.wav");
        assert_eq!(abs, PathBuf::from("/home/user/project/samples/kick.wav"));
    }

    #[test]
    fn resolve_absolute_path_unchanged() {
        let mgr = BufferManager::new(PathBuf::from("/home/user/project"));
        let abs = mgr.resolve_path("/other/path/sample.wav");
        assert_eq!(abs, PathBuf::from("/other/path/sample.wav"));
    }

    #[test]
    fn sample_for_path_matches_relative() {
        let mut mgr = BufferManager::new(PathBuf::from("/project"));
        mgr.alloc("samples/kick.wav");
        let result = mgr.sample_for_path(Path::new("/project/samples/kick.wav"));
        assert_eq!(result, Some("samples/kick.wav".to_string()));
    }

    #[test]
    fn sample_for_path_no_match() {
        let mut mgr = BufferManager::new(PathBuf::from("/project"));
        mgr.alloc("samples/kick.wav");
        let result = mgr.sample_for_path(Path::new("/other/file.wav"));
        assert_eq!(result, None);
    }

    #[test]
    fn empty_manager() {
        let mgr = BufferManager::new(PathBuf::from("/project"));
        assert!(mgr.is_empty());
        assert_eq!(mgr.len(), 0);
    }

    #[test]
    fn all_samples_returns_keys() {
        let mut mgr = BufferManager::new(PathBuf::from("/project"));
        mgr.alloc("samples/kick.wav");
        mgr.alloc("samples/snare.wav");
        let mut samples = mgr.all_samples();
        samples.sort();
        assert_eq!(samples, vec!["samples/kick.wav", "samples/snare.wav"]);
    }
}
