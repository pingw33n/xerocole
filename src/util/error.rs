use backtrace::Backtrace;
use std::any::{Any, TypeId};
use std::borrow::Cow;
use std::fmt;
use std::cell::RefCell;

pub trait Object: 'static + Any + fmt::Debug + fmt::Display + Send {
    #[doc(hidden)]
    fn type_id(&self) -> TypeId {
        TypeId::of::<Self>()
    }
}

impl dyn Object {
    pub fn is<T: Object + 'static>(&self) -> bool {
        let t = TypeId::of::<T>();
        let boxed = self.type_id();
        t == boxed
    }

    pub fn downcast<T: Object + 'static>(self: Box<Self>) -> Result<Box<T>, Box<dyn Object>> {
        if self.is::<T>() {
            unsafe {
                let raw: *mut dyn Object = Box::into_raw(self);
                Ok(Box::from_raw(raw as *mut T))
            }
        } else {
            Err(self)
        }
    }

    pub fn downcast_ref<T: Object + 'static>(&self) -> Option<&T> {
        if self.is::<T>() {
            unsafe {
                Some(&*(self as *const dyn Object as *const T))
            }
        } else {
            None
        }
    }

    pub fn downcast_mut<T: Object + 'static>(&mut self) -> Option<&mut T> {
        if self.is::<T>() {
            unsafe {
                Some(&mut *(self as *mut dyn Object as *mut T))
            }
        } else {
            None
        }
    }
}

impl<T: 'static + Any + fmt::Debug + fmt::Display + Send> Object for T {}

/// Generic chained error type.
///
/// Consists of:
/// - Error ID - a lightweight type, likely a `Copy`able and `Eq`able enum, that can be used to
///   easily match the error kind.
/// - Optional details. This object provides context to the error and is written next
///   to the id when error is displayed.
/// - Optional cause. This object is assumed to be the logical cause of this error. Written below
///   the the error ID + details when error is displayed.
/// - Backtrace of the location where error was first created. Shown only when debug format is
///   requested.
/// - Arbitrary context stack. When error is propagated upwards different layers can push
///   context messages onto the context stack. This allows additional information to be stored
///   with the message for even better of understanding of the error reason and context.
pub struct Error<T>(Box<Inner<T>>);

impl<T> Error<T> {
    pub fn new(id: impl Into<T>, details: impl Object) -> Self {
        Self::new0(Inner::new(id, details))
    }

    pub fn without_details(id: impl Into<T>) -> Self {
        Self::new0(Inner::without_details(id))
    }

    pub fn with_cause(self, cause: impl Object) -> Self {
        Self::new0(self.0.with_cause(cause))
    }

    pub fn with_context(self, message: impl Into<Cow<'static, str>>) -> Self {
        Self::new0(self.0.with_context(message))
    }

    pub fn id(&self) -> &T {
        self.0.id()
    }

    pub fn details(&self) -> Option<&Box<Object>> {
        self.0.details()
    }

    pub fn map_details<F, R>(self, f: F) -> Self
        where F: FnOnce(Box<Object>) -> R,
              R: Object,
    {
        Self::new0(self.0.map_details(f))
    }

    fn new0(inner: Inner<T>) -> Self {
        Self(Box::new(inner))
    }
}

impl<T: fmt::Debug + fmt::Display> fmt::Debug for Error<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(&self.0, f)
    }
}

impl<T: fmt::Debug + fmt::Display> fmt::Display for Error<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}

struct Inner<T> {
    id: T,
    details: Option<Box<dyn Object>>,
    cause: Option<Box<dyn Object>>,
    backtrace: RefCell<Backtrace>,
    context: Option<Context>,
}

impl<T> Inner<T> {
    pub fn new(id: impl Into<T>, details: impl Object) -> Self {
        Self::new0(id, Some(Box::new(details)), None)
    }

    pub fn without_details(id: impl Into<T>) -> Self {
        Self::new0(id, None, None)
    }

    pub fn with_cause(self, cause: impl Object) -> Self {
        Self::new0(self.id, self.details, Some(Box::new(cause)))
    }

    pub fn with_context(mut self, message: impl Into<Cow<'static, str>>) -> Self {
        let new_ctx = Context {
            message: message.into(),
            next: None,
        };
        if self.context.is_some() {
            let mut ctx = self.context.as_mut();
            while let Some(m) = ctx {
                if m.next.is_none() {
                    m.next = Some(Box::new(new_ctx));
                    break;
                }
                ctx = m.next.as_mut().map(|v| v.as_mut());
            }
        } else {
            self.context = Some(new_ctx);
        }
        self
    }

    pub fn id(&self) -> &T {
        &self.id
    }

    pub fn details(&self) -> Option<&Box<Object>> {
        self.details.as_ref()
    }

    pub fn map_details<F, R>(mut self, f: F) -> Self
        where F: FnOnce(Box<Object>) -> R,
              R: Object,
    {
        let details = self.details.take().map(|v| Box::new(f(v)) as Box<Object>);
        self.details = details;
        self
    }

