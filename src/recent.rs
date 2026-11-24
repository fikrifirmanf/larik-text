use gpui::{App, Global};
use std::path::{Path, PathBuf};

pub const LIMIT: usize = 10;

/// Most recently opened files, newest first. Saved so the list survives restarts.
pub struct Recent {
    files: Vec<PathBuf>,
    store: Option<PathBuf>,
    /// Bumps on every change so the menu bar knows to rebuild.
    pub revision: u64,
}

impl Global for Recent {}

impl Recent {
    pub fn load(store: Option<PathBuf>) -> Self {
        let files = store
            .as_deref()
            .and_then(|path| std::fs::read(path).ok())
            .and_then(|bytes| serde_json::from_slice::<Vec<PathBuf>>(&bytes).ok())
            .unwrap_or_default();
        Self {
            files: files.into_iter().take(LIMIT).collect(),
            store,
            revision: 0,
        }
    }

    pub fn files(&self) -> &[PathBuf] {
        &self.files
    }

    fn push(&mut self, path: &Path) {
        self.files.retain(|existing| existing != path);
        self.files.insert(0, path.to_path_buf());
        self.files.truncate(LIMIT);
    }

    fn save(&self) {
        let Some(store) = &self.store else { return };
        let write = || -> anyhow::Result<()> {
            use std::io::Write as _;
            let dir = store.parent().ok_or_else(|| anyhow::anyhow!("No folder"))?;
            std::fs::create_dir_all(dir)?;
            let mut file = tempfile::NamedTempFile::new_in(dir)?;
            file.write_all(&serde_json::to_vec(&self.files)?)?;
            file.persist(store)?;
            Ok(())
        };
        if let Err(error) = write() {
            eprintln!("Couldn’t save recent files: {error:#}");
        }
    }
}

pub fn add(path: &Path, cx: &mut App) {
    if !cx.has_global::<Recent>() {
        return;
    }
    let recent = cx.global_mut::<Recent>();
    recent.push(path);
    recent.revision += 1;
    recent.save();
    crate::menus_changed(cx);
}

pub fn clear(cx: &mut App) {
    if !cx.has_global::<Recent>() {
        return;
    }
    let recent = cx.global_mut::<Recent>();
    recent.files.clear();
    recent.revision += 1;
    recent.save();
    crate::menus_changed(cx);
}

/// A menu label: the file name, then its folder with the home directory shortened.
pub fn label(path: &Path) -> String {
    let name = path.file_name().map_or_else(
        || path.display().to_string(),
        |n| n.to_string_lossy().into_owned(),
    );
    let folder = path.parent().map(|dir| {
        match std::env::home_dir()
            .and_then(|home| dir.strip_prefix(home).ok().map(Path::to_path_buf))
        {
            Some(rest) if rest.as_os_str().is_empty() => "~".to_owned(),
            Some(rest) => format!("~/{}", rest.display()),
            None => dir.display().to_string(),
        }
    });
    match folder {
        Some(folder) => format!("{name}  —  {folder}"),
        None => name,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn newest_first_without_duplicates_and_capped() {
        let mut recent = Recent::load(None);
        for i in 0..12 {
            recent.push(Path::new(&format!("/tmp/f{i}.txt")));
        }
        recent.push(Path::new("/tmp/f5.txt"));
        assert_eq!(recent.files().len(), LIMIT);
        assert_eq!(recent.files()[0], Path::new("/tmp/f5.txt"));
        assert_eq!(
            recent
                .files()
                .iter()
                .filter(|p| **p == Path::new("/tmp/f5.txt"))
                .count(),
            1
        );
    }

    #[test]
    fn labels_show_name_then_folder() {
        assert_eq!(label(Path::new("/var/log/a.txt")), "a.txt  —  /var/log");
    }
}
