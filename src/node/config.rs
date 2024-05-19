use std::{marker::PhantomData, net::IpAddr};

use bitcoin::Network;

use crate::tx::store::TransactionStore;

pub struct NodeConfig<S, E>
where
    S: TransactionStore<E>,
{
    required_peers: usize,
    white_list: Option<Vec<(IpAddr, u16)>>,
    network: Network,
    addresses: Vec<bitcoin::Address>,
    tx_store: S,
    phatom: PhantomData<E>,
}
