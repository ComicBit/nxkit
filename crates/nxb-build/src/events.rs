//! The NSP script's console events (`Progress` in `build_ipa_from_nsp.py`): in `--progress json` mode one JSON object
//! per stdout line (`json.dumps(obj, ensure_ascii=False)`), the events `stage`, `step`, `progress`, `stage_end` and
//! `say`, each with `t` (Unix time, 3 decimals) last, the other modes print `[k/n] name` lines.

use std::io::Write;
use std::sync::Mutex;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use nxb_pyjson::Value;

/// Console mode (`--progress`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// `json`.
    Json,
    /// `lines` / `bar` / `auto` (plain lines here).
    Lines,
    /// `off`.
    Off,
}

/// What a progress event says apart from the count: step, total, unit, note.
type Shape = (String, Option<u64>, String, String);

struct Cur {
    k: usize,
    name: String,
    t0: Instant,
    /// The step within the stage and its count (`Progress.set` in Python).
    step: String,
    done: Option<u64>,
    total: Option<u64>,
    unit: String,
    note: String,
    /// (when, done) of the last minute, for the ETA.
    hist: std::collections::VecDeque<(f64, u64)>,
    /// The last progress event: when, and what it said apart from the count.
    jlast: Option<(Instant, Shape)>,
}

/// A count update for [`Progress::set`] (fields left `None` keep their value).
#[derive(Clone, Debug, Default)]
pub struct Update<'a> {
    /// A new step (restarts the count).
    pub step: Option<&'a str>,
    /// Items done.
    pub done: Option<u64>,
    /// Items in all.
    pub total: Option<u64>,
    /// Unit (`shaders`, `B`).
    pub unit: Option<&'a str>,
    /// Shown after the count.
    pub note: Option<&'a str>,
}

const JSON_INTERVAL: f64 = 0.5;

/// The event writer (thread-safe).
pub struct Progress {
    mode: Mode,
    state: Mutex<(usize, usize, Option<Cur>)>,
    out: Mutex<Box<dyn Write + Send>>,
}

fn now() -> f64 {
    let t = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0);
    (t * 1000.0).round() / 1000.0
}

fn clock() -> String {
    let t = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let (_, _, _, h, m, s) = nxb_pack::tz::local(t);
    format!("{h:02}:{m:02}:{s:02}")
}

fn hms(s: f64) -> String {
    let s = s as u64;
    let (h, m) = ((s / 60) / 60, (s / 60) % 60);
    if h > 0 {
        format!("{h}:{m:02}:{:02}", s % 60)
    } else {
        format!("{m}:{:02}", s % 60)
    }
}

impl Progress {
    /// A writer to `out`.
    pub fn new(mode: Mode, out: Box<dyn Write + Send>) -> Progress {
        Progress {
            mode,
            state: Mutex::new((0, 0, None)),
            out: Mutex::new(out),
        }
    }

    fn json(&self, mut fields: Vec<(String, Value)>) {
        fields.push(("t".into(), Value::Float(now())));
        if let Ok(mut o) = self.out.lock() {
            let _ = writeln!(o, "{}", nxb_pyjson::dumps(&Value::Object(fields)));
            let _ = o.flush();
        }
    }

    fn line(&self, s: &str) {
        if let Ok(mut o) = self.out.lock() {
            let _ = writeln!(o, "{s}");
            let _ = o.flush();
        }
    }

    /// `plan(n)`.
    pub fn plan(&self, n: usize) {
        if let Ok(mut g) = self.state.lock() {
            (g.0, g.1) = (0, n);
        }
    }

    /// `stage(name, step)`: ends the current stage, starts the next.
    pub fn stage(&self, name: &str, step: &str) {
        self.end("", true);
        let (k, n) = match self.state.lock() {
            Ok(mut g) => {
                g.0 += 1;
                g.2 = Some(Cur {
                    k: g.0,
                    name: name.into(),
                    t0: Instant::now(),
                    step: step.into(),
                    done: None,
                    total: None,
                    unit: String::new(),
                    note: String::new(),
                    hist: Default::default(),
                    jlast: None,
                });
                (g.0, g.1)
            }
            Err(_) => return,
        };
        match self.mode {
            Mode::Json => self.json(vec![
                ("ev".into(), Value::Str("stage".into())),
                ("k".into(), Value::Int(k as i64)),
                ("n".into(), Value::Int(n as i64)),
                ("name".into(), Value::Str(name.into())),
                ("step".into(), Value::Str(step.into())),
            ]),
            Mode::Lines => self.line(&format!(
                "[{}] [{k}/{n}] {name}{} …",
                clock(),
                if step.is_empty() {
                    String::new()
                } else {
                    format!(" · {step}")
                }
            )),
            Mode::Off => self.line(&format!("[{}] [{k}/{n}] {name}", clock())),
        }
    }

