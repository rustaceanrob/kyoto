
RPC_USER="test"
RPC_PASSWORD="kyoto"
CHAIN="regtest"
WALLET="test_kyoto"
bitcoin-cli -chain=$CHAIN -rpcuser=$RPC_USER -rpcpassword=$RPC_PASSWORD createwallet $WALLET
bitcoin-cli -chain=$CHAIN -rpcuser=$RPC_USER -rpcpassword=$RPC_PASSWORD loadwallet $WALLET
NEW_ADDRESS=$(bitcoin-cli -chain=$CHAIN -rpcuser=$RPC_USER -rpcpassword=$RPC_PASSWORD getnewaddress)
bitcoin-cli -chain=$CHAIN -rpcuser=$RPC_USER -rpcpassword=$RPC_PASSWORD generatetoaddress 2500 $NEW_ADDRESS