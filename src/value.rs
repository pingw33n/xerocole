use std::collections::HashMap;
use std::ops::{Deref, DerefMut, Range};

pub type List = Vec<Spanned<Value>>;
pub type Map = HashMap<String, Spanned<Value>>;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Spanned<T> {
    pub value: T,
    pub span: Range<u32>,
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
    pub fn as_str(&self) -> Result<&str, ValueError> {
        self.as_string().map(|s| s.as_str())
    }

    pub fn get_opt(&self, key: &str) -> Result<Option<&Spanned<Value>>, ValueError> {
        Ok(self.as_map()?.get(key))
    }

    pub fn get_opt_str(&self, key: &str) -> Result<Option<&str>, ValueError> {
        Ok(self.get_opt_string(key)?.map(|s| s.as_str()))
    }

    pub fn get(&self, key: &str) -> Result<&Spanned<Value>, ValueError> {
        match self.get_opt(key)? {
            Some(v) => Ok(v),
            None => Err(ValueError {
                msg: format!("Map must specify required key `{}`", key),
                span: self.span.clone(),
            }),
        }
    }

    pub fn remove_opt(&mut self, key: &str) -> Result<Option<Spanned<Value>>, ValueError> {
        Ok(self.as_map_mut()?.remove(key))
    }

    pub fn remove(&mut self, key: &str) -> Result<Spanned<Value>, ValueError> {
        match self.remove_opt(key)? {
            Some(v) => Ok(v),
            None => Err(ValueError {
                msg: format!("Map must specify required key `{}`", key),
                span: self.span.clone(),
            }),
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
    pub fn get_opt(&self, key: &str) -> Result<Option<&Spanned<Value>>, ValueError> {
        Ok(self.as_map()?.get(key))
    }

    pub fn get_opt_str(&self, key: &str) -> Result<Option<&str>, ValueError> {
        Ok(self.get_opt_string(key)?.map(|s| s.as_str()))
    }

    pub fn get(&self, key: &str) -> Result<&Spanned<Value>, ValueError> {
        match self.get_opt(key)? {
            Some(v) => Ok(v),
            None => Err(ValueError {
                msg: format!("Map must specify required key `{}`", key),
                span: 0..0,
            }),
        }
    }
}

#[derive(Debug)]
pub struct ValueError {
    pub msg: String,
    pub span: Range<u32>,
}

impl ValueError {
    pub fn at(self, span: Range<u32>) -> Self {
        Self {
            msg: self.msg,
            span,
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

    pub fn as_str(&self) -> Result<&str, ValueError> {
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
            pub fn $into_vari(self) -> Result<$ty, ValueError> {
                let kind = self.kind();
                if let $val :: $vari ( v ) = self {
                    Ok(v)
                } else {
                    Err(ValueError {
                        msg: format!(concat!(stringify!($vari), " value expected but {:?} found"), kind),
                        span: 0..0,
                    })
                }
            }

            pub fn $as_vari(&self) -> Result<& $ty, ValueError> {
                if let $val :: $vari ( v ) = self {
                    Ok(v)
                } else {
                    Err(ValueError {
                        msg: format!(concat!(stringify!($vari), " value expected but {:?} found"), self.kind()),
                        span: 0..0,
                    })
                }
            }

            pub fn $as_vari_mut(&mut self) -> Result<&mut $ty, ValueError> {
                if let $val :: $vari ( v ) = self {
                    Ok(v)
                } else {
                    Err(ValueError {
                        msg: format!(concat!(stringify!($vari), " value expected but {:?} found"), self.kind()),
                        span: 0..0,
                    })
                }
            }

            pub fn $get_opt_vari(&self, key: &str) -> Result<Option<& $ty>, ValueError> {
                Ok(match self.get_opt(key)? {
                    Some(v) => Some(v.$as_vari()?),
                    None => None,
                })

            }
        }

        impl Spanned<$val> {
            pub fn $into_vari(self) -> Result<$ty, ValueError> {
                let span = self.span;
                self.value.$into_vari().map_err(move |e| e.at(span))
            }

            pub fn $as_vari(&self) -> Result<& $ty, ValueError> {
                self.value.$as_vari().map_err(move |e| e.at(self.span.clone()))
            }

            pub fn $as_vari_mut(&mut self) -> Result<&mut $ty, ValueError> {
                match self.value.$as_vari_mut() {
                    Ok(v) => Ok(v),
                    Err(e) => Err(e.at(self.span.clone())),
                }
            }

            pub fn $get_opt_vari(&self, key: &str) -> Result<Option<& $ty>, ValueError> {
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
