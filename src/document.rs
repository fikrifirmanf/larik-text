use anyhow::{Context as _, Result, bail};
use encoding_rs::WINDOWS_1252;
use similar::{ChangeTag, TextDiff};
use std::{
    borrow::Cow,
    fs,
    io::Write as _,
    path::{Path, PathBuf},
    sync::Arc,
};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum Encoding {
    #[default]
    Utf8,
    Utf8Bom,
    Utf16Le,
    Utf16Be,
    Windows1252,
}

impl Encoding {
    pub fn label(self) -> &'static str {
        match self {
            Self::Utf8 => "UTF-8",
            Self::Utf8Bom => "UTF-8 BOM",
            Self::Utf16Le => "UTF-16 LE",
            Self::Utf16Be => "UTF-16 BE",
            Self::Windows1252 => "Windows-1252",
        }
    }

    fn bom(self) -> &'static [u8] {
        match self {
            Self::Utf8Bom => b"\xef\xbb\xbf",
            Self::Utf16Le => b"\xff\xfe",
            Self::Utf16Be => b"\xfe\xff",
            _ => b"",
        }
    }

    fn decode(self, bytes: &[u8]) -> Result<String> {
        let content = bytes.strip_prefix(self.bom()).unwrap_or(bytes);
        match self {
            Self::Utf8 | Self::Utf8Bom => Ok(str::from_utf8(content)?.into()),
            Self::Windows1252 => Ok(WINDOWS_1252.decode_without_bom_handling(content).0.into()),
            Self::Utf16Le | Self::Utf16Be => {
                let (units, remainder) = content.as_chunks::<2>();
                if !remainder.is_empty() {
                    bail!("The file ends with an incomplete UTF-16 character.");
                }
                let units: Vec<_> = units
                    .iter()
                    .map(|bytes| match self {
                        Self::Utf16Le => u16::from_le_bytes(*bytes),
                        _ => u16::from_be_bytes(*bytes),
                    })
                    .collect();
                Ok(String::from_utf16(&units)?)
            }
        }
    }

    fn encode(self, text: &str) -> Result<Vec<u8>> {
        let mut bytes = self.bom().to_vec();
        match self {
            Self::Utf8 | Self::Utf8Bom => bytes.extend_from_slice(text.as_bytes()),
            Self::Windows1252 => {
                let (encoded, _, had_errors) = WINDOWS_1252.encode(text);
                if had_errors {
                    bail!("Some characters cannot be saved as Windows-1252. Choose Save as UTF-8.");
                }
                bytes.extend_from_slice(&encoded);
            }
            Self::Utf16Le | Self::Utf16Be => {
                for unit in text.encode_utf16() {
                    bytes.extend(match self {
                        Self::Utf16Le => unit.to_le_bytes(),
                        _ => unit.to_be_bytes(),
                    });
                }
            }
        }
        Ok(bytes)
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
enum Ending {
    #[default]
    Lf,
    CrLf,
    Cr,
}

impl Ending {
    fn text(self) -> &'static str {
        match self {
            Self::Lf => "\n",
            Self::CrLf => "\r\n",
            Self::Cr => "\r",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Lf => "LF",
            Self::CrLf => "CRLF",
            Self::Cr => "CR",
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct LineEndings {
    preferred: Ending,
    mixed: Option<Vec<Ending>>,
}

impl LineEndings {
    fn normalize(text: &str) -> (String, Self) {
        let mut normalized = String::with_capacity(text.len());
        let mut rest = text;
        let mut actual = Vec::new();
        let mut counts = [0; 3];
        while let Some(i) = rest.find(['\r', '\n']) {
            normalized.push_str(&rest[..i]);
            normalized.push('\n');
            rest = &rest[i..];
            let ending = if rest.starts_with("\r\n") {
                Ending::CrLf
            } else if rest.starts_with('\r') {
                Ending::Cr
            } else {
                Ending::Lf
            };
            rest = &rest[ending.text().len()..];
            counts[ending as usize] += 1;
            actual.push(ending);
        }
        normalized.push_str(rest);
        let preferred = actual.first().copied().map_or(Ending::Lf, |first| {
            [Ending::Lf, Ending::CrLf, Ending::Cr]
                .into_iter()
                .max_by_key(|ending| (counts[*ending as usize], *ending == first))
                .unwrap_or(first)
        });
        let mixed = (counts.iter().filter(|count| **count > 0).count() > 1).then_some(actual);
        (normalized, Self { preferred, mixed })
    }

    pub fn label(&self) -> &'static str {
        if self.mixed.is_some() {
            "Mixed"
        } else {
            self.preferred.label()
        }
    }

    fn restore<'a>(&self, original: &str, text: &'a str) -> Cow<'a, str> {
        let Some(endings) = &self.mixed else {
            return if self.preferred == Ending::Lf {
                Cow::Borrowed(text)
            } else {
                Cow::Owned(text.replace('\n', self.preferred.text()))
            };
        };

        let original = Self::normalize(original).0;
        let diff = TextDiff::configure()
            .timeout(std::time::Duration::from_millis(150))
            .diff_lines(original.as_str(), text);
        let mut restored = String::with_capacity(text.len());
        for change in diff.iter_all_changes() {
            if change.tag() == ChangeTag::Delete {
                continue;
            }
            let line = change.value();
            if let Some(content) = line.strip_suffix('\n') {
                restored.push_str(content);
                let ending = change.old_index().and_then(|index| endings.get(index));
                restored.push_str(ending.copied().unwrap_or(self.preferred).text());
            } else {
                restored.push_str(line);
            }
        }
        Cow::Owned(restored)
    }
}

