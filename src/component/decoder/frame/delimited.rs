use memchr::*;
use regex::bytes::Regex;
use std::marker::PhantomData;
use std::ops::Range;

use super::*;
use crate::component::{ComponentKind, Metadata, Provider as CProvider};
use crate::value::*;

pub const NAME: &'static str = "delimited";

const STRING: &'static str = "string";
const LINE: &'static str = "line";
const GLUE: &'static str = "glue";
const GLUE_ON: &'static str = "on";
const GLUE_TO: &'static str = "to";
const GLUE_TO_PREVIOUS: &'static str = "previous";
const GLUE_TO_NEXT: &'static str = "next";

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

        let glue = if let Some(glue) = ctx.config.remove_opt(GLUE)? {
            let on = glue.get(GLUE_ON)?;
            let on = Regex::new(on.as_str()?)
                .map_err(move |_| on.new_error("invalid regular expression"))?;

            let to = glue.get(GLUE_TO)?;
            let to = match to.as_str()? {
                GLUE_TO_PREVIOUS => GlueTo::Previous,
                GLUE_TO_NEXT => GlueTo::Next,
                _ => return Err(ErrorDetails::new(
                        format!("`{}` must be one of [\"{}\", \"{}\"]",
                            to.as_str().unwrap(), GLUE_TO_PREVIOUS, GLUE_TO_NEXT),
                        to.span.clone())
                        .wrap_id(ErrorId::Parse)),
            };
            Some(Glue {
                on,
                to,
            })
        } else {
            None
        };

        Ok(Arc::new(FactoryImpl {
            delimiter,
            glue,
        }))
    }
}

struct FactoryImpl {
    delimiter: Delimiter,
    glue: Option<Glue>,
}

impl Factory for FactoryImpl {
    fn new(&self) -> Box<Decoder> {
        Box::new(DecoderImpl {
            delimiter: self.delimiter.clone(),
            glue: self.glue.clone().map(|config| GlueState::new(config))
        })
    }
}

#[derive(Clone, Debug)]
enum Delimiter {
    Line,
    String(String),
}

#[derive(Clone, Copy, Debug)]
enum GlueTo {
    Previous,
    Next,
}

#[derive(Clone, Debug)]
struct Glue {
    on: Regex,
    to: GlueTo,
}

#[derive(Debug)]
struct Flush {
    slice: Range<usize>,
    delim_len: usize,
}

#[derive(Debug)]
enum GlueResult {
    Flush(Flush),
    Glue,
}

#[derive(Debug)]
struct GlueState {
    config: Glue,

    /// Points to the start of active slice of the glue buffer.
    /// Effectively it is located immediately after the last flushed slice.
    start: usize,

    /// Absolute position in the glue buffer. This is where new frame will begin.
    pos: usize,

    /// Last matched delimiter len.
    delim_len: usize,
}

impl GlueState {
    pub fn new(config: Glue) -> Self {
        Self {
            config,
            start: 0,
            pos: 0,
            delim_len: 0,
        }
    }

    /// `buf` is the current glue buffer which consists of the previous glued frames with new
    /// frame at `self.pos`. The new frame includes the delimiter bytes of len `delim_len`.
    pub fn update(&mut self, buf: &[u8], delim_len: usize) -> GlueResult {
        // line
        //     glued^
        //     glued^
        // line

        // line\
        // glued\
        // glued
        // line

        let frame = &buf[self.pos..buf.len() - delim_len];
        if self.config.on.is_match(frame) {
            match self.config.to {
                GlueTo::Previous => {
                    self.pos = buf.len();
                    self.delim_len = delim_len;
                }
                GlueTo::Next => {}
            }
            GlueResult::Glue
        } else {
            let r = match self.config.to {
                GlueTo::Previous => {
                    let r = if self.pos == self.start {
                        GlueResult::Glue
                    } else {
                        GlueResult::Flush(Flush {
                            slice: self.start..self.pos,
                            delim_len: self.delim_len,
                        })
                    };
                    self.start = self.pos;
                    r
                }
                GlueTo::Next => {
                    let r = GlueResult::Flush(Flush {
                        slice: self.start..buf.len(),
                        delim_len,
                    });
                    dbg!(&r);
                    self.start = buf.len();
                    r
                }
            };
            self.pos = buf.len();
            self.delim_len = delim_len;
            r
        }
    }

    pub fn reset(&mut self) {
        self.start = 0;
        self.pos = 0;
        self.delim_len = 0;
    }

    pub fn rebase(&mut self) {
        self.pos -= self.start;
        self.start = 0;
        self.delim_len = 0;
    }
}

struct DecoderImpl {
    delimiter: Delimiter,
    glue: Option<GlueState>,
}

