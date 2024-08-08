if [ -z "$1" ]; then
  echo "Usage: $0 <path to .bitcoin directory>"
  exit 1
fi
BITCOIN_DIR="$1"
echo "Cleaning up: $BITCOIN_DIR and starting integration tests"
rm -rf "$BITCOIN_DIR/regtest"
sleep 2
rm -rf data
bitcoind --chain=regtest --txindex --blockfilterindex --peerblockfilters --rpcport=18443 --rpcuser=test --rpcpassword=kyoto --rest=1 --server=1 --listen=1 --printtoconsole=0 &
sleep 2
cargo test test_reorg -- --nocapture
echo "Cleaning up..."
sleep 1
rm -rf "$BITCOIN_DIR/regtest"
rm -rf data
bitcoind --chain=regtest --txindex --blockfilterindex --peerblockfilters --rpcport=18443 --rpcuser=test --rpcpassword=kyoto --rest=1 --server=1 --listen=1 --printtoconsole=0 & 
sleep 2
cargo test test_mine_after_reorg -- --nocapture
echo "Cleaning up..."
sleep 1
rm -rf data
rm -rf "$BITCOIN_DIR/regtest"
bitcoind --chain=regtest --txindex --blockfilterindex --peerblockfilters --rpcport=18443 --rpcuser=test --rpcpassword=kyoto --rest=1 --server=1 --listen=1 --printtoconsole=0 &
sleep 2
cargo test test_sql_reorg -- --nocapture
echo "Cleaning up..."
sleep 1
rm -rf data
rm -rf "$BITCOIN_DIR/regtest"
bitcoind --chain=regtest --txindex --blockfilterindex --peerblockfilters --rpcport=18443 --rpcuser=test --rpcpassword=kyoto --rest=1 --server=1 --listen=1 --printtoconsole=0 &
sleep 2
cargo test test_two_deep_reorg -- --nocapture
echo "Cleaning up..."
sleep 1
rm -rf data
rm -rf "$BITCOIN_DIR/regtest"
bitcoind --chain=regtest --txindex --blockfilterindex --peerblockfilters --rpcport=18443 --rpcuser=test --rpcpassword=kyoto --rest=1 --server=1 --listen=1 --printtoconsole=0 &
sleep 2
RPC_USER="test"
RPC_PASSWORD="kyoto"
CHAIN="regtest"
WALLET="test_kyoto"
bitcoin-cli -chain=$CHAIN -rpcuser=$RPC_USER -rpcpassword=$RPC_PASSWORD createwallet $WALLET
bitcoin-cli -chain=$CHAIN -rpcuser=$RPC_USER -rpcpassword=$RPC_PASSWORD loadwallet $WALLET
NEW_ADDRESS=$(bitcoin-cli -chain=$CHAIN -rpcuser=$RPC_USER -rpcpassword=$RPC_PASSWORD getnewaddress)
bitcoin-cli -chain=$CHAIN -rpcuser=$RPC_USER -rpcpassword=$RPC_PASSWORD generatetoaddress 2500 $NEW_ADDRESS
cargo test test_sql_stale_anchor -- --nocapture