#[derive(Clone, Debug, Default)]
pub struct FileFormat {
    pub encoding: Encoding,
    pub endings: LineEndings,
}

#[derive(Debug)]
pub struct LoadedFile {
    pub path: PathBuf,
    pub bytes: Arc<[u8]>,
    pub text: String,
    pub format: FileFormat,
}

#[derive(Debug)]
pub enum OpenError {
    Read(anyhow::Error),
    Encoding(PathBuf),
    Binary,
}

pub fn open(path: PathBuf, encoding: Option<Encoding>) -> Result<LoadedFile, OpenError> {
    let read = || -> Result<_> {
        let path = fs::canonicalize(path)?;
        if !fs::metadata(&path)?.is_file() {
            bail!("Choose a regular file.");
        }
        let bytes = fs::read(&path)?;
        Ok((path, bytes))
    };
    let (path, bytes) = read().map_err(OpenError::Read)?;
    decode_file(path, bytes.into(), encoding)
}

pub fn decode_file(
    path: PathBuf,
    bytes: Arc<[u8]>,
    encoding: Option<Encoding>,
) -> Result<LoadedFile, OpenError> {
    let encoding = encoding.unwrap_or_else(|| {
        [Encoding::Utf8Bom, Encoding::Utf16Le, Encoding::Utf16Be]
            .into_iter()
            .find(|encoding| bytes.starts_with(encoding.bom()))
            .unwrap_or_default()
    });
    if matches!(
        encoding,
        Encoding::Utf8 | Encoding::Utf8Bom | Encoding::Windows1252
    ) && bytes.contains(&0)
    {
        return Err(OpenError::Binary);
    }
    let decoded = encoding
        .decode(&bytes)
        .map_err(|_| OpenError::Encoding(path.clone()))?;
    if decoded
        .chars()
        .any(|c| c.is_control() && !matches!(c, '\n' | '\r' | '\t' | '\x0c' | '\x0b'))
    {
        return Err(OpenError::Binary);
    }
    let (text, endings) = LineEndings::normalize(&decoded);
    Ok(LoadedFile {
        path,
        bytes,
        text,
        format: FileFormat { encoding, endings },
    })
}