impl Decoder for DecoderImpl {
    fn decode<'a>(&mut self, inp: &'a [u8], out: &mut Vec<&'a [u8]>) -> Result<Decode> {
        let start = self.glue.as_ref().map(|g| g.pos).unwrap_or(0);
        Ok(match &self.delimiter {
            Delimiter::Line => decode_line(inp, out, &mut self.glue, start),
            Delimiter::String(s) => {
                let s = s.as_bytes();
                match s.len() {
                    0 => decode_undelimited(inp, out),

                    1 => decode_string(inp, out, &mut self.glue,
                        new_memchr1(s[0], inp, start)
                        .map(|i| (i, 1))),

                    2 => decode_string(inp, out, &mut self.glue,
                        new_memchr2(s[0], s[1], inp, start)
                        .map(|i| (i, 2))),

                    3 => decode_string(inp, out, &mut self.glue,
                        new_memchr3(s[0], s[1], s[2], inp, start)
                        .map(|i| (i, 3))),

                    len => decode_string(inp, out, &mut self.glue,
                        new_memchr3_overlapping(s[0], s[1], s[2], inp, start)
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
        if let Some(glue) = &mut self.glue {
            glue.reset();
        }
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
    pub fn new(buf: &'a [u8], start: usize) -> Self {
        let mut r = Self {
            buf,
            cr: 0,
            lf: 0,
        };
        r.next_cr(start);
        r.next_lf(start);
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

fn decode_line<'a>(inp: &'a [u8], out: &mut Vec<&'a [u8]>,
    glue: &mut Option<GlueState>, start: usize) -> Decode
{
    decode_string(inp, out, glue, LineEndings::new(inp, start))
}

fn finish_line<'a>(inp: &'a [u8], out: &mut Vec<&'a [u8]>) -> Decode {
    let trailing_delim = if inp.len() > 1 &&
        inp[inp.len() - 2] == b'\r' && inp[inp.len() - 1] == b'\n'
    {
        2
    } else if inp.len() > 0 && (inp[inp.len() - 1] == b'\r' || inp[inp.len() - 1] == b'\n') {
        1
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
    glue: &mut Option<GlueState>,
    iter: impl Iterator<Item=(usize, usize)>) -> Decode
{
    let mut read = 0;
    let mut written = 0;
    if let Some(glue) = glue {
        for (i, delimiter_len) in iter {
            assert!(i >= glue.pos);
            match glue.update(&inp[..i + delimiter_len], delimiter_len) {
                GlueResult::Glue => continue,
                GlueResult::Flush(flush) => {
                    dbg!((i, delimiter_len));
                    dbg!(&flush);
                    dbg!(glue.delim_len);
                    out.push(&inp[flush.slice.start..flush.slice.end - flush.delim_len]);
                    read = flush.slice.end;
                }
            }
            written += 1;
        }
        glue.rebase();
        dbg!(&glue);
    } else {
        for (i, delimiter_len) in iter {
            dbg!((i, delimiter_len));
            out.push(&inp[read..i]);
            read = i + delimiter_len;
            written += 1;
        }
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

    mod glue {
        use super::*;

        mod any_line {
            use super::*;

            fn new<'a>(on: &str, to: GlueTo) -> (Box<Decoder>, Vec<&'a [u8]>) {
                let to = match to {
                    GlueTo::Previous => GLUE_TO_PREVIOUS,
                    GlueTo::Next => GLUE_TO_NEXT,
                };
                let dec = ProviderImpl.new(New { config: value! {{
                    GLUE => {
                        GLUE_ON => on,
                        GLUE_TO => to
                    }
                }}.into() }).unwrap().new();
                let frames = Vec::new();
                (dec, frames)
            }

            #[test]
            fn resets_after_finish() {
                let (ref mut dec, ref mut frames) = new("^!", GlueTo::Previous);

                assert_eq!(dec.decode(&b"line1\nline2\n"[..], frames).unwrap(), decode(6, 1));
                assert_eq!(dec.finish(&b"line2\n"[..], frames).unwrap(), decode(6, 2));
                assert_eq!(dec.decode(&b"line3\n\n"[..], frames).unwrap(), decode(6, 1));

                assert_eq!(&frames[..], &[
                    &b"line1"[..],
                    &b"line2"[..],
                    &b""[..],
                    &b"line3"[..],
                ]);
            }

            #[test]
            fn line_to_previous() {
                let (ref mut dec, ref mut frames) = new("^[\\s!]", GlueTo::Previous);

                let inp = &b"line0\r\n\
                    line1\n line1.2\
                    \n! line1.3\
                    \nline2\n\
                    \tline2.1\r"
                    [..];

                assert_eq!(dec.decode(&inp[..13], frames).unwrap(), decode(7, 1));
                assert_eq!(&frames[..], &[&b"line0"[..]]);

                frames.clear();
                assert_eq!(dec.decode(&inp[7..21], frames).unwrap(), decode(0, 0));
                assert_eq!(frames.len(), 0);

                assert_eq!(dec.decode(&inp[7..31], frames).unwrap(), decode(0, 0));
                assert_eq!(frames.len(), 0);

                assert_eq!(dec.decode(&inp[7..38], frames).unwrap(), decode(25, 1));
                assert_eq!(&frames[..], &[&b"line1\n line1.2\n! line1.3"[..]]);

                frames.clear();
                assert_eq!(dec.finish(&inp[32..], frames).unwrap(), decode(15, 2));
                assert_eq!(&frames[..], &[&b"line2\n\tline2.1"[..], &b""[..]]);
            }

            #[test]
            fn line_to_next() {
                let (ref mut dec, ref mut frames) = new("[~!]$", GlueTo::Next);

                let inp = &b"line1\r\
                    line2 ~\n\
                    line2.1 !\n\
                    line2.2\n\
                    line3!\r\
                    line3.1~\r"
                    [..];

                assert_eq!(dec.decode(&inp[..10], frames).unwrap(), decode(6, 1));
                assert_eq!(&frames[..], &[&b"line1"[..]]);

                frames.clear();
                assert_eq!(dec.decode(&inp[6..35], frames).unwrap(), decode(26, 1));
                assert_eq!(&frames[..], &[&b"line2 ~\nline2.1 !\nline2.2"[..]]);

                frames.clear();
                assert_eq!(dec.decode(&inp[32..], frames).unwrap(), decode(0, 0));
                assert_eq!(frames.len(), 0);

                assert_eq!(dec.finish(&inp[32..], frames).unwrap(), decode(16, 2));
                assert_eq!(&frames[..], &[&b"line3!\rline3.1~"[..], &b""[..]]);
            }
        }
    }
}