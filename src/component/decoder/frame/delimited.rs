use memchr::*;
use std::marker::PhantomData;

use super::*;
use crate::component::{ComponentKind, Metadata, Provider as CProvider};
use crate::value::*;

pub const NAME: &'static str = "delimited";

const STRING: &'static str = "string";
const LINE: &'static str = "line";

pub fn provider() -> Box<Provider> {
    Box::new(ProviderImpl)
}

struct ProviderImpl;

impl CProvider for ProviderImpl {
    fn metadata(&self) -> Metadata {
        Metadata {
            kind: ComponentKind::FrameDecoder,
            name: NAME,
        }
    }
}

impl Provider for ProviderImpl {
    fn new(&self, mut ctx: New) -> Result<Arc<Factory>> {
        let delimiter = if let Some((key, delimiter)) = ctx.config.remove_exclusive_opt(&[
            STRING,
            LINE,
        ])? {
            match key {
                STRING => Delimiter::String(delimiter.into_string()?),
                LINE => match delimiter.as_str()? {
                    "any" => Delimiter::Line,
                    "dos" => Delimiter::String("\r\n".into()),
                    "unix" => Delimiter::String("\n".into()),
                    "mac" => Delimiter::String("\r".into()),
                    _ => return Err(ErrorDetails::new(
                        format!("`{}` must be one of [\"any\", \"dos\", \"unix\", \"mac\"]", LINE),
                        delimiter.span.clone())
                        .wrap_id(ErrorId::Parse)),
                }
                _ => unreachable!(),
            }
        } else {
            Delimiter::Line
        };

        Ok(Arc::new(FactoryImpl {
            delimiter,
        }))
    }
}

struct FactoryImpl {
    delimiter: Delimiter,
}

impl Factory for FactoryImpl {
    fn new(&self) -> Box<Decoder> {
        Box::new(DecoderImpl {
            delimiter: self.delimiter.clone(),
        })
    }
}

#[derive(Clone, Debug)]
enum Delimiter {
    Line,
    String(String),
}

struct DecoderImpl {
    delimiter: Delimiter,
}

impl Decoder for DecoderImpl {
    fn decode<'a>(&mut self, inp: &'a [u8], out: &mut Vec<&'a [u8]>) -> Result<Decode> {
        Ok(match &self.delimiter {
            Delimiter::Line => decode_line(inp, out),
            Delimiter::String(s) => {
                let s = s.as_bytes();
                match s.len() {
                    0 => decode_undelimited(inp, out),
                    1 => decode_string(inp, out, new_memchr1(s[0], inp, 0)
                        .map(|i| (i, 1))),
                    2 => decode_string(inp, out, new_memchr2(s[0], s[1], inp, 0)
                        .map(|i| (i, 2))),
                    3 => decode_string(inp, out, new_memchr3(s[0], s[1], s[2], inp, 0)
                        .map(|i| (i, 3))),
                    len => decode_string(inp, out, new_memchr3_overlapping(s[0], s[1], s[2], inp, 0)
                        .filter(|i| {
                            let start = i + 3;
                            let end = i + s.len();
                            if inp.len() >= end {
                                &inp[start..end] == &s[3..]
                            } else {
                                false
                            }
                        })
                        .map(|i| (i, len))),
                }
            }
        })
    }

    fn finish<'a>(&mut self, inp: &'a [u8], out: &mut Vec<&'a [u8]>) -> Result<Decode> {
        Ok(match &self.delimiter {
            Delimiter::Line => finish_line(inp, out),
            Delimiter::String(_) => decode_undelimited(inp, out),
        })
    }
}

fn decode_undelimited<'a>(inp: &'a [u8], out: &mut Vec<&'a [u8]>) -> Decode {
    out.push(inp.into());
    Decode {
        read: inp.len(),
        written: 1,
    }
}

