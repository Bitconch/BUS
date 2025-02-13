use influx_db_client as influxdb;
use crate::metrics;
use std::env;
use std::sync::atomic::{AtomicUsize, Ordering};
use buffett_timing::timing;

const DEFAULT_METRICS_RATE: usize = 100;

/// define a public struct of Counter
/// with the public fields of name, counts, times, lastlog, lograte
pub struct Counter {
    pub name: &'static str,
    pub counts: AtomicUsize,
    pub times: AtomicUsize,
    pub lastlog: AtomicUsize,
    pub lograte: AtomicUsize,
}

#[macro_export]
/// using the macro_rules! macro to created a macro named new_counter
macro_rules! new_counter {
    ($name:expr, $lograte:expr) => {
        Counter {
            name: $name,
            counts: AtomicUsize::new(0),
            times: AtomicUsize::new(0),
            lastlog: AtomicUsize::new(0),
            lograte: AtomicUsize::new($lograte),
        }
    };
}

#[macro_export]
macro_rules! sub_counter {
    ($name:expr, $count:expr) => {
        unsafe { $name.inc($count) };
    };
}

#[macro_export]
macro_rules! sub_new_counter_info {
    ($name:expr, $count:expr) => {{
        sub_new_counter!($name, $count, Level::Info, 0);
    }};
    ($name:expr, $count:expr, $lograte:expr) => {{
        sub_new_counter!($name, $count, Level::Info, $lograte);
    }};
}

#[macro_export]
macro_rules! sub_new_counter {($name:expr, $count:expr, $level:expr, $lograte:expr) => 
        {{if log_enabled!($level) {static mut INC_NEW_COUNTER: Counter = new_counter!($name, $lograte);
            sub_counter!(INC_NEW_COUNTER, $count);
        }
    }};
}

/// implementing default_log_rate and inc methods on Counter structure
impl Counter {
    /// define the function of default_log_rate,
    /// and its return value type is usize
    fn default_log_rate() -> usize {
        /// fetches the environment variable key of "BITCONCH_DASHBOARD_RATE" from the current process
        /// parse x into the type of usize through closures
        /// if parsing failed, then return the const of DEFAULT_METRICS_RATE
        /// if failed to fetches the environment variable key, then return the const of DEFAULT_METRICS_RATE
        let v = env::var("BITCONCH_DASHBOARD_RATE")
            .map(|x| x.parse().unwrap_or(DEFAULT_METRICS_RATE))
            .unwrap_or(DEFAULT_METRICS_RATE);
        /// if v == 0, then return the const of DEFAULT_METRICS_RATE
        /// otherwise return v
        if v == 0 {
            DEFAULT_METRICS_RATE
        } else {
            v
        }
    }
    /// define the function of inc
    pub fn inc(&mut self, events: usize) {
        /// adds "events" to the current value of "counts", returning the previous value of "counts"
        let counts = self.counts.fetch_add(events, Ordering::Relaxed);
        /// add 1 to the current value of "times", and return the previous value of "times"
        let times = self.times.fetch_add(1, Ordering::Relaxed);
        /// loads the value of "lograte" from the atomic intege
        let mut lograte = self.lograte.load(Ordering::Relaxed);
        /// if lograte == 0
        /// then call the function "default_log_rate" on Counter structure
        /// and stores the value of "lograte" into the atomic integer
        if lograte == 0 {
            lograte = Counter::default_log_rate();
            self.lograte.store(lograte, Ordering::Relaxed);
        }
        /// if times % lograte == 0 and times > 0
        if times % lograte == 0 && times > 0 {
            let lastlog = self.lastlog.load(Ordering::Relaxed);
            /// logs the message at the Info level
            info!(
                "COUNTER:{{\"name\": \"{}\", \"counts\": {}, \"samples\": {},  \"now\": {}, \"events\": {}}}",
                self.name,
                counts + events,
                times,
                timing::timestamp(),
                events,
            );
            /// use the submit method of the metrics crate 
            /// and new a Point named "counter-{}" with theCounter structure field "name" in format,
            /// and a field named "count" whose value is "counts - lastlog" of type i64
            metrics::submit(
                influxdb::Point::new(&format!("counter-{}", self.name))
                    .add_field(
                        "count",
                        influxdb::Value::Integer(counts as i64 - lastlog as i64),
                    ).to_owned(),
            );
            /// stores the value into the atomic integer if the current value is the same as the current value
            self.lastlog
                .compare_and_swap(lastlog, counts, Ordering::Relaxed);
        }
    }
}
