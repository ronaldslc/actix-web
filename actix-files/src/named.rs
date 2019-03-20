use std::fs::{File, Metadata};
use std::io;
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[cfg(unix)]
use std::os::unix::fs::MetadataExt;

use mime;
use mime_guess::guess_mime_type;

use actix_http::error::Error;
use actix_http::http::header::{self, ContentDisposition, DispositionParam, CONTENT_ENCODING};
use actix_web::http::{ContentEncoding, Method, StatusCode};
use actix_web::{HttpMessage, HttpRequest, HttpResponse, Responder};

use crate::config::{DefaultConfig, StaticFileConfig};
use crate::range::HttpRange;
use crate::ChunkedReadFile;

/// A file with an associated name.
#[derive(Debug)]
pub struct NamedFile<C = DefaultConfig> {
    path: PathBuf,
    file: File,
    pub(crate) content_type: mime::Mime,
    pub(crate) content_disposition: header::ContentDisposition,
    pub(crate) md: Metadata,
    modified: Option<SystemTime>,
    encoding: Option<ContentEncoding>,
    pub(crate) status_code: StatusCode,
    _cd_map: PhantomData<C>,
}

impl NamedFile {
    /// Creates an instance from a previously opened file.
    ///
    /// The given `path` need not exist and is only used to determine the `ContentType` and
    /// `ContentDisposition` headers.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// extern crate actix_web;
    ///
    /// use actix_web::fs::NamedFile;
    /// use std::io::{self, Write};
    /// use std::env;
    /// use std::fs::File;
    ///
    /// fn main() -> io::Result<()> {
    ///     let mut file = File::create("foo.txt")?;
    ///     file.write_all(b"Hello, world!")?;
    ///     let named_file = NamedFile::from_file(file, "bar.txt")?;
    ///     Ok(())
    /// }
    /// ```
    pub fn from_file<P: AsRef<Path>>(file: File, path: P) -> io::Result<NamedFile> {
        Self::from_file_with_config(file, path, DefaultConfig)
    }

    /// Attempts to open a file in read-only mode.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// use actix_web::fs::NamedFile;
    ///
    /// let file = NamedFile::open("foo.txt");
    /// ```
    pub fn open<P: AsRef<Path>>(path: P) -> io::Result<NamedFile> {
        Self::open_with_config(path, DefaultConfig)
    }
}

impl<C: StaticFileConfig> NamedFile<C> {
    /// Creates an instance from a previously opened file using the provided configuration.
    ///
    /// The given `path` need not exist and is only used to determine the `ContentType` and
    /// `ContentDisposition` headers.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// extern crate actix_web;
    ///
    /// use actix_web::fs::{DefaultConfig, NamedFile};
    /// use std::io::{self, Write};
    /// use std::env;
    /// use std::fs::File;
    ///
    /// fn main() -> io::Result<()> {
    ///     let mut file = File::create("foo.txt")?;
    ///     file.write_all(b"Hello, world!")?;
    ///     let named_file = NamedFile::from_file_with_config(file, "bar.txt", DefaultConfig)?;
    ///     Ok(())
    /// }
    /// ```
    pub fn from_file_with_config<P: AsRef<Path>>(
        file: File,
        path: P,
        _: C,
    ) -> io::Result<NamedFile<C>> {
        let path = path.as_ref().to_path_buf();

        // Get the name of the file and use it to construct default Content-Type
        // and Content-Disposition values
        let (content_type, content_disposition) = {
            let filename = match path.file_name() {
                Some(name) => name.to_string_lossy(),
                None => {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "Provided path has no filename",
                    ));
                }
            };