/// Iterator over all possible line endings: `\n`, `\r`, `\r\n`.
struct LineEndings<'a> {
    buf: &'a [u8],
    cr: usize,
    lf: usize,
}

impl<'a> LineEndings<'a> {
    pub fn new(buf: &'a [u8]) -> Self {
        let mut r = Self {
            buf,
            cr: 0,
            lf: 0,
        };
        r.next_cr(0);
        r.next_lf(0);
        r
    }

    fn next_cr(&mut self, start: usize) {
        self.cr = memchr(b'\r', &self.buf[start..]).map(|i| start + i).unwrap_or(self.buf.len());
    }

    fn next_lf(&mut self, start: usize) {
        self.lf = memchr(b'\n', &self.buf[start..]).map(|i| start + i).unwrap_or(self.buf.len());
    }
}

impl Iterator for LineEndings<'_> {
    /// (index of the matched line ending delimiter, length of the matched line ending delimiter)
    type Item = (usize, usize);

    fn next(&mut self) -> Option<Self::Item> {
        Some(if self.cr < self.lf {
            if self.cr + 1 >= self.buf.len() {
                return None;
            }
            let len = if self.buf[self.cr + 1] == b'\n' {
                // \r\n
                2
            } else {
                1
            };
            let i = self.cr;

            let j = self.cr + len;
            self.next_cr(j);
            self.next_lf(j);

            (i, len)
        } else {
            if self.lf >= self.buf.len() {
                return None;
            }
            let i = self.lf;
            self.next_lf(self.lf + 1);
            (i, 1)
        })
    }
}

fn decode_line<'a>(inp: &'a [u8], out: &mut Vec<&'a [u8]>) -> Decode {
    decode_string(inp, out, LineEndings::new(inp))
}

fn finish_line<'a>(inp: &'a [u8], out: &mut Vec<&'a [u8]>) -> Decode {
    let trailing_delim = if inp.len() > 0 && inp[inp.len() - 1] == b'\r' {
        1
    } else if inp.len() > 1 && inp[inp.len() - 2] == b'\r' && inp[inp.len() - 1] == b'\n' {
        2
    } else {
        0
    };
    out.push(&inp[..inp.len() - trailing_delim]);
    if trailing_delim > 0 {
        out.push(&[]);
    }
    Decode {
        read: inp.len(),
        written: 1 + (trailing_delim > 0) as usize,
    }
}

fn decode_string<'a>(inp: &'a [u8], out: &mut Vec<&'a [u8]>,
    iter: impl Iterator<Item=(usize, usize)>) -> Decode
{
    let mut read = 0;
    let mut written = 0;
    for (i, delimiter_len) in iter {
        out.push(&inp[read..i]);
        read = i + delimiter_len;
        written += 1;
    }
    Decode {
        read,
        written,
    }
}

trait Len {
    const LEN: usize;
}

struct Len1;
impl Len for Len1 {
    const LEN: usize = 1;
}

struct Len2;
impl Len for Len2 {
    const LEN: usize = 2;
}

struct Len3;
impl Len for Len3 {
    const LEN: usize = 3;
}

/// Iterator over non-overlapping `memchr`-based matches.
struct Memchr<'a, F, L> {
    f: F,
    haystack: &'a [u8],
    i: usize,
    _l: PhantomData<L>,
}

#[inline]
fn new_memchr<L, F>(haystack: &[u8], start: usize, f: F) -> Memchr<F, L>
    where F: FnMut(&[u8]) -> Option<usize>,
          L: Len,
{
    Memchr {
        f,
        haystack,
        i: start,
        _l: PhantomData,
    }
}

#[inline]
fn new_memchr1<'a>(n: u8, hs: &'a [u8], start: usize) -> impl 'a + Iterator<Item=usize> {
    new_memchr::<Len1, _>(hs, start, move |hs| memchr(n, hs))
}

