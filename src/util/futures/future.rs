use futures::prelude::*;
use std::marker::PhantomData;

pub type BoxFuture<T, E> = Box<Future<Item=T, Error=E> + Send + 'static>;

pub trait FutureExt: Future {
    fn into_box(self) -> BoxFuture<Self::Item, Self::Error>
        where Self: Sized + Send + 'static
    {
        Box::new(self)
    }

    fn inspect_err<F>(self, f: F) -> InspectErr<Self, F>
        where F: FnMut(&Self::Error),
              Self: Sized,
    {
        InspectErr {
            future: self,
            f,
        }
    }

    fn infallible<E>(self) -> Infallible<Self, E>
        where Self: Sized,
    {
        Infallible {
            future: self,
            _ty: PhantomData,
        }
    }
}

impl<T: Future> FutureExt for T {
}

pub struct InspectErr<U, F> {
    future: U,
    f: F,
}

impl<U, F> Future for InspectErr<U, F>
        where U: Future,
              F: FnMut(&U::Error) {
    type Item = U::Item;
    type Error = U::Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        self.future.poll()
            .map_err(|e| {
                (self.f)(&e);
                e
            })
    }
}

pub struct Infallible<F, E>{
    future: F,
    _ty: PhantomData<E>,
}

impl<F: Future, E> Future for Infallible<F, E> {
    type Item = F::Item;
    type Error = E;

    fn poll(&mut self) -> Poll<F::Item, Self::Error> {
        self.future.poll().map_err(|_| -> E {
            panic!("infallible future failed");
        })
    }
}

pub struct Blocking<F> {
    f: F,
}

impl<F, R> Future for Blocking<F>
    where F: FnMut() -> R
{
    type Item = R;
    type Error = tokio_threadpool::BlockingError;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        tokio_threadpool::blocking(|| (self.f)())
    }
}

pub fn blocking<F, R>(f: F) -> Blocking<F>
    where F: FnMut() -> R
{
    Blocking { f }
}