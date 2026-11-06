//! Admission by machine load: start another child run only while the machine has room for it.
//!
//! [`Headroom::acquire`] hands out a [`Permit`] per run. Below [`Limits::floor`] runs a permit is granted at once, at
//! [`Limits::ceiling`] never. In between, each load sample says how many runs still fit ([`room`]): as many as the
//! CPU below [`Limits::cpu`] holds at [`Limits::per_permit_cpu`] each, and the memory below [`Limits::mem`] of
//! physical memory holds at [`Limits::per_permit_bytes`] each, none under memory pressure. That many start at once.
//! The next sample counts only when it was taken entirely after the last grant and [`Limits::settle`] passed, so the
//! new runs show in the load before more start.

use std::sync::{Condvar, Mutex, PoisonError};
use std::time::{Duration, Instant};

use nxb_sys::system::{self, CpuTicks};

/// How far [`Headroom`] lets the machine fill up.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Limits {
    /// CPU busy share (0 to 1) above which no run starts.
    pub cpu: f64,
    /// Share of physical memory (0 to 1) that memory in use plus a new run must stay below.
    pub mem: f64,
    /// Runs granted regardless of load.
    pub floor: usize,
    /// Runs never exceeded.
    pub ceiling: usize,
    /// Minimum time between two grants above the floor.
    pub settle: Duration,
    /// Memory a new run is expected to need at its peak.
    pub per_permit_bytes: u64,
    /// Share of the machine's CPU (0 to 1) one run keeps busy.
    pub per_permit_cpu: f64,
    /// How often the load is read at most.
    pub sample_every: Duration,
}

impl Limits {
    /// 90 % CPU and memory, a sample a second, two seconds after a grant before the next sample counts, one logical
    /// core per run.
    pub fn new(floor: usize, ceiling: usize, per_permit_bytes: u64) -> Limits {
        let cores = std::thread::available_parallelism().map_or(8, std::num::NonZero::get);
        // core counts are small, the conversion is exact
        #[allow(clippy::cast_precision_loss)]
        let per_permit_cpu = 1.0 / cores as f64;
        Limits {
            cpu: 0.9,
            mem: 0.9,
            floor: floor.max(1),
            ceiling: ceiling.max(floor.max(1)),
            settle: Duration::from_secs(2),
            per_permit_bytes,
            per_permit_cpu,
            sample_every: Duration::from_secs(1),
        }
    }
}

/// One reading of the machine's load. `None` fields are unknown.
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub struct Sample {
    /// CPU busy share since the previous sample, 0 to 1.
    pub cpu: Option<f64>,
    /// Memory in use in bytes.
    pub mem_used: Option<u64>,
    /// Physical memory in bytes.
    pub mem_total: Option<u64>,
    /// Kernel memory pressure: 1 normal, 2 warning, 4 critical.
    pub pressure: Option<u32>,
}

/// A source of [`Sample`]s. The CPU share covers the time since the previous call.
pub trait Gauge: Send {
    /// Read the load now.
    fn sample(&mut self) -> Sample;
}

/// The machine's real load through `nxb-sys`. The kernel's CPU counters move in bursts about once a second: a reading
/// without new ticks has no CPU share and keeps the window open.
#[derive(Debug, Default)]
pub struct SystemGauge {
    last: Option<CpuTicks>,
}

impl Gauge for SystemGauge {
    fn sample(&mut self) -> Sample {
        let now = system::cpu_ticks();
        let cpu = match (now, self.last) {
            (Some(n), Some(l)) => n.busy_since(&l),
            _ => None,
        };
        if cpu.is_some() || self.last.is_none() {
            self.last = now;
        }
        Sample {
            cpu,
            mem_used: system::memory_used(),
            mem_total: system::memsize(),
            pressure: system::memory_pressure(),
        }
    }
}

/// How many more runs fit by sample `s` with `running` runs holding permits: at most up to the ceiling, none when a
/// load figure is unknown or the kernel reports memory pressure. The floor is not counted here.
pub fn room(l: &Limits, running: usize, s: &Sample) -> usize {
    let (Some(cpu), Some(used), Some(total)) = (s.cpu, s.mem_used, s.mem_total) else {
        return 0;
    };
    if s.pressure.is_some_and(|p| p > 1) {
        return 0;
    }
    let left = l.ceiling.saturating_sub(running);
    // shares and byte counts far below 2^52: the conversions are exact enough, the results small and non-negative
    #[allow(
        clippy::cast_precision_loss,
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss
    )]
    let fit = |free: f64, each: f64| -> usize {
        if free <= 0.0 {
            0
        } else if each <= 0.0 {
            left
        } else {
            (free / each).floor().min(left as f64) as usize
        }
    };
    #[allow(clippy::cast_precision_loss)]
    let mem = fit(
        l.mem * total as f64 - used as f64,
        l.per_permit_bytes as f64,
    );
    fit(l.cpu - cpu, l.per_permit_cpu).min(mem)
}