    /// `end(summary, ok)`.
    pub fn end(&self, summary: &str, ok: bool) {
        let cur = match self.state.lock() {
            Ok(mut g) => g.2.take(),
            Err(_) => None,
        };
        let Some(c) = cur else { return };
        let el = c.t0.elapsed().as_secs_f64();
        match self.mode {
            Mode::Json => self.json(vec![
                ("ev".into(), Value::Str("stage_end".into())),
                ("k".into(), Value::Int(c.k as i64)),
                ("name".into(), Value::Str(c.name)),
                ("ok".into(), Value::Bool(ok)),
                ("summary".into(), Value::Str(summary.into())),
                (
                    "elapsed".into(),
                    Value::Float((el * 1000.0).round() / 1000.0),
                ),
            ]),
            _ => {
                let n = self.state.lock().map(|g| g.1).unwrap_or(0);
                self.line(&format!(
                    "[{}] {} [{}/{n}] {} ({}){}",
                    clock(),
                    if ok { "✓" } else { "✗" },
                    c.k,
                    c.name,
                    hms(el),
                    if summary.is_empty() {
                        String::new()
                    } else {
                        format!(": {summary}")
                    }
                ))
            }
        }
    }

    /// `set(...)`: a `step` event when the step changes, then a `progress` event when anything changed (a count change
    /// alone at most every 0.5 s), with Python's fields: `k name step done total unit est note frac eta elapsed t`.
    pub fn set(&self, u: Update<'_>) {
        let Ok(mut g) = self.state.lock() else { return };
        let n_all = g.1;
        let Some(st) = g.2.as_mut() else { return };
        let mut events: Vec<Vec<(String, Value)>> = Vec::new();
        let mut lines = Vec::new();
        if let Some(step) = u.step
            && step != st.step
        {
            st.step = step.into();
            (st.done, st.total, st.unit, st.note) = (None, None, String::new(), String::new());
            st.hist.clear();
            events.push(vec![
                ("ev".into(), Value::Str("step".into())),
                ("k".into(), Value::Int(st.k as i64)),
                ("name".into(), Value::Str(st.name.clone())),
                ("step".into(), Value::Str(step.into())),
            ]);
            lines.push(format!(
                "[{}] [{}/{n_all}] {} · {step} …",
                clock(),
                st.k,
                st.name
            ));
        }
        if let Some(x) = u.unit {
            st.unit = x.into();
        }
        if let Some(x) = u.total {
            st.total = Some(x);
        }
        if let Some(x) = u.note {
            st.note = x.into();
        }
        let wall = now();
        if let Some(d) = u.done {
            st.done = Some(d);
            st.hist.push_back((wall, d));
            while st.hist.len() > 2 && st.hist[1].0 < wall - 60.0 {
                st.hist.pop_front();
            }
        }
        let shape = (st.step.clone(), st.total, st.unit.clone(), st.note.clone());
        let send = match &st.jlast {
            None => true,
            Some((t, prev)) => *prev != shape || t.elapsed().as_secs_f64() >= JSON_INTERVAL,
        };
        if send && self.mode == Mode::Json {
            st.jlast = Some((Instant::now(), shape));
            let opt = |v: Option<u64>| v.map_or(Value::Null, |x| Value::Int(x as i64));
            let frac = match (st.done, st.total) {
                (Some(d), Some(t)) if t > 0 => Value::Float((d as f64 / t as f64).min(0.99)),
                _ => Value::Null,
            };
            let eta = match (st.total, st.hist.front(), st.hist.back(), st.done) {
                (Some(t), Some(a), Some(b), Some(d))
                    if st.hist.len() >= 2 && b.0 - a.0 >= 3.0 && b.1 > a.1 =>
                {
                    Value::Float(t.saturating_sub(d) as f64 * (b.0 - a.0) / (b.1 - a.1) as f64)
                }
                _ => Value::Null,
            };
            let el = st.t0.elapsed().as_secs_f64();
            events.push(vec![
                ("ev".into(), Value::Str("progress".into())),
                ("k".into(), Value::Int(st.k as i64)),
                ("name".into(), Value::Str(st.name.clone())),
                ("step".into(), Value::Str(st.step.clone())),
                ("done".into(), opt(st.done)),
                ("total".into(), opt(st.total)),
                ("unit".into(), Value::Str(st.unit.clone())),
                ("est".into(), Value::Bool(false)),
                ("note".into(), Value::Str(st.note.clone())),
                ("frac".into(), frac),
                ("eta".into(), eta),
                (
                    "elapsed".into(),
                    Value::Float((el * 1000.0).round() / 1000.0),
                ),
            ]);
        }
        drop(g);
        match self.mode {
            Mode::Json => events.into_iter().for_each(|e| self.json(e)),
            Mode::Lines => lines.iter().for_each(|l| self.line(l)),
            Mode::Off => {}
        }
    }

    /// `say(msg)`.
    pub fn say(&self, msg: &str) {
        match self.mode {
            Mode::Json => self.json(vec![
                ("ev".into(), Value::Str("say".into())),
                ("msg".into(), Value::Str(msg.into())),
            ]),
            _ => self.line(&format!("[{}] {msg}", clock())),
        }
    }
}
