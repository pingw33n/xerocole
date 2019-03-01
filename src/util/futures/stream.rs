use futures::prelude::*;
use std::marker::PhantomData;

pub type BoxStream<T, E> = Box<Stream<Item=T, Error=E> + Send + 'static>;

pub trait StreamExt: Stream {
    fn into_box(self) -> BoxStream<Self::Item, Self::Error>
        where Self: Sized + Send + 'static
    {
        Box::new(self)
    }

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