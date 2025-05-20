use std::time::Duration;

use tokio::time::Instant;

// Very simple denial of service protection so a peer cannot spam us with unsolicited messages.
#[derive(Debug, Clone)]
pub(crate) struct MessageCounter {
    timer: MessageTimer,
    header: i32,
    filter_header: i32,
    filters: i64,
    block: i32,
}

impl MessageCounter {
    pub(crate) fn new(timeout: Duration) -> Self {
        Self {
            timer: MessageTimer::new(timeout),
            header: 0,
            filter_header: 0,
            filters: 0,
            block: 0,
        }
    }

    pub(crate) fn got_header(&mut self) {
        self.timer.untrack();
    }

    pub(crate) fn got_filter_header(&mut self) {
        self.timer.untrack();
        self.filter_header -= 1;
    }

    pub(crate) fn got_filter(&mut self) {
        self.timer.untrack();
        self.filters -= 1;
    }

    pub(crate) fn got_block(&mut self) {
        self.timer.untrack();
        self.block -= 1;
    }

    pub(crate) fn sent_header(&mut self) {
        self.timer.track();
    }

    pub(crate) fn sent_filter_header(&mut self) {
        self.timer.track();
        self.filter_header += 1;
    }

    pub(crate) fn sent_filters(&mut self) {
        self.timer.track();
        self.filters += 1000;
    }

    pub(crate) fn sent_block(&mut self) {
        self.timer.track();
        self.block += 1;
    }

    pub(crate) fn unsolicited(&self) -> bool {
        self.header < 0 || self.filters < 0 || self.filter_header < 0 || self.block < 0
    }

    pub(crate) fn unresponsive(&self) -> bool {
        self.timer.unresponsive()
    }
}

//
#[derive(Debug, Clone)]
pub(crate) struct MessageTimer {
    tracked_time: Option<Instant>,
    timeout: Duration,
}

impl MessageTimer {
    pub(crate) fn new(timeout: Duration) -> Self {
        Self {
            tracked_time: None,
            timeout,
        }
    }

    pub(crate) fn track(&mut self) {
        self.tracked_time = Some(Instant::now())
    }

    pub(crate) fn untrack(&mut self) {
        self.tracked_time = None;
    }

    pub(crate) fn unresponsive(&self) -> bool {
        match self.tracked_time {
            Some(time) => Instant::now().duration_since(time) > self.timeout,
            None => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use tokio::time;

    use super::*;

    #[tokio::test(start_paused = true)]
    async fn test_timer_works() {
        let mut timer = MessageTimer::new(Duration::from_secs(3));
        assert!(!timer.unresponsive());
        timer.track();
        assert!(!timer.unresponsive());
        timer.untrack();
        assert!(!timer.unresponsive());
        timer.untrack();
        assert!(!timer.unresponsive());
        timer.track();
        assert!(!timer.unresponsive());
        time::sleep(Duration::from_secs(6)).await;
        assert!(timer.unresponsive());
    }

    #[test]
    fn test_counter_works() {
        let mut counter = MessageCounter::new(Duration::from_secs(3));
        counter.sent_header();
        assert!(counter.timer.tracked_time.is_some());
        counter.got_header();
        assert!(counter.timer.tracked_time.is_none());
        counter.sent_filter_header();
        assert!(counter.timer.tracked_time.is_some());
        counter.got_filter_header();
        assert!(counter.timer.tracked_time.is_none());
        counter.sent_filters();
        assert!(counter.timer.tracked_time.is_some());
        counter.got_filter();
        assert!(counter.timer.tracked_time.is_none());
        counter.sent_block();
        assert!(counter.timer.tracked_time.is_some());
        counter.got_block();
        assert!(counter.timer.tracked_time.is_none());
    }
}
