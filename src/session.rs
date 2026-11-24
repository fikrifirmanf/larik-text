use crate::document::{Encoding, LoadedFile, decode_file, open};
use serde::{Deserialize, Serialize};
use std::{
    io::BufWriter,
    path::{Path, PathBuf},
};

#[derive(Default, Deserialize, Serialize)]
pub struct Session {
    pub active: usize,
    pub tabs: Vec<Tab>,
}

#[derive(Deserialize, Serialize)]
pub struct Tab {
    pub path: Option<PathBuf>,
    pub encoding: Encoding,
    #[serde(default)]
    pub language: Option<String>,
    pub draft: Option<String>,
    pub original: Option<Vec<u8>>,
}

impl Tab {
    pub fn restore(self) -> (Option<LoadedFile>, Option<String>) {
        let file = self.path.and_then(|path| match self.original {
            Some(bytes) => decode_file(path, bytes.into(), Some(self.encoding)).ok(),
            None => open(path, Some(self.encoding)).ok(),
        });
        (file, self.draft)
    }
}

pub fn load(path: &Path) -> anyhow::Result<Session> {
    match std::fs::File::open(path) {
        Ok(file) => Ok(serde_json::from_reader(std::io::BufReader::new(file))?),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Session::default()),
        Err(error) => Err(error.into()),
    }
}

pub fn save(path: &Path, session: Session) -> anyhow::Result<()> {
    let dir = path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("The session has no parent directory."))?;
    std::fs::create_dir_all(dir)?;
    let mut file = tempfile::NamedTempFile::new_in(dir)?;
    {
        use std::io::Write as _;
        let mut writer = BufWriter::new(file.as_file_mut());
        serde_json::to_writer(&mut writer, &session)?;
        writer.flush()?;
    }
    file.as_file().sync_all()?;
    file.persist(path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::{self, SaveError};

    #[test]
    fn checkpoint_preserves_drafts_and_the_original_conflict_baseline() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("notes.md");
        std::fs::write(&path, b"original\r\n").unwrap();
        let checkpoint = directory.path().join("session.json");
        save(
            &checkpoint,
            Session {
                active: 1,
                tabs: vec![
                    Tab {
                        path: Some(path.clone()),
                        encoding: Encoding::Utf8,
                        language: None,
                        draft: Some("edited\n".into()),
                        original: Some(b"original\r\n".to_vec()),
                    },
                    Tab {
                        path: None,
                        encoding: Encoding::Utf8,
                        language: None,
                        draft: Some("unsaved".into()),
                        original: None,
                    },
                ],
            },
        )
        .unwrap();

        std::fs::write(&path, b"updated elsewhere\r\n").unwrap();
        let session = load(&checkpoint).unwrap();
        assert_eq!(session.active, 1);
        let mut tabs = session.tabs.into_iter();
        let (file, draft) = tabs.next().unwrap().restore();
        let file = file.unwrap();
        assert_eq!(file.text, "original\n");
        assert_eq!(file.format.endings.label(), "CRLF");
        let result = document::save(
            &path,
            Some(&file.bytes),
            &file.text,
            &draft.unwrap(),
            &file.format,
        );
        assert!(matches!(result, Err(SaveError::Changed)));
        assert_eq!(tabs.next().unwrap().restore().1.as_deref(), Some("unsaved"));
        assert_eq!(std::fs::read(path).unwrap(), b"updated elsewhere\r\n");
    }
}
