// Very simple denial of service protection so a peer cannot spam us with unsolicited messages.
#[derive(Debug, Clone)]
pub(crate) struct MessageCounter {
    version: i8,
    verack: i8,
    header: i32,
    filter_header: i32,
    filters: i64,
    addrs: i32,
    block: i32,
}

impl MessageCounter {
    pub(crate) fn new() -> Self {
        Self {
            version: 1,
            verack: 1,
            header: 0,
            filter_header: 0,
            filters: 0,
            addrs: 0,
            block: 0,
        }
    }

    pub(crate) fn got_version(&mut self) {
        self.version -= 1;
    }

    pub(crate) fn got_verack(&mut self) {
        self.verack -= 1;
    }

    pub(crate) fn got_header(&mut self) {
        self.header -= 1;
    }

    pub(crate) fn got_filter_header(&mut self) {
        self.filter_header -= 1;
    }

    pub(crate) fn got_filter(&mut self) {
        self.filters -= 1;
    }

    pub(crate) fn got_addrs(&mut self) {
        self.addrs -= 1;
    }

    pub(crate) fn got_block(&mut self) {
        self.block -= 1;
    }

    pub(crate) fn sent_header(&mut self) {
        self.header += 1;
    }

    pub(crate) fn sent_filter_header(&mut self) {
        self.filter_header += 1;
    }

    pub(crate) fn sent_filters(&mut self) {
        self.filters += 1000;
    }

    pub(crate) fn sent_addrs(&mut self) {
        self.addrs += 5;
    }

    pub(crate) fn sent_block(&mut self) {
        self.block += 1;
    }

    pub(crate) fn unsolicited(&self) -> bool {
        self.version < 0
            || self.header < 0
            || self.filters < 0
            || self.verack < 0
            || self.filter_header < 0
            || self.addrs < 0
            || self.block < 0
    }
}
