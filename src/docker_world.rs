use std::cell::OnceCell;
use std::collections::HashMap;
use std::fs;
use typst::World;
use std::path::PathBuf;
use fontdb::{Database};
use typst::font::{Font, FontBook, FontInfo};
use comemo::Prehashed;
use chrono::{DateTime, Datelike, Local, Timelike};
use typst::diag::{EcoString, FileResult, StrResult};
use typst::eval::{Bytes, Datetime, Library, Tracer};
use typst::syntax::{FileId, Source, VirtualPath};

pub struct FontDb {
    fonts: Vec<LazyFont>
}

struct LazyFont {
    index: u32,
    path: PathBuf,
    data: OnceCell<Option<Font>>,
}

impl LazyFont {
    fn get(&self) -> Option<Font> {
        self.data.get_or_init(|| {
            let data = fs::read(&self.path).ok()?.into();
            Font::new(data, self.index)
        }).clone()
    }
}

impl FontDb {
    fn get(&self, index: usize) -> Option<Font> {
        self.fonts[index].get()
    }

    pub fn new(fontdir: Option<PathBuf>, book: &mut FontBook) -> Self {
        let mut database = Database::new();
        let mut fonts= vec![];

        if let Some(fontdir) = fontdir {
            database.load_fonts_dir(fontdir);
        }
        database.load_system_fonts();

        for face in database.faces() {
            let path = match &face.source {
                fontdb::Source::File(path) | fontdb::Source::SharedFile(path, _) => path,
                fontdb::Source::Binary(_) => continue
            };

            let info = database.with_face_data(face.id, FontInfo::new).expect("Font load snafu");

            if let Some(info) = info {
                book.push(info);
                fonts.push(
                    LazyFont {
                        path: path.clone(),
                        index: face.index,
                        data: OnceCell::new()
                    }
                )
            }
        }

        Self {
            fonts
        }
    }


}
pub struct DockerWorld {
    fonts: FontDb,
    book: Prehashed<FontBook>,
    library: Prehashed<Library>,
    main: FileId,
    now: OnceCell<DateTime<Local>>,
    sources: HashMap<FileId, Bytes>
}

fn file_id(filename: &str) -> FileId {
    FileId::new(None, VirtualPath::new(PathBuf::from(filename)))
}

pub struct DocumentFile {
    pub name: FileId,
    pub data: Bytes
}

impl DocumentFile {
    pub fn new(name: &str, data: Vec<u8>) -> Self {
        Self {
            name: file_id(name),
            data: data.into()
        }
    }
}

impl DockerWorld {
    pub fn new(main_document: DocumentFile, other_files: Vec<DocumentFile>,fontdir: Option<PathBuf>) -> Self {
        let mut book = FontBook::new();
        let fonts = FontDb::new(fontdir, &mut book);
        let main = main_document.name;
        let mut sources: HashMap<FileId, Bytes> = HashMap::new();
        sources.insert(main, main_document.data);
        for file in other_files {
            sources.insert(file.name, file.data);
        }
        Self {
            main,
            fonts,
            book: Prehashed::new(book),
            library: Prehashed::new(typst_library::build()),
            sources,
            now: OnceCell::new()
        }
    }

    pub fn compile(&mut self) -> StrResult<Vec<u8>> {
        let mut tracer = Tracer::default();
        let result = typst::compile(self, &mut tracer);

        match result {
            Err(_) => { Err(EcoString::from("Something terrible has happened")) }
            Ok(document) => {
                Ok(
                    typst::export::pdf(&document, None, self.now())
                )
            }
        }

    }

    /// Get the current date and time in UTC.
    fn now(&self) -> Option<Datetime> {
        let now = self.now.get_or_init(Local::now);
        Datetime::from_ymd_hms(
            now.year(),
            now.month().try_into().ok()?,
            now.day().try_into().ok()?,
            now.hour().try_into().ok()?,
            now.minute().try_into().ok()?,
            now.second().try_into().ok()?,
        )
    }
}

impl World for DockerWorld {
    fn library(&self) -> &Prehashed<Library> {
        &self.library
    }

    fn book(&self) -> &Prehashed<FontBook> {
        &self.book
    }

    fn main(&self) -> Source {
        self.source(self.main).unwrap()
    }

    fn source(&self, id: FileId) -> FileResult<Source> {
        let raw_data = self.sources.get(&id).expect("No Such Source file");
        Ok(Source::new(id, decode_utf8(&raw_data).parse().unwrap()))
    }

    fn file(&self, id: FileId) -> FileResult<Bytes> {
        let data = self.sources.get(&id).expect("No Such Source file");
        Ok(data.clone())
    }

    fn font(&self, index: usize) -> Option<Font> { self.fonts.get(index) }

    fn today(&self, offset: Option<i64>) -> Option<Datetime> {
        let now = self.now.get_or_init(Local::now);

        let naive = match offset {
            None => { now.naive_local() }
            Some(o) => { now.naive_utc() + chrono::Duration::hours(o) }
        };

        Datetime::from_ymd(
            naive.year(),
            naive.month().try_into().ok()?,
            naive.day().try_into().ok()?
        )
    }
}

/// Decode UTF-8 with an optional BOM.
fn decode_utf8(buf: &[u8]) -> &str {
    // Remove UTF-8 BOM.
    std::str::from_utf8(
        buf.strip_prefix(b"\xef\xbb\xbf").unwrap_or(buf),
    ).expect("What the hell")
}