use std::borrow::Cow;
use std::collections::HashMap;
use std::fmt;
use std::ops::{Deref, DerefMut, Range};

use crate::error::*;

pub type List = Vec<Spanned<Value>>;
pub type Map = HashMap<String, Spanned<Value>>;
pub type Span = Range<u32>;

type Result<T> = std::result::Result<T, Error>;

#[derive(Debug)]
pub struct ErrorDetails {
    pub msg: Cow<'static, str>,
    pub span: Span,
}

impl ErrorDetails {
    pub fn new(msg: impl Into<Cow<'static, str>>, span: Span) -> Self {
        Self {
            msg: msg.into(),
            span,
        }
    }

    pub fn at(self, span: Span) -> Self {
        Self {
            msg: self.msg,
            span,
        }
    }
}

impl fmt::Display for ErrorDetails {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "[{}..{}] {}", self.span.start, self.span.end, self.msg)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Spanned<T> {
    pub value: T,
    pub span: Span,
}

impl<T> Spanned<T> {
    pub fn new_error(&self, msg: impl Into<Cow<'static, str>>) -> Error {
        Error::new(ErrorId::Parse, ErrorDetails::new(msg, self.span.clone()))
    }
}

impl<T> From<T> for Spanned<T> {
    fn from(value: T) -> Self {
        Self {
            value,
            span: 0..0,
        }
    }
}

impl<T> Deref for Spanned<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

impl<T> DerefMut for Spanned<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.value
    }
}

impl Spanned<Value> {
    pub fn as_str(&self) -> Result<&str> {
        self.as_string().map(|s| s.as_str())
    }

    pub fn get_opt(&self, key: &str) -> Result<Option<&Spanned<Value>>> {
        Ok(self.as_map()?.get(key))
    }

    pub fn get_opt_str(&self, key: &str) -> Result<Option<&str>> {
        Ok(self.get_opt_string(key)?.map(|s| s.as_str()))
    }

    pub fn get(&self, key: &str) -> Result<&Spanned<Value>> {
        match self.get_opt(key)? {
            Some(v) => Ok(v),
            None => Err(ErrorDetails::new(format!("Map must specify required key `{}`", key),
                self.span.clone()).wrap_id(ErrorId::Parse)),
        }
    }

    pub fn remove_opt(&mut self, key: &str) -> Result<Option<Spanned<Value>>> {
        Ok(self.as_map_mut()?.remove(key))
    }

    pub fn remove(&mut self, key: &str) -> Result<Spanned<Value>> {
        match self.remove_opt(key)? {
            Some(v) => Ok(v),
            None => Err(ErrorDetails::new(format!("Map must specify required key `{}`", key),
                self.span.clone()).wrap_id(ErrorId::Parse)),
        }
    }
}

impl<'a> From<&'a str> for Spanned<Value> {
    fn from(v: &'a str) -> Self {
        Self::from(Value::from(v))
    }
}

impl From<Vec<Value>> for Spanned<Value> {
    fn from(v: Vec<Value>) -> Self {
        Self::from(v.into_iter().map(|v| Self::from(v)).collect::<Vec<_>>())
    }
}

impl From<HashMap<String, Value>> for Spanned<Value> {
    fn from(v: HashMap<String, Value>) -> Self {
        Self::from(v.into_iter().map(|(k, v)| (k, Self::from(v))).collect::<HashMap<_, _>>())
    }
}

impl Value {
    pub fn get_opt(&self, key: &str) -> Result<Option<&Spanned<Value>>> {
        Ok(self.as_map()?.get(key))
    }

    pub fn get_opt_str(&self, key: &str) -> Result<Option<&str>> {
        Ok(self.get_opt_string(key)?.map(|s| s.as_str()))
    }

