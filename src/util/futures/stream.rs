use futures::prelude::*;
use std::marker::PhantomData;

pub trait StreamExt: Stream {
    fn infallible<E>(self) -> Infallible<Self, E>
            where Self: Sized {
        Infallible {
            stream: self,
            _ty: PhantomData,
        }
    }
}

impl<T: Stream> StreamExt for T {
}

pub struct Infallible<S, E>{
    stream: S,
    _ty: PhantomData<E>,
}

impl<S: Stream, E> Stream for Infallible<S, E> {
    type Item = S::Item;
    type Error = E;

    fn poll(&mut self) -> Poll<Option<S::Item>, E> {
        self.stream.poll().map_err(|_| -> E {
            panic!("infallible stream failed");
        })
    }
}