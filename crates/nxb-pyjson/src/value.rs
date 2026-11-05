//! The ordered JSON value.

/// A JSON value. Objects keep insertion order, as Python dicts do.
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    /// `null` / `None`.
    Null,
    /// `true` / `false`.
    Bool(bool),
    /// An integer (Python `int` in the range of `i64`).
    Int(i64),
    /// A float (Python `float`).
    Float(f64),
    /// A string.
    Str(String),
    /// A list.
    List(Vec<Value>),
    /// An object in insertion order.
    Object(Vec<(String, Value)>),
}

impl Value {
    /// The member `k` of an object.
    pub fn get(&self, k: &str) -> Option<&Value> {
        match self {
            Value::Object(m) => m.iter().find(|(n, _)| n == k).map(|(_, v)| v),
            _ => None,
        }
    }

    /// Set or replace member `k` (a new key goes last, as `dict[k] = v`).
    pub fn set(&mut self, k: &str, v: Value) {
        if let Value::Object(m) = self {
            match m.iter_mut().find(|(n, _)| n == k) {
                Some(slot) => slot.1 = v,
                None => m.push((k.to_string(), v)),
            }
        }
    }

    /// Remove member `k` (as `dict.pop(k, None)`).
    pub fn remove(&mut self, k: &str) -> Option<Value> {
        if let Value::Object(m) = self {
            let i = m.iter().position(|(n, _)| n == k)?;
            return Some(m.remove(i).1);
        }
        None
    }

    /// The string, if this is one.
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::Str(s) => Some(s),
            _ => None,
        }
    }

    /// The integer, if this is one.
    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Value::Int(i) => Some(*i),
            _ => None,
        }
    }

    /// The list, if this is one.
    pub fn as_list(&self) -> Option<&[Value]> {
        match self {
            Value::List(l) => Some(l),
            _ => None,
        }
    }

    /// A string value.
    pub fn str(s: impl Into<String>) -> Value {
        Value::Str(s.into())
    }

    /// An object from pairs.
    pub fn obj<const N: usize>(pairs: [(&str, Value); N]) -> Value {
        Value::Object(pairs.into_iter().map(|(k, v)| (k.to_string(), v)).collect())
    }
}