    pub fn get(&self, key: &str) -> Result<&Spanned<Value>> {
        match self.get_opt(key)? {
            Some(v) => Ok(v),
            None => Err(ErrorDetails::new(format!("Map must specify required key `{}`", key),
                0..0).wrap_id(ErrorId::Parse)),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Bool(bool),
    Int(i64),
    Float(f64),
    List(List),
    Map(Map),
    String(String),
}

impl Value {
    pub fn kind(&self) -> ValueKind {
        use self::Value::*;
        match self {
            Bool(_) => ValueKind::Bool,
            Int(_) => ValueKind::Int,
            Float(_) => ValueKind::Float,
            List(_) => ValueKind::List,
            Map(_) => ValueKind::Map,
            String(_) => ValueKind::String,
        }
    }

    pub fn as_str(&self) -> Result<&str> {
        self.as_string().map(|s| s.as_str())
    }
}

impl<'a> From<&'a str> for Value {
    fn from(v: &'a str) -> Self {
        v.to_owned().into()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ValueKind {
    Bool,
    Int,
    Float,
    List,
    Map,
    String,
}

macro_rules! impl_bits {
    ($val:ident { $($vari:ident ( $ty:ty ) :
            $as_vari:ident,
            $as_vari_mut:ident,
            $into_vari:ident,
            $get_opt_vari:ident;)+ }) => {$(
        impl $val {
            pub fn $into_vari(self) -> Result<$ty> {
                if let $val :: $vari ( v ) = self {
                    Ok(v)
                } else {
                    Err(ErrorDetails::new(
                        format!(concat!(stringify!($vari), " value expected but {:?} found"), self.kind()),
                        0..0).wrap_id(ErrorId::Parse))
                }
            }

            pub fn $as_vari(&self) -> Result<& $ty> {
                if let $val :: $vari ( v ) = self {
                    Ok(v)
                } else {
                    Err(ErrorDetails::new(
                        format!(concat!(stringify!($vari), " value expected but {:?} found"), self.kind()),
                        0..0).wrap_id(ErrorId::Parse))
                }
            }

            pub fn $as_vari_mut(&mut self) -> Result<&mut $ty> {
                if let $val :: $vari ( v ) = self {
                    Ok(v)
                } else {
                    Err(ErrorDetails::new(
                        format!(concat!(stringify!($vari), " value expected but {:?} found"), self.kind()),
                        0..0).wrap_id(ErrorId::Parse))
                }
            }

            pub fn $get_opt_vari(&self, key: &str) -> Result<Option<& $ty>> {
                Ok(match self.get_opt(key)? {
                    Some(v) => Some(v.$as_vari()?),
                    None => None,
                })

            }
        }

        impl Spanned<$val> {
            pub fn $into_vari(self) -> Result<$ty> {
                let span = self.span;
                self.value.$into_vari().map_err(move |e|
                    e.map_details(|d| d.downcast::<ErrorDetails>().unwrap().at(span)))
            }

            pub fn $as_vari(&self) -> Result<& $ty> {
                self.value.$as_vari().map_err(move |e|
                    e.map_details(|d| d.downcast::<ErrorDetails>().unwrap().at(self.span.clone())))
            }

            pub fn $as_vari_mut(&mut self) -> Result<&mut $ty> {
                let span = self.span.clone();
                match self.value.$as_vari_mut() {
                    Ok(v) => Ok(v),
                    Err(e) => Err(e.map_details(|d| d.downcast::<ErrorDetails>().unwrap().at(span))),
                }
            }

            pub fn $get_opt_vari(&self, key: &str) -> Result<Option<& $ty>> {
                Ok(match self.get_opt(key)? {
                    Some(v) => Some(v.$as_vari()?),
                    None => None,
                })

            }
        }

        impl From<$ty> for $val {
            fn from(v: $ty) -> Self {
                $val::$vari(v)
            }
        }

        impl From<$ty> for Spanned<$val> {
            fn from(v: $ty) -> Self {
                Spanned::from($val::from(v))
            }
        }
    )+}
}

impl_bits!(Value {
    Bool(bool): as_bool, as_bool_mut, into_bool, get_opt_bool;
    Int(i64): as_int, as_int_mut, into_int, get_opt_int;
    Float(f64): as_float, as_float_mut, into_float, get_opt_float;
    List(List): as_list, as_list_mut, into_list, get_opt_list;
    Map(Map): as_map, as_map_mut, into_map, get_opt_map;
    String(String): as_string, as_string_mut, into_string, get_opt_string;
});
