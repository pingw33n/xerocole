use futures::prelude::*;

pub trait FutureExt: Future {
    fn inspect_err<F>(self, f: F) -> InspectErr<Self, F>
            where F: FnMut(&Self::Error),
                  Self: Sized {
        InspectErr {
            future: self,
            f,
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