    fn new0(id: impl Into<T>, details: Option<Box<Object>>,
            cause: Option<Box<Object>>) -> Self {
        Self {
            id: id.into(),
            details,
            cause,
            backtrace: RefCell::new(Backtrace::new_unresolved()),
            context: None,
        }
    }

    fn print_backtrace(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut bt = self.backtrace.borrow_mut();
        bt.resolve();
        write!(f, "{:?}", bt)
    }
}

impl<T: fmt::Debug + fmt::Display> fmt::Debug for Inner<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(self, f)?;
        writeln!(f)?;
        self.print_backtrace(f)
    }
}

impl<T: fmt::Debug + fmt::Display> fmt::Display for Inner<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.id)?;
        if let Some(details) = self.details.as_ref() {
            write!(f, ": {}", details)?;
        }
        write!(f, " ({:?})", self.id)?;

        if let Some(cause) = self.cause.as_ref() {
            writeln!(f)?;
            write!(f, "   => caused by: {}", cause)?;
        }

        let mut ctx = self.context.as_ref();
        while let Some(c) = ctx {
            writeln!(f)?;
            write!(f, "   ...while {}", c.message)?;
            ctx = c.next.as_ref().map(|v| v.as_ref());
        }

        Ok(())
    }
}

struct Context {
    message: Cow<'static, str>,
    next: Option<Box<Self>>,
}

pub trait ErrorExt: Object {
    fn wrap_err<IdIn, IdOut, D>(self, id: IdIn, details: D) -> Error<IdOut>
        where IdOut: fmt::Debug + fmt::Display,
              IdIn: Into<IdOut>,
              D: Object,
              Self: 'static + Sized + Send,
    {
        self.wrap_with(|_| (id, details))
    }

    fn wrap_with<IdIn, IdOut, D, F>(self, f: F) -> Error<IdOut>
        where IdOut: fmt::Debug + fmt::Display,
              IdIn: Into<IdOut>,
              D: Object,
              F: FnOnce(&Self) -> (IdIn, D),
              Self: 'static + Sized + Send,
    {
        let (id, details) = f(&self);
        Error::new(id, details).with_cause(self)
    }

    fn wrap_id<IdIn, IdOut>(self, id: IdIn) -> Error<IdOut>
        where IdOut: fmt::Debug + fmt::Display,
              IdIn: Into<IdOut>,
              Self: 'static + Sized + Send,
    {
         Error::without_details(id).with_cause(self)
    }
}

impl<T: Object> ErrorExt for T {}

pub trait ResultExt<T, E> {
    fn wrap_err<IdIn, IdOut, D>(self, id: IdIn, details: D) -> Result<T, Error<IdOut>>
        where IdOut: fmt::Debug + fmt::Display,
              IdIn: Into<IdOut>,
              D: Object,
              Self: Sized,
    {
        self.wrap_err_with(|_| (id, details))
    }

    fn wrap_err_with<IdIn, IdOut, D, F>(self, f: F) -> Result<T, Error<IdOut>>
            where IdOut: fmt::Debug + fmt::Display,
                  IdIn: Into<IdOut>,
                  D: Object,
                  F: FnOnce(&E) -> (IdIn, D);

    fn wrap_err_id<IdIn, IdOut>(self, id: IdIn) -> Result<T, Error<IdOut>>
        where IdOut: fmt::Debug + fmt::Display,
              IdIn: Into<IdOut>;
}

impl<T, E: Object> ResultExt<T, E> for Result<T, E> {
    fn wrap_err_with<IdIn, IdOut, D, F>(self, f: F) -> Result<T, Error<IdOut>>
            where IdOut: fmt::Debug + fmt::Display,
                  IdIn: Into<IdOut>,
                  D: Object,
                  F: FnOnce(&E) -> (IdIn, D) {
        self.map_err(move |cause| cause.wrap_with(f))
    }

    fn wrap_err_id<IdIn, IdOut>(self, id: IdIn) -> Result<T, Error<IdOut>>
        where IdOut: fmt::Debug + fmt::Display,
              IdIn: Into<IdOut>,
    {
        self.map_err(move |cause| cause.wrap_id(id))
    }
}

pub trait ResultErrorExt<T> {
    fn context(self, msg: impl Into<Cow<'static, str>>) -> Self
            where Self: Sized
    {
        self.context_with(|_| msg)
    }

    fn context_with<R, F>(self, f: F) -> Self
    where
        Self: Sized,
        F: FnOnce(&Error<T>) -> R,
        R: Into<Cow<'static, str>>;
}

impl<T, Id> ResultErrorExt<Id> for Result<T, Error<Id>> {
    fn context_with<R, F>(self, f: F) -> Self
    where
        F: FnOnce(&Error<Id>) -> R,
        R: Into<Cow<'static, str>>
    {
        self.map_err(|e| {
            let msg = f(&e);
            e.with_context(msg)
        })
    }
}