struct State {
    gauge: Box<dyn Gauge>,
    running: usize,
    peak: usize,
    last_grant: Option<Instant>,
    /// When the latest sample was taken.
    taken: Instant,
    /// Runs the latest sample still lets start above the floor.
    room: usize,
}

/// Admission of child runs by machine load, see the module docs.
pub struct Headroom {
    limits: Limits,
    state: Mutex<State>,
    cv: Condvar,
}

/// A granted run. Dropping it frees the slot.
pub struct Permit<'a> {
    h: &'a Headroom,
}

impl Drop for Permit<'_> {
    fn drop(&mut self) {
        let mut st = self.h.state.lock().unwrap_or_else(PoisonError::into_inner);
        st.running = st.running.saturating_sub(1);
        drop(st);
        self.h.cv.notify_all();
    }
}

impl Headroom {
    /// Admission by the machine's real load.
    pub fn new(limits: Limits) -> Headroom {
        Headroom::with_gauge(limits, Box::new(SystemGauge::default()))
    }

    /// Admission by the load `gauge` reports.
    pub fn with_gauge(limits: Limits, mut gauge: Box<dyn Gauge>) -> Headroom {
        // primes the CPU counters, the first sample with a window comes in `acquire`
        let _ = gauge.sample();
        let now = Instant::now();
        Headroom {
            limits,
            state: Mutex::new(State {
                gauge,
                running: 0,
                peak: 0,
                last_grant: None,
                taken: now,
                room: 0,
            }),
            cv: Condvar::new(),
        }
    }

    /// The limits in use.
    pub fn limits(&self) -> &Limits {
        &self.limits
    }

    /// The most runs that held a permit at once.
    pub fn peak(&self) -> usize {
        self.state
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .peak
    }