#[derive(Debug, thiserror::Error)]
pub enum SaveError {
    #[error("The file changed on disk.")]
    Changed,
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

pub fn save(
    path: &Path,
    expected: Option<&[u8]>,
    original: &str,
    text: &str,
    format: &FileFormat,
) -> Result<LoadedFile, SaveError> {
    let destination = if let Some(expected) = expected {
        Some(Cow::Borrowed(expected))
    } else {
        match fs::read(path) {
            Ok(bytes) => Some(Cow::Owned(bytes)),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
            Err(error) => {
                return Err(anyhow::Error::new(error)
                    .context("Could not check the destination file.")
                    .into());
            }
        }
    };
    let normalized = LineEndings::normalize(text).0;
    let restored = format.endings.restore(original, &normalized);
    let bytes = format.encoding.encode(&restored)?;
    let format = FileFormat {
        encoding: format.encoding,
        endings: LineEndings::normalize(&restored).1,
    };
    let folder = path.parent().context("The file has no parent folder.")?;
    let mut temporary =
        tempfile::NamedTempFile::new_in(folder).context("Could not write to the folder.")?;
    temporary
        .write_all(&bytes)
        .context("Could not write the file.")?;

    if let Ok(metadata) = fs::metadata(path) {
        temporary
            .as_file()
            .set_permissions(metadata.permissions())
            .context("Could not preserve file permissions.")?;
    }
    temporary
        .as_file()
        .sync_all()
        .context("Could not flush the file.")?;
    if let Some(expected) = destination.as_deref() {
        match fs::read(path) {
            Ok(actual) if actual == expected => {}
            Ok(_) => return Err(SaveError::Changed),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Err(SaveError::Changed);
            }
            Err(error) => {
                return Err(anyhow::Error::new(error)
                    .context("Could not check the original file.")
                    .into());
            }
        }
        temporary
            .persist(path)
            .context("Could not replace the file.")?;
    } else {
        match temporary.persist_noclobber(path) {
            Ok(_) => {}
            Err(error) if error.error.kind() == std::io::ErrorKind::AlreadyExists => {
                return Err(SaveError::Changed);
            }
            Err(error) => {
                return Err(anyhow::Error::new(error)
                    .context("Could not create the file.")
                    .into());
            }
        }
    }
    let path = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    Ok(LoadedFile {
        path,
        bytes: bytes.into(),
        text: normalized,
        format,
    })
}

pub fn file_name(path: &Path) -> String {
    path.file_name()
        .unwrap_or(path.as_os_str())
        .to_string_lossy()
        .into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saving_preserves_utf16_bom_and_crlf() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("note.txt");
        fs::write(&path, b"\xff\xfeh\x00i\x00\r\x00\n\x00").unwrap();
        let opened = open(path, None).unwrap();
        assert_eq!(opened.text, "hi\n");
        assert_eq!(opened.format.encoding, Encoding::Utf16Le);
        save(
            &opened.path,
            Some(&opened.bytes),
            &opened.text,
            "bye\n🙂\n",
            &opened.format,
        )
        .unwrap();
        assert_eq!(
            fs::read(&opened.path).unwrap(),
            b"\xff\xfeb\x00y\x00e\x00\r\x00\n\x00\x3d\xd8\x42\xde\r\x00\n\x00",
        );
    }

    #[test]
    fn mixed_endings_follow_unchanged_lines_after_an_insertion() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("notes.txt");
        fs::write(&path, b"first\r\nsecond\nthird\r\n").unwrap();
        let opened = open(path, None).unwrap();
        assert_eq!(opened.format.endings.label(), "Mixed");
        save(
            &opened.path,
            Some(&opened.bytes),
            &opened.text,
            "first\ninserted\nsecond\nthird\n",
            &opened.format,
        )
        .unwrap();
        assert_eq!(
            fs::read(&opened.path).unwrap(),
            b"first\r\ninserted\r\nsecond\nthird\r\n"
        );
    }

    #[test]
    fn saving_keeps_an_external_edit_untouched() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("config");
        fs::write(&path, b"original\n").unwrap();
        let opened = open(path, None).unwrap();
        fs::write(&opened.path, b"external edit\n").unwrap();
        let result = save(
            &opened.path,
            Some(&opened.bytes),
            &opened.text,
            "my edit\n",
            &opened.format,
        );
        assert!(matches!(result, Err(SaveError::Changed)));
        assert_eq!(fs::read(&opened.path).unwrap(), b"external edit\n");
    }

    #[test]
    fn binary_and_undecodable_files_are_distinguished() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("unknown.extension");
        fs::write(&path, b"test\0data").unwrap();
        assert!(matches!(open(path.clone(), None), Err(OpenError::Binary)));
        fs::write(&path, b"caf\xe9\n").unwrap();
        assert!(matches!(
            open(path.clone(), None),
            Err(OpenError::Encoding(_))
        ));
        let opened = open(path, Some(Encoding::Windows1252)).unwrap();
        assert_eq!(opened.text, "café\n");
    }

    #[test]
    fn unsupported_characters_cannot_overwrite_a_legacy_text_file() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("note.txt");
        fs::write(&path, b"caf\xe9").unwrap();
        let opened = open(path, Some(Encoding::Windows1252)).unwrap();
        let result = save(
            &opened.path,
            Some(&opened.bytes),
            &opened.text,
            "café 🙂",
            &opened.format,
        );
        assert!(matches!(result, Err(SaveError::Other(_))));
        assert_eq!(fs::read(&opened.path).unwrap(), b"caf\xe9");
    }
}
