use serde::Serialize;
/// Streaming NDJSON (Newline-Delimited JSON) reader and writer.
///
/// The writer serialises one JSON object per line, separated by `'\n'`.
/// The reader deserialises lines lazily via the `Iterator` trait so that
/// arbitrarily large files can be processed without loading them entirely
/// into memory.
use serde::de::DeserializeOwned;
use std::io::{BufRead, BufReader, BufWriter, Read, Write};

/// Streams serialised records as NDJSON lines to the underlying writer.
pub struct NdjsonWriter<W: Write> {
    writer: BufWriter<W>,
    count: u64,
}

impl<W: Write> NdjsonWriter<W> {
    /// Wrap `writer` in a buffered NDJSON stream.
    pub fn new(writer: W) -> Self {
        Self {
            writer: BufWriter::new(writer),
            count: 0,
        }
    }

    /// Serialise a single record as one JSON line and append a newline.
    pub fn write_record<T: Serialize>(&mut self, record: &T) -> anyhow::Result<()> {
        serde_json::to_writer(&mut self.writer, record)?;
        self.writer.write_all(b"\n")?;
        self.count += 1;
        Ok(())
    }

    /// How many records have been written so far.
    pub fn count(&self) -> u64 {
        self.count
    }

    /// Flush buffers and return the inner writer.
    pub fn finish(mut self) -> anyhow::Result<W> {
        self.writer.flush()?;
        // BufWriter::into_inner can fail; map the error.
        self.writer
            .into_inner()
            .map_err(|e| anyhow::anyhow!("flush error: {e}"))
    }
}

/// Lazily reads NDJSON lines from a buffered reader, yielding
/// `serde_json::Value` items.
pub struct NdjsonReader<R: BufRead> {
    reader: R,
    line_buf: String,
}

impl<R: BufRead> NdjsonReader<R> {
    /// Create a new reader over an already-buffered source.
    pub fn new(reader: R) -> Self {
        Self {
            reader,
            line_buf: String::new(),
        }
    }
}

impl<R: BufRead> Iterator for NdjsonReader<R> {
    type Item = anyhow::Result<serde_json::Value>;

    fn next(&mut self) -> Option<Self::Item> {
        self.line_buf.clear();
        match self.reader.read_line(&mut self.line_buf) {
            Ok(0) => None, // EOF
            Ok(_) => {
                let trimmed = self.line_buf.trim();
                if trimmed.is_empty() {
                    // Skip blank lines and try the next one.
                    return self.next();
                }
                Some(
                    serde_json::from_str(trimmed)
                        .map_err(|e| anyhow::anyhow!("NDJSON parse error: {e}")),
                )
            }
            Err(e) => Some(Err(e.into())),
        }
    }
}

/// Convenience: create a typed NDJSON reader that deserialises each line
/// directly into `T`.
pub fn read_typed<T: DeserializeOwned, R: Read>(
    reader: R,
) -> impl Iterator<Item = anyhow::Result<T>> {
    let buf = BufReader::new(reader);
    TypedNdjsonReader {
        reader: buf,
        line_buf: String::new(),
        _phantom: std::marker::PhantomData,
    }
}

/// Internal iterator for `read_typed`.
struct TypedNdjsonReader<T, R: BufRead> {
    reader: R,
    line_buf: String,
    _phantom: std::marker::PhantomData<T>,
}

impl<T: DeserializeOwned, R: BufRead> Iterator for TypedNdjsonReader<T, R> {
    type Item = anyhow::Result<T>;

    fn next(&mut self) -> Option<Self::Item> {
        self.line_buf.clear();
        match self.reader.read_line(&mut self.line_buf) {
            Ok(0) => None,
            Ok(_) => {
                let trimmed = self.line_buf.trim();
                if trimmed.is_empty() {
                    return self.next();
                }
                Some(
                    serde_json::from_str(trimmed)
                        .map_err(|e| anyhow::anyhow!("NDJSON parse error: {e}")),
                )
            }
            Err(e) => Some(Err(e.into())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};
    use std::io::Cursor;

    #[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
    struct TestRecord {
        id: u64,
        name: String,
    }

    #[test]
    fn write_and_read_records() {
        let mut buf = Vec::new();
        {
            let mut writer = NdjsonWriter::new(&mut buf);
            for i in 0..5 {
                writer
                    .write_record(&TestRecord {
                        id: i,
                        name: format!("item-{i}"),
                    })
                    .unwrap();
            }
            assert_eq!(writer.count(), 5);
            writer.finish().unwrap();
        }

        let reader = NdjsonReader::new(BufReader::new(Cursor::new(&buf)));
        let values: Vec<serde_json::Value> = reader.map(|r| r.unwrap()).collect();
        assert_eq!(values.len(), 5);
        assert_eq!(values[0]["id"], 0);
        assert_eq!(values[4]["name"], "item-4");
    }

    #[test]
    fn typed_read() {
        let mut buf = Vec::new();
        {
            let mut writer = NdjsonWriter::new(&mut buf);
            writer
                .write_record(&TestRecord {
                    id: 1,
                    name: "hello".into(),
                })
                .unwrap();
            writer
                .write_record(&TestRecord {
                    id: 2,
                    name: "world".into(),
                })
                .unwrap();
            writer.finish().unwrap();
        }

        let records: Vec<TestRecord> = read_typed(Cursor::new(&buf)).map(|r| r.unwrap()).collect();
        assert_eq!(records.len(), 2);
        assert_eq!(
            records[0],
            TestRecord {
                id: 1,
                name: "hello".into()
            }
        );
    }

    #[test]
    fn empty_file_yields_no_records() {
        let reader = NdjsonReader::new(BufReader::new(Cursor::new(b"")));
        let values: Vec<_> = reader.collect();
        assert!(values.is_empty());
    }

    #[test]
    fn blank_lines_are_skipped() {
        let data = b"{\"id\":1}\n\n\n{\"id\":2}\n";
        let reader = NdjsonReader::new(BufReader::new(Cursor::new(data)));
        let values: Vec<serde_json::Value> = reader.map(|r| r.unwrap()).collect();
        assert_eq!(values.len(), 2);
    }

    #[test]
    fn malformed_line_returns_error() {
        let data = b"not-json\n";
        let reader = NdjsonReader::new(BufReader::new(Cursor::new(data)));
        let results: Vec<_> = reader.collect();
        assert_eq!(results.len(), 1);
        assert!(results[0].is_err());
    }

    #[test]
    fn finish_returns_inner_writer() {
        let buf: Vec<u8> = Vec::new();
        let writer = NdjsonWriter::new(buf);
        let inner = writer.finish().unwrap();
        assert!(inner.is_empty());
    }

    #[test]
    fn large_batch_round_trip() {
        let mut buf = Vec::new();
        let count = 1_000;
        {
            let mut writer = NdjsonWriter::new(&mut buf);
            for i in 0..count {
                writer
                    .write_record(&TestRecord {
                        id: i,
                        name: format!("record-{i}"),
                    })
                    .unwrap();
            }
            assert_eq!(writer.count(), count);
            writer.finish().unwrap();
        }

        let records: Vec<TestRecord> = read_typed(Cursor::new(&buf)).map(|r| r.unwrap()).collect();
        assert_eq!(records.len(), count as usize);
        assert_eq!(records[999].id, 999);
    }
}
