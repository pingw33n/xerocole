use flate2::{Decompress, FlushDecompress, Status};
use gzip_header::read_gz_header;
use std::io;

use super::*;
use std::io::Cursor;

pub struct Provider;

impl Provider {
    pub const NAME: &'static str = "gzip";
}

impl crate::component::Provider for Provider {
    fn metadata(&self) -> Metadata {
        Metadata {
            kind: ComponentKind::StreamDecoder,
            name: Self::NAME,
        }
    }
}

impl DecoderProvider for Provider {
    fn new(&self, _ctx: New) -> Result<Arc<Factory>> {
        Ok(Arc::new(FactoryImpl {
        }))
    }
}

struct FactoryImpl {
}

impl Factory for FactoryImpl {
    fn new(&self) -> Box<Decoder> {
        Box::new(DecoderImpl {
            state: State::Header,
        })
    }
}

enum State {
    Header,
    Decompress(Decompress),
    Footer,
}

struct DecoderImpl {
    state: State,
}

impl Decoder for DecoderImpl {
    fn decode(&mut self, inp: &[u8], out: &mut [u8]) -> Result<Decode> {
        match &self.state {
            State::Header => {
                let inp = &mut Cursor::new(inp);
                return match read_gz_header(inp) {
                    Ok(_) => {
                        self.state = State::Decompress(Decompress::new(false));
                        Ok(Decode {
                            read: inp.position() as usize,
                            written: 0,
                        })
                    }
                    Err(e) => {
                        return if e.kind() == io::ErrorKind::UnexpectedEof {
                            Ok(Decode {
                                read: 0,
                                written: 0,
                            })
                        } else {
                            Err(e.wrap_id(ErrorId::Io))
                        };
                    }
                }
            }
            State::Footer => {
                return Ok(if inp.len() >= 8 {
                    self.state = State::Header;
                    Decode {
                        read: 8,
                        written: 0,

                    }
                } else {
                    Decode {
                        read: 0,
                        written: 0,

                    }
                });
            }
            State::Decompress(_) => {}
        }

        let dec = if let State::Decompress(dec) = &mut self.state {
            dec
        } else {
            unreachable!();
        };
        let (r, stream_end) = {
            let in_before = dec.total_in();
            let out_before = dec.total_out();
            let status = dec.decompress(inp, out, FlushDecompress::None)
                .wrap_err_id(ErrorId::Io)?;
            let read = (dec.total_in() - in_before) as usize;
            let written = (dec.total_out() - out_before) as usize;
            let stream_end = status == Status::StreamEnd;
            (Decode {
                read,
                written,
            }, stream_end)
        };
        if stream_end {
            self.state = State::Footer;
        }
        Ok(r)
    }
}