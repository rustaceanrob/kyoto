use crate::impl_sourceless_error;

#[derive(Debug)]
pub enum CFHeaderSyncError {
    EmptyMessage,
    UnknownStophash,
    StopHashMismatch,
    UnrequestedStophash,
    PrevHeaderMismatch,
    HeaderChainIndexOverflow,
    UnexpectedCFHeaderMessage,
    StartHeightMisalignment,
}

impl core::fmt::Display for CFHeaderSyncError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            CFHeaderSyncError::EmptyMessage => write!(f, "empty headers message."),
            CFHeaderSyncError::UnknownStophash => {
                write!(f, "a stop hash received was not found in our chain.")
            }
            CFHeaderSyncError::StopHashMismatch => {
                write!(f, "the requested and received stop hashes do not match.")
            }
            CFHeaderSyncError::UnrequestedStophash => {
                write!(f, "we did not request this stop hash.")
            }
            CFHeaderSyncError::PrevHeaderMismatch => write!(f, "previous filter header mismatch."),
            CFHeaderSyncError::HeaderChainIndexOverflow => write!(
                f,
                "indexed out of bounds on the header chain trying to find a block hash."
            ),
            CFHeaderSyncError::UnexpectedCFHeaderMessage => write!(
                f,
                "we already had a message from this peer staged in our queue."
            ),
            CFHeaderSyncError::StartHeightMisalignment => write!(
                f,
                "the size of the batch and the requested start height do not align"
            ),
        }
    }
}

impl_sourceless_error!(CFHeaderSyncError);

#[derive(Debug)]
pub enum CFilterSyncError {
    UnknownStophash,
    UnrequestedStophash,
    UnknownFilterHash,
    MisalignedFilterHash,
    Filter(FilterError),
}

impl core::fmt::Display for CFilterSyncError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CFilterSyncError::UnknownStophash => {
                write!(f, "a stop hash recevied was not found in our chain.")
            }
            CFilterSyncError::UnrequestedStophash => {
                write!(f, "we did not request this stop hash.")
            }
            CFilterSyncError::UnknownFilterHash => write!(
                f,
                "we could not find the filter hash corresponding to that stop hash."
            ),
            CFilterSyncError::MisalignedFilterHash => write!(
                f,
                "the filter hash from our header chain and this filter hash do not match."
            ),
            CFilterSyncError::Filter(_) => write!(
                f,
                "the filter experienced an IO error checking for Script inclusions."
            ),
        }
    }
}

impl_sourceless_error!(CFilterSyncError);

#[derive(Debug)]
pub enum FilterError {
    IORead,
}

impl core::fmt::Display for FilterError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FilterError::IORead => write!(f, "unable to read from the filter contents buffer."),
        }
    }
}

impl_sourceless_error!(FilterError);
