pub mod future;
pub mod signal;
pub mod stream;

pub use self::future::FutureExt;
pub use self::signal::pulse;
pub use self::stream::StreamExt;

pub type BoxFuture<T, E> = Box<futures::Future<Item=T, Error=E> + Send>;
pub type BoxStream<T, E> = Box<futures::Stream<Item=T, Error=E> + Send>;