            let ct = guess_mime_type(&path);
            let disposition_type = C::content_disposition_map(ct.type_());
            let cd = ContentDisposition {
                disposition: disposition_type,
                parameters: vec![DispositionParam::Filename(filename.into_owned())],
            };
            (ct, cd)
        };

        let md = file.metadata()?;
        let modified = md.modified().ok();
        let encoding = None;
        Ok(NamedFile {
            path,
            file,
            content_type,
            content_disposition,
            md,
            modified,
            encoding,
            status_code: StatusCode::OK,
            _cd_map: PhantomData,
        })
    }

    /// Attempts to open a file in read-only mode using provided configuration.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// use actix_web::fs::{DefaultConfig, NamedFile};
    ///
    /// let file = NamedFile::open_with_config("foo.txt", DefaultConfig);
    /// ```
    pub fn open_with_config<P: AsRef<Path>>(
        path: P,
        config: C,
    ) -> io::Result<NamedFile<C>> {
        Self::from_file_with_config(File::open(&path)?, path, config)
    }

    /// Returns reference to the underlying `File` object.
    #[inline]
    pub fn file(&self) -> &File {
        &self.file
    }

    /// Retrieve the path of this file.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// # use std::io;
    /// use actix_web::fs::NamedFile;
    ///
    /// # fn path() -> io::Result<()> {
    /// let file = NamedFile::open("test.txt")?;
    /// assert_eq!(file.path().as_os_str(), "foo.txt");
    /// # Ok(())
    /// # }
    /// ```
    #[inline]
    pub fn path(&self) -> &Path {
        self.path.as_path()
    }

    /// Set response **Status Code**
    pub fn set_status_code(mut self, status: StatusCode) -> Self {
        self.status_code = status;
        self
    }

    /// Set the MIME Content-Type for serving this file. By default
    /// the Content-Type is inferred from the filename extension.
    #[inline]
    pub fn set_content_type(mut self, mime_type: mime::Mime) -> Self {
        self.content_type = mime_type;
        self
    }

    /// Set the Content-Disposition for serving this file. This allows
    /// changing the inline/attachment disposition as well as the filename
    /// sent to the peer. By default the disposition is `inline` for text,
    /// image, and video content types, and `attachment` otherwise, and
    /// the filename is taken from the path provided in the `open` method
    /// after converting it to UTF-8 using
    /// [to_string_lossy](https://doc.rust-lang.org/std/ffi/struct.OsStr.html#method.to_string_lossy).
    #[inline]
    pub fn set_content_disposition(mut self, cd: header::ContentDisposition) -> Self {
        self.content_disposition = cd;
        self
    }

    /// Set content encoding for serving this file
    #[inline]
    pub fn set_content_encoding(mut self, enc: ContentEncoding) -> Self {
        self.encoding = Some(enc);
        self
    }

    pub(crate) fn etag(&self) -> Option<header::EntityTag> {
        // This etag format is similar to Apache's.
        self.modified.as_ref().map(|mtime| {
            let ino = {
                #[cfg(unix)]
                {
                    self.md.ino()
                }
                #[cfg(not(unix))]
                {
                    0
                }
            };

            let dur = mtime
                .duration_since(UNIX_EPOCH)
                .expect("modification time must be after epoch");
            header::EntityTag::strong(format!(
                "{:x}:{:x}:{:x}:{:x}",
                ino,
                self.md.len(),
                dur.as_secs(),
                dur.subsec_nanos()
            ))
        })
    }

    pub(crate) fn last_modified(&self) -> Option<header::HttpDate> {
        self.modified.map(|mtime| mtime.into())
    }
}

impl<C> Deref for NamedFile<C> {
    type Target = File;

    fn deref(&self) -> &File {
        &self.file
    }
}

impl<C> DerefMut for NamedFile<C> {
    fn deref_mut(&mut self) -> &mut File {
        &mut self.file
    }
}

/// Returns true if `req` has no `If-Match` header or one which matches `etag`.
fn any_match(etag: Option<&header::EntityTag>, req: &HttpRequest) -> bool {
    match req.get_header::<header::IfMatch>() {
        None | Some(header::IfMatch::Any) => true,
        Some(header::IfMatch::Items(ref items)) => {
            if let Some(some_etag) = etag {
                for item in items {
                    if item.strong_eq(some_etag) {
                        return true;
                    }
                }
            }
            false
        }
    }
}

/// Returns true if `req` doesn't have an `If-None-Match` header matching `req`.
fn none_match(etag: Option<&header::EntityTag>, req: &HttpRequest) -> bool {
    match req.get_header::<header::IfNoneMatch>() {
        Some(header::IfNoneMatch::Any) => false,
        Some(header::IfNoneMatch::Items(ref items)) => {
            if let Some(some_etag) = etag {
                for item in items {
                    if item.weak_eq(some_etag) {
                        return false;
                    }
                }
            }
            true
        }
        None => true,
    }
}

impl<C: StaticFileConfig> Responder for NamedFile<C> {
    type Error = Error;
    type Future = Result<HttpResponse, Error>;

