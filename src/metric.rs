use std::collections::HashMap;
use std::fmt;
use std::sync::Mutex;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Number {
    Int(i64),
    Float(f64),
}

impl From<i64> for Number {
    fn from(v: i64) -> Self {
        Number::Int(v)
    }
}

impl From<f64> for Number {
    fn from(v: f64) -> Self {
        Number::Float(v)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Gauge(Number),
    Counter(Number),
    Text(String),
}

pub struct Metrics {
    values: Mutex<HashMap<String, Value>>,
}

impl Metrics {
    pub fn new() -> Self {
        Self {
            values: Mutex::new(HashMap::new()),
        }
    }

    pub fn set(&self, name: String, value: Value) {
        self.values.lock().unwrap().insert(name, value);
    }

    pub fn inc(&self, name: &str, delta: impl Into<Number>) {
        let delta = delta.into();
        match delta {
            Number::Int(n) => assert!(n >= 0),
            Number::Float(n) => assert!(n >= 0.0),
        }
        match self.values.lock().unwrap().get_mut(name).unwrap() {
            Value::Counter(n) => {
                match delta {
                    Number::Int(d) => {
                        match n {
                            Number::Int(i) => *i += d,
                            Number::Float(i) => *i += d as f64,
                        }
                    }
                    Number::Float(d) => {
                        match n {
                            Number::Int(_) => panic!("wrong number variant"),
                            Number::Float(i) => *i += d,
                        }
                    }
                }

            },
            Value::Gauge(_) => panic!("can't inc Gauge"),
            Value::Text(_) => panic!("can't inc Text"),
        }
    }
}

impl fmt::Debug for Metrics {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:#?}", &*self.values.lock().unwrap())
    }
}