    /// Wait until one more run fits, then hold its slot until the permit is dropped.
    pub fn acquire(&self) -> Permit<'_> {
        let l = &self.limits;
        let mut st = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        loop {
            let now = Instant::now();
            if st.running >= l.floor && st.room == 0 && now - st.taken >= l.sample_every {
                let s = st.gauge.sample();
                // the window (previous sample to now) must lie after the last grant, or the new runs do not show yet
                let settled = st
                    .last_grant
                    .is_none_or(|g| st.taken >= g && now - g >= l.settle);
                // a reading without CPU ticks keeps the window open, the next one covers it
                if s.cpu.is_some() {
                    st.taken = now;
                }
                st.room = if settled { room(l, st.running, &s) } else { 0 };
            }
            let above = st.running >= l.floor;
            if st.running < l.ceiling && (!above || st.room > 0) {
                if above {
                    st.room -= 1;
                }
                st.running += 1;
                st.peak = st.peak.max(st.running);
                st.last_grant = Some(now);
                return Permit { h: self };
            }
            st = self
                .cv
                .wait_timeout(st, l.sample_every / 2)
                .unwrap_or_else(PoisonError::into_inner)
                .0;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    const GIB: u64 = 1 << 30;

    fn idle() -> Sample {
        Sample {
            cpu: Some(0.3),
            mem_used: Some(8 * GIB),
            mem_total: Some(32 * GIB),
            pressure: Some(1),
        }
    }

    fn limits() -> Limits {
        Limits {
            per_permit_cpu: 0.1,
            ..Limits::new(2, 8, 4 * GIB)
        }
    }

    #[test]
    fn room_by_cpu_and_memory() {
        // CPU: (0.9 − 0.3) / 0.1 = 6. Memory: (28.8 − 8) / 4 = 5.2
        assert_eq!(room(&limits(), 2, &idle()), 5);
        let lean = Limits {
            per_permit_bytes: GIB,
            ..limits()
        };
        assert_eq!(room(&lean, 2, &idle()), 6);
        assert_eq!(room(&lean, 5, &idle()), 3);
        assert_eq!(room(&lean, 8, &idle()), 0);
        assert_eq!(
            room(
                &lean,
                2,
                &Sample {
                    cpu: Some(0.85),
                    ..idle()
                }
            ),
            0
        );
        assert_eq!(
            room(
                &lean,
                2,
                &Sample {
                    cpu: Some(0.95),
                    ..idle()
                }
            ),
            0
        );
        // 26 GiB used, 2.8 GiB left below 90 %
        assert_eq!(
            room(
                &limits(),
                2,
                &Sample {
                    mem_used: Some(26 * GIB),
                    ..idle()
                }
            ),
            0
        );
        assert_eq!(
            room(
                &lean,
                2,
                &Sample {
                    mem_used: Some(26 * GIB),
                    ..idle()
                }
            ),
            2
        );
    }

    #[test]
    fn no_room_under_pressure_or_unknown_load() {
        assert_eq!(
            room(
                &limits(),
                2,
                &Sample {
                    pressure: Some(2),
                    ..idle()
                }
            ),
            0
        );
        assert_eq!(
            room(
                &limits(),
                2,
                &Sample {
                    pressure: None,
                    ..idle()
                }
            ),
            5
        );
        assert_eq!(room(&limits(), 2, &Sample::default()), 0);
        assert_eq!(
            room(
                &limits(),
                2,
                &Sample {
                    cpu: None,
                    ..idle()
                }
            ),
            0
        );
    }

    struct Fixed(Sample);
    impl Gauge for Fixed {
        fn sample(&mut self) -> Sample {
            self.0
        }
    }

    fn fast(floor: usize, ceiling: usize) -> Limits {
        Limits {
            settle: Duration::from_millis(1),
            sample_every: Duration::from_millis(5),
            per_permit_cpu: 0.05,
            ..Limits::new(floor, ceiling, GIB)
        }
    }

    fn run_many(h: &Headroom, n: usize, hold: Duration) -> usize {
        let now = AtomicUsize::new(0);
        let peak = AtomicUsize::new(0);
        std::thread::scope(|s| {
            for _ in 0..n {
                s.spawn(|| {
                    let _p = h.acquire();
                    let c = now.fetch_add(1, Ordering::SeqCst) + 1;
                    peak.fetch_max(c, Ordering::SeqCst);
                    std::thread::sleep(hold);
                    now.fetch_sub(1, Ordering::SeqCst);
                });
            }
        });
        peak.load(Ordering::SeqCst)
    }

    #[test]
    fn grows_to_the_ceiling_when_the_machine_is_idle() {
        let h = Headroom::with_gauge(fast(1, 4), Box::new(Fixed(idle())));
        let peak = run_many(&h, 8, Duration::from_millis(300));
        assert_eq!(peak, 4);
        assert_eq!(h.peak(), 4);
    }

    #[test]
    fn stays_at_the_floor_when_the_machine_is_busy() {
        let busy = Sample {
            cpu: Some(0.99),
            ..idle()
        };
        let h = Headroom::with_gauge(fast(2, 8), Box::new(Fixed(busy)));
        assert_eq!(run_many(&h, 6, Duration::from_millis(100)), 2);
    }

    #[test]
    fn a_released_permit_lets_a_waiter_in() {
        let h = Arc::new(Headroom::with_gauge(fast(1, 1), Box::new(Fixed(idle()))));
        let p = h.acquire();
        let h2 = Arc::clone(&h);
        let t = std::thread::spawn(move || {
            let _q = h2.acquire();
        });
        std::thread::sleep(Duration::from_millis(50));
        assert!(!t.is_finished());
        drop(p);
        t.join().unwrap();
    }

    #[test]
    fn system_gauge_reads_the_machine() {
        let mut g = SystemGauge::default();
        let _ = g.sample();
        // the counters move about once a second
        let mut s = Sample::default();
        for _ in 0..30 {
            std::thread::sleep(Duration::from_millis(100));
            s = g.sample();
            if s.cpu.is_some() {
                break;
            }
        }
        let raw = (
            nxb_sys::system::cpu_ticks(),
            nxb_sys::system::memory_used(),
            nxb_sys::system::memsize(),
            nxb_sys::system::memory_pressure(),
        );
        if cfg!(target_os = "macos") {
            assert!(
                s.cpu.is_some() && s.mem_used.is_some() && s.mem_total.is_some(),
                "{s:?}, raw {raw:?}"
            );
        }
    }
}
