use tokio::net::ToSocketAddrs;

pub(crate) trait PeerPersist<T, E>
where
    T: ToSocketAddrs,
{
    // add a peer to list of peers to try. cannot already be in banned or tried.
    fn add_to_new(peer: T) -> Result<(), E>;

    // we have done a handshake with this peer and have compatible versions and services.
    fn add_to_tried(peer: T) -> Result<(), E>;

    // get a random peer we have tried before.
    fn get_random_tried() -> Result<T, E>;

    // get a random peer to try.
    fn get_random_new() -> Result<T, E>;

    // ban a peer for some unix time in seconds.
    fn ban(peer: T, time: u64) -> Result<(), E>;

    // filter peers with sufficient time past and unban them.
    fn unban() -> Result<(), E>;
}