#[inline]
fn new_memchr2<'a>(n1: u8, n2: u8, hs: &'a [u8], start: usize) -> impl 'a + Iterator<Item=usize> {
    new_memchr::<Len2, _>(hs, start, move |hs| memchr2(n1, n2, hs))
}

#[inline]
fn new_memchr3<'a>(n1: u8, n2: u8, n3: u8, hs: &'a [u8], start: usize) -> impl 'a + Iterator<Item=usize> {
    new_memchr::<Len3, _>(hs, start, move |hs| memchr3(n1, n2, n3, hs))
}

#[inline]
fn new_memchr3_overlapping<'a>(n1: u8, n2: u8, n3: u8, hs: &'a [u8], start: usize) -> impl 'a + Iterator<Item=usize> {
    new_memchr::<Len1, _>(hs, start, move |hs| memchr3(n1, n2, n3, hs))
}

impl<F, L> Iterator for Memchr<'_, F, L>
    where F: FnMut(&[u8]) -> Option<usize>,
          L: Len,
{
    type Item = usize;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        if let Some(i) = (self.f)(&self.haystack[self.i..]) {
            self.i += i;
            let i = self.i;
            self.i += L::LEN;
            Some(i)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    fn decode(read: usize, written: usize) -> Decode {
        Decode {
            read,
            written,
        }
    }

    mod line_any {
        use super::*;

        fn new<'a>() -> (Box<Decoder>, Vec<&'a [u8]>) {
            let dec = ProviderImpl.new(New { config: value!{{}}.into() }).unwrap().new();
            let frames = Vec::new();
            (dec, frames)
        }

        #[test]
        fn empty() {
            let (ref mut dec, ref mut frames) = new();

            assert_eq!(dec.decode(&b""[..], frames).unwrap(), decode(0, 0));
            assert_eq!(frames.len(), 0);
            assert_eq!(dec.finish(&b""[..], frames).unwrap(), decode(0, 1));
            assert_eq!(&frames[..], &[&b""[..]]);
        }

        #[test]
        fn trailing() {
            let (ref mut dec, ref mut frames) = new();

            assert_eq!(dec.decode(&b"test\x00"[..], frames).unwrap(), decode(0, 0));
            assert_eq!(frames.len(), 0);
            assert_eq!(dec.finish(&b"test\x00"[..], frames).unwrap(), decode(5, 1));
            assert_eq!(&frames[..], &[&b"test\x00"[..]]);
        }

        #[test]
        fn lf() {
            let (ref mut dec, ref mut frames) = new();

            assert_eq!(dec.decode(&b"\n"[..], frames).unwrap(), decode(1, 1));
            assert_eq!(&frames[..], &[&b""[..]]);
        }

        #[test]
        fn cr() {
            let (ref mut dec, ref mut frames) = new();

            assert_eq!(dec.decode(&b"\r"[..], frames).unwrap(), decode(0, 0));
            assert_eq!(frames.len(), 0);
            assert_eq!(dec.finish(&b"\r"[..], frames).unwrap(), decode(1, 2));
            assert_eq!(&frames[..], &[&b""[..], &b""[..]]);
        }

        #[test]
        fn cr_lf() {
            let (ref mut dec, ref mut frames) = new();

            assert_eq!(dec.decode(&b"\r"[..], frames).unwrap(), decode(0, 0));
            assert_eq!(frames.len(), 0);
            assert_eq!(dec.finish(&b"\r\n"[..], frames).unwrap(), decode(2, 2));
            assert_eq!(&frames[..], &[&b""[..], &b""[..]]);
        }

        #[test]
        fn mixed_empty() {
            let (ref mut dec, ref mut frames) = new();

            assert_eq!(dec.decode(&b"\n\n"[..], frames).unwrap(), decode(2, 2));
            assert_eq!(&frames[..], &[&b""[..], &b""[..]]);

            assert_eq!(dec.decode(&b"\r\n"[..], frames).unwrap(), decode(2, 1));
            assert_eq!(&frames[2..], &[&b""[..]]);

            assert_eq!(dec.decode(&b"\r\n\r\n"[..], frames).unwrap(), decode(4, 2));
            assert_eq!(&frames[3..], &[&b""[..], &b""[..]]);

            assert_eq!(dec.decode(&b"\r\n\n\r\r\n"[..], frames).unwrap(), decode(6, 4));
            assert_eq!(&frames[5..], &[&b""[..], &b""[..],
                &b""[..], &b""[..]]);

            assert_eq!(dec.decode(&b"\n\r\n"[..], frames).unwrap(), decode(3, 2));
            assert_eq!(&frames[9..], &[&b""[..], &b""[..]]);
        }

        #[test]
        fn mixed() {
            let (ref mut dec, ref mut frames) = new();

            let s = &b"line 1\r\n\
                   line 2\n\
                   line 3\r\n\
                   line 4"[..];
            assert_eq!(dec.decode(&s[..22], frames).unwrap(), decode(15, 2));
            assert_eq!(&frames[..], &[&b"line 1"[..], &b"line 2"[..]]);

            assert_eq!(dec.decode(&s[15..], frames).unwrap(), decode(8, 1));
            assert_eq!(&frames[2..], &[&b"line 3"[..]]);

            assert_eq!(dec.finish(&s[23..], frames).unwrap(), decode(6, 1));
            assert_eq!(&frames[3..], &[&b"line 4"[..]]);

            assert_eq!(&frames[..], &[
                &b"line 1"[..],
                &b"line 2"[..],
                &b"line 3"[..],
                &b"line 4"[..],
            ]);
        }
    }

    mod string {
        use super::*;

        fn new<'a>() -> (Box<Decoder>, Vec<&'a [u8]>) {
            new_with_str("~!~")
        }

        fn new_with_str<'a>(s: &str) -> (Box<Decoder>, Vec<&'a [u8]>) {
            let dec = ProviderImpl.new(New { config: value!{{ STRING => s }}.into() })
                .unwrap().new();
            let frames = Vec::new();
            (dec, frames)
        }

        #[test]
        fn empty() {
            let (ref mut dec, ref mut frames) = new();

            assert_eq!(dec.decode(&b""[..], frames).unwrap(), decode(0, 0));
            assert_eq!(frames.len(), 0);
            assert_eq!(dec.finish(&b""[..], frames).unwrap(), decode(0, 1));
            assert_eq!(&frames[..], &[&b""[..]]);
        }

        #[test]
        fn one() {
            let (ref mut dec, ref mut frames) = new();

            assert_eq!(dec.decode(&b"test\x00~!~"[..], frames).unwrap(), decode(8, 1));
            assert_eq!(&frames[..], &[&b"test\x00"[..]]);
            assert_eq!(dec.finish(&b""[..], frames).unwrap(), decode(0, 1));
            assert_eq!(&frames[..], &[&b"test\x00"[..], &b""[..]]);
        }

        #[test]
        fn two() {
            let (ref mut dec, ref mut frames) = new();

            assert_eq!(dec.decode(&b"test\x01~!~test\x02~!~"[..], frames).unwrap(), decode(16, 2));
            assert_eq!(&frames[..], &[&b"test\x01"[..], &b"test\x02"[..]]);
            assert_eq!(dec.finish(&b""[..], frames).unwrap(), decode(0, 1));
            assert_eq!(&frames[..], &[&b"test\x01"[..], &b"test\x02"[..], &b""[..]]);
        }

        #[test]
        fn long() {
            let (ref mut dec, ref mut frames) = new_with_str("ddddelim");

            assert_eq!(dec.decode(&b"line1_dddddelim_line2_ddddelim"[..], frames).unwrap(),
                decode(30, 2));
            assert_eq!(&frames[..], &[&b"line1_d"[..], &b"_line2_"[..]]);
        }
    }
}