    fn respond_to(self, req: &HttpRequest) -> Self::Future {
        if self.status_code != StatusCode::OK {
            let mut resp = HttpResponse::build(self.status_code);
            resp.set(header::ContentType(self.content_type.clone()))
                .header(
                    header::CONTENT_DISPOSITION,
                    self.content_disposition.to_string(),
                );

            // if file encoding has been set, propagate to response header
            if let Some(current_encoding) = self.encoding {
                resp.set_header(CONTENT_ENCODING, current_encoding.as_str());
            }

            let reader = ChunkedReadFile {
                size: self.md.len(),
                offset: 0,
                file: Some(self.file),
                fut: None,
                counter: 0,
            };
            return Ok(resp.streaming(reader));
        }

        if !C::is_method_allowed(req.method()) {
            return Ok(HttpResponse::MethodNotAllowed()
                .header(header::CONTENT_TYPE, "text/plain")
                .header(header::ALLOW, "GET, HEAD")
                .body("This resource only supports GET and HEAD."));
        }

        let etag = if C::is_use_etag() { self.etag() } else { None };
        let last_modified = if C::is_use_last_modifier() {
            self.last_modified()
        } else {
            None
        };

        // check preconditions
        let precondition_failed = if !any_match(etag.as_ref(), req) {
            true
        } else if let (Some(ref m), Some(header::IfUnmodifiedSince(ref since))) =
            (last_modified, req.get_header())
        {
            m > since
        } else {
            false
        };

        // check last modified
        let not_modified = if !none_match(etag.as_ref(), req) {
            true
        } else if req.headers().contains_key(header::IF_NONE_MATCH) {
            false
        } else if let (Some(ref m), Some(header::IfModifiedSince(ref since))) =
            (last_modified, req.get_header())
        {
            m <= since
        } else {
            false
        };

        let mut resp = HttpResponse::build(self.status_code);
        resp.set(header::ContentType(self.content_type.clone()))
            .header(
                header::CONTENT_DISPOSITION,
                self.content_disposition.to_string(),
            );

        // if file encoding has been set, propagate to response header
        if let Some(current_encoding) = self.encoding {
            resp.set_header(CONTENT_ENCODING, current_encoding.as_str());
        }

        resp.if_some(last_modified, |lm, resp| {
            resp.set(header::LastModified(lm));
        })
        .if_some(etag, |etag, resp| {
            resp.set(header::ETag(etag));
        });

        resp.header(header::ACCEPT_RANGES, "bytes");

        let mut length = self.md.len();
        let mut offset = 0;

        // check for range header
        if let Some(ranges) = req.headers().get(header::RANGE) {
            if let Ok(rangesheader) = ranges.to_str() {
                if let Ok(rangesvec) = HttpRange::parse(rangesheader, length) {
                    length = rangesvec[0].length;
                    offset = rangesvec[0].start;

                    // if file encoding has been set, propagate to response header
                    if let Some(current_encoding) = self.encoding {
                        resp.set_header(CONTENT_ENCODING, current_encoding.as_str());
                    }

                    resp.header(
                        header::CONTENT_RANGE,
                        format!(
                            "bytes {}-{}/{}",
                            offset,
                            offset + length - 1,
                            self.md.len()
                        ),
                    );
                } else {
                    resp.header(header::CONTENT_RANGE, format!("bytes */{}", length));
                    return Ok(resp.status(StatusCode::RANGE_NOT_SATISFIABLE).finish());
                };
            } else {
                return Ok(resp.status(StatusCode::BAD_REQUEST).finish());
            };
        };

        resp.header(header::CONTENT_LENGTH, format!("{}", length));

        if precondition_failed {
            return Ok(resp.status(StatusCode::PRECONDITION_FAILED).finish());
        } else if not_modified {
            return Ok(resp.status(StatusCode::NOT_MODIFIED).finish());
        }

        if *req.method() == Method::HEAD {
            Ok(resp.finish())
        } else {
            let reader = ChunkedReadFile {
                offset,
                size: length,
                file: Some(self.file),
                fut: None,
                counter: 0,
            };
            if offset != 0 || length != self.md.len() {
                return Ok(resp.status(StatusCode::PARTIAL_CONTENT).streaming(reader));
            };
            Ok(resp.streaming(reader))
        }
    }
}
