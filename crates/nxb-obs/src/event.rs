//! The event model and its NDJSON encoding.

use crate::json::escape_into;
use nxb_error::ErrorClass;
use std::fmt::Write as _;

/// A value in a summary event.
#[derive(Clone, PartialEq, Debug)]
pub enum Value {
    /// A string.
    Str(String),
    /// An unsigned integer.
    U64(u64),
    /// A signed integer.
    I64(i64),
    /// A float, non-finite values encode as `null`.
    F64(f64),
    /// A boolean.
    Bool(bool),
}

/// One progress event (design-v2 13.1).
#[derive(Clone, PartialEq, Debug)]
#[non_exhaustive]
pub enum Event {
    /// A phase starts: `{"ev":"phase","name":…}`.
    Phase {
        /// Phase name, for example `shaders`.
        name: String,
    },
    /// Work done so far: `{"ev":"progress","step":…,"done":…,"total":…,"unit":…}`.
    Progress {
        /// The step, for example `Mac shaders`.
        step: String,
        /// Units done.
        done: u64,
        /// Units in total (0 when unknown).
        total: u64,
        /// The unit, for example `shaders`.
        unit: String,
    },
    /// One shader finished (debug level): `{"ev":"shader","key":…,"status":…,"ms":…}`.
    Shader {
        /// Shader key.
        key: String,
        /// `ok`, `hit`, or the reason string.
        status: String,
        /// Wall milliseconds.
        ms: u64,
    },
    /// An error: `{"ev":"error","class":…,"message":…}`.
    Error {
        /// The class.
        class: ErrorClass,
        /// The message (a `[RED]` text without the prefix, or a shader reason).
        message: String,
    },
    /// The run's summary: `{"ev":"summary", <fields in order>}`.
    Summary {
        /// Ordered fields. Names `ev` and duplicates are the caller's responsibility to avoid.
        fields: Vec<(String, Value)>,
    },
}

impl Event {
    /// The `ev` tag.
    pub fn kind(&self) -> &'static str {
        match self {
            Event::Phase { .. } => "phase",
            Event::Progress { .. } => "progress",
            Event::Shader { .. } => "shader",
            Event::Error { .. } => "error",
            Event::Summary { .. } => "summary",
        }
    }

    /// One line of compact JSON without the trailing newline.
    ///
    /// ```
    /// use nxb_obs::Event;
    /// let e = Event::Progress { step: "Mac shaders".into(), done: 3, total: 10, unit: "shaders".into() };
    /// assert_eq!(e.to_ndjson(), r#"{"ev":"progress","step":"Mac shaders","done":3,"total":10,"unit":"shaders"}"#);
    /// ```
    pub fn to_ndjson(&self) -> String {
        let mut o = String::with_capacity(96);
        o.push_str("{\"ev\":\"");
        o.push_str(self.kind());
        o.push('"');
        match self {
            Event::Phase { name } => str_field(&mut o, "name", name),
            Event::Progress {
                step,
                done,
                total,
                unit,
            } => {
                str_field(&mut o, "step", step);
                num_field(&mut o, "done", *done);
                num_field(&mut o, "total", *total);
                str_field(&mut o, "unit", unit);
            }
            Event::Shader { key, status, ms } => {
                str_field(&mut o, "key", key);
                str_field(&mut o, "status", status);
                num_field(&mut o, "ms", *ms);
            }
            Event::Error { class, message } => {
                str_field(&mut o, "class", class.name());
                str_field(&mut o, "message", message);
            }
            Event::Summary { fields } => {
                for (name, v) in fields {
                    o.push(',');
                    escape_into(&mut o, name);
                    o.push(':');
                    value(&mut o, v);
                }
            }
        }
        o.push('}');
        o
    }
}

fn str_field(o: &mut String, name: &str, v: &str) {
    o.push_str(",\"");
    o.push_str(name);
    o.push_str("\":");
    escape_into(o, v);
}

fn num_field(o: &mut String, name: &str, v: u64) {
    let _ = write!(o, ",\"{name}\":{v}");
}

fn value(o: &mut String, v: &Value) {
    match v {
        Value::Str(s) => escape_into(o, s),
        Value::U64(n) => {
            let _ = write!(o, "{n}");
        }
        Value::I64(n) => {
            let _ = write!(o, "{n}");
        }
        Value::F64(f) if f.is_finite() => {
            // `{:?}` keeps a decimal point for integral values (1.0), like Python's float repr.
            let _ = write!(o, "{f:?}");
        }
        Value::F64(_) => o.push_str("null"),
        Value::Bool(b) => o.push_str(if *b { "true" } else { "false" }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodings() {
        assert_eq!(
            Event::Phase {
                name: "shaders".into()
            }
            .to_ndjson(),
            r#"{"ev":"phase","name":"shaders"}"#
        );
        assert_eq!(
            Event::Error {
                class: ErrorClass::Env,
                message: "the disk is full: /x".into()
            }
            .to_ndjson(),
            r#"{"ev":"error","class":"env","message":"the disk is full: /x"}"#
        );
        assert_eq!(
            Event::Summary {
                fields: vec![
                    ("ok".into(), Value::U64(5)),
                    ("rate".into(), Value::F64(1.0)),
                    ("nan".into(), Value::F64(f64::NAN)),
                    ("delta".into(), Value::I64(-2)),
                    ("done".into(), Value::Bool(true)),
                    ("who".into(), Value::Str("a\"b".into())),
                ]
            }
            .to_ndjson(),
            r#"{"ev":"summary","ok":5,"rate":1.0,"nan":null,"delta":-2,"done":true,"who":"a\"b"}"#
        );
    }

    #[test]
    fn shader_event_single_line() {
        let e = Event::Shader {
            key: "0123456789abcdef".into(),
            status: "metal failed\nline two".into(),
            ms: 12,
        };
        let line = e.to_ndjson();
        assert!(!line.contains('\n'));
        assert!(line.ends_with(",\"ms\":12}"));
    }
}
