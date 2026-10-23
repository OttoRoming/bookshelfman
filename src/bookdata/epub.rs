//! EPUB support for [`BookData`](super::BookData).

use std::io::{Read, Seek};
use std::path::Path;

use epub::doc::EpubDoc;

use super::{BookData, CoverImage, Error, Result};

/// Reads metadata from the EPUB at `path`.
pub(crate) fn read(path: &Path) -> Result<BookData> {
    let mut doc = EpubDoc::new(path).map_err(|source| Error::Epub {
        path: path.to_owned(),
        source,
    })?;

    let title = doc
        .get_title()
        .map(|title| title.trim().to_owned())
        .filter(|title| !title.is_empty())
        .or_else(|| fallback_title(path))
        .ok_or_else(|| Error::MissingTitle {
            path: path.to_owned(),
        })?;

    Ok(BookData {
        title,
        page_count: None, // EPUB doesn't store page count
        cover: read_cover(&mut doc),
    })
}

/// Uses the file name as the title when the EPUB has no title metadata.
fn fallback_title(path: &Path) -> Option<String> {
    path.file_stem()
        .map(|stem| stem.to_string_lossy().trim().to_owned())
        .filter(|title| !title.is_empty())
}

/// Reads the cover, falling back to a resource that looks like one.
///
/// Some EPUB files do not mark their cover in the metadata that the `epub`
/// crate checks. In that case, an image whose id or file name contains
/// `cover` is used instead.
fn read_cover<R: Read + Seek>(doc: &mut EpubDoc<R>) -> Option<CoverImage> {
    if let Some((bytes, mime_type)) = doc.get_cover() {
        return Some(CoverImage { bytes, mime_type });
    }

    let mut candidates = doc
        .resources
        .iter()
        .filter(|(_, resource)| resource.mime.starts_with("image/"))
        .filter(|(id, resource)| {
            id.to_ascii_lowercase().contains("cover")
                || resource
                    .path
                    .file_stem()
                    .and_then(|stem| stem.to_str())
                    .is_some_and(|stem| stem.to_ascii_lowercase().contains("cover"))
        })
        .map(|(id, resource)| (id.clone(), resource.path.clone()))
        .collect::<Vec<_>>();

    candidates.sort_by(|(_, left), (_, right)| left.cmp(right));

    let (id, _) = candidates.first()?;
    let (bytes, mime_type) = doc.get_resource(id)?;

    Some(CoverImage { bytes, mime_type })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::{self, File};
    use std::io::Write;
    use std::path::PathBuf;

    use zip::write::SimpleFileOptions;
    use zip::{CompressionMethod, ZipWriter};

    const PAGE_COUNT: usize = 3;
    const COVER: &[u8] = b"\xff\xd8\xff\xe0fake jpeg cover\xff\xd9";

    const CONTAINER_XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container">
  <rootfiles>
    <rootfile full-path="OEBPS/content.opf" media-type="application/oebps-package+xml"/>
  </rootfiles>
</container>"#;

    struct TempDir(PathBuf);

    impl TempDir {
        fn new() -> Self {
            let path = std::env::temp_dir()
                .join(format!("bookshelfman-epub-test-{}", uuid::Uuid::new_v4()));
            fs::create_dir_all(&path).unwrap();

            Self(path)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    /// Writes a small, valid EPUB 2 file.
    fn write_epub(path: &Path, title: Option<&str>, cover: Option<&[u8]>, declare_cover: bool) {
        let file = File::create(path).unwrap();
        let mut zip = ZipWriter::new(file);
        let stored = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
        let deflated = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);

        write_entry(&mut zip, "mimetype", stored, b"application/epub+zip");
        write_entry(
            &mut zip,
            "META-INF/container.xml",
            deflated,
            CONTAINER_XML.as_bytes(),
        );

        let title_element = title
            .map(|title| format!("<dc:title>{title}</dc:title>"))
            .unwrap_or_default();

        let cover_element = if cover.is_some() && declare_cover {
            r#"<meta name="cover" content="cover-image"/>"#
        } else {
            ""
        };

        let cover_item = if cover.is_some() {
            r#"<item id="cover-image" href="images/cover.jpg" media-type="image/jpeg"/>"#
        } else {
            ""
        };

        let chapter_items = (0..PAGE_COUNT)
            .map(|index| {
                format!(
                    r#"<item id="chapter-{index}" href="chapter-{index}.xhtml" media-type="application/xhtml+xml"/>"#
                )
            })
            .collect::<String>();

        let spine_items = (0..PAGE_COUNT)
            .map(|index| format!(r#"<itemref idref="chapter-{index}"/>"#))
            .collect::<String>();

        let package = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<package xmlns="http://www.idpf.org/2007/opf" version="2.0" unique-identifier="book-id">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
    <dc:identifier id="book-id">urn:uuid:0e1a6b9c-3f2d-4a5b-8c7d-9e0f1a2b3c4d</dc:identifier>
    <dc:language>en</dc:language>
    {title_element}
    {cover_element}
  </metadata>
  <manifest>
    {cover_item}
    {chapter_items}
  </manifest>
  <spine>
    {spine_items}
  </spine>
</package>"#
        );

        write_entry(&mut zip, "OEBPS/content.opf", deflated, package.as_bytes());

        if let Some(cover) = cover {
            write_entry(&mut zip, "OEBPS/images/cover.jpg", deflated, cover);
        }

        for index in 0..PAGE_COUNT {
            let chapter = format!(
                r#"<?xml version="1.0" encoding="UTF-8"?>
<html xmlns="http://www.w3.org/1999/xhtml"><body><p>Chapter {index}</p></body></html>"#
            );
            write_entry(
                &mut zip,
                format!("OEBPS/chapter-{index}.xhtml"),
                deflated,
                chapter.as_bytes(),
            );
        }

        zip.finish().unwrap();
    }

    fn write_entry<W: Write + Seek>(
        zip: &mut ZipWriter<W>,
        name: impl ToString,
        options: SimpleFileOptions,
        bytes: &[u8],
    ) {
        zip.start_file(name, options).unwrap();
        zip.write_all(bytes).unwrap();
    }

    #[test]
    fn reads_title_page_count_and_cover() {
        let dir = TempDir::new();
        let path = dir.0.join("The Dispossessed.epub");
        write_epub(&path, Some("The Dispossessed"), Some(COVER), true);

        let book = BookData::open(&path).unwrap();

        assert_eq!(book.title, "The Dispossessed");
        assert_eq!(book.page_count, PAGE_COUNT);
        assert_eq!(
            book.cover,
            Some(CoverImage {
                bytes: COVER.to_vec(),
                mime_type: "image/jpeg".to_owned(),
            })
        );
    }

    #[test]
    fn matches_extension_case_insensitively() {
        let dir = TempDir::new();
        let path = dir.0.join("The Dispossessed.EPUB");
        write_epub(&path, Some("The Dispossessed"), None, false);

        let book = BookData::open(&path).unwrap();

        assert_eq!(book.title, "The Dispossessed");
    }

    #[test]
    fn falls_back_to_the_file_stem_for_a_missing_title() {
        let dir = TempDir::new();
        let path = dir.0.join("Metamorphosis.epub");
        write_epub(&path, None, None, false);

        let book = BookData::open(&path).unwrap();

        assert_eq!(book.title, "Metamorphosis");
    }

    #[test]
    fn reports_a_missing_cover() {
        let dir = TempDir::new();
        let path = dir.0.join("No Cover.epub");
        write_epub(&path, Some("No Cover"), None, false);

        let book = BookData::open(&path).unwrap();

        assert_eq!(book.cover, None);
    }

    #[test]
    fn finds_an_undeclared_cover() {
        let dir = TempDir::new();
        let path = dir.0.join("Undeclared Cover.epub");
        write_epub(&path, Some("Undeclared Cover"), Some(COVER), false);

        let book = BookData::open(&path).unwrap();
        let cover = book.cover.expect("cover should be found");

        assert_eq!(cover.bytes, COVER);
        assert_eq!(cover.mime_type, "image/jpeg");
    }

    #[test]
    fn reports_invalid_epubs() {
        let dir = TempDir::new();
        let path = dir.0.join("Broken.epub");
        fs::write(&path, b"this is not an epub").unwrap();

        let error = BookData::open(&path).unwrap_err();

        assert!(matches!(error, Error::Epub { .. }));
    }
}
