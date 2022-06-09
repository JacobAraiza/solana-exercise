set -eu

# Minting new tokens
echo "Minting new tokens"
token_x=$(spl-token create-token | awk '{print $3}' | tr -d '\n')
token_y=$(spl-token create-token | awk '{print $3}' | tr -d '\n')
echo "  Token X: $token_x"
echo "  Token Y: $token_y"

# Create Alice with 100X
alice=$(solana-keygen new -o ./alice-keypair.json --force --no-bip39-passphrase | grep -o "pubkey: .*" | awk '{print $2}' | tr -d '\n')
solana airdrop 100 $alice > /dev/null
alice_x=$(spl-token create-account $token_x --owner $alice | awk '{print $3}')
spl-token mint $token_x 100 $alice_x > /dev/null

# Create Bob with 100Y
bob=$(solana-keygen new -o ./bob-keypair.json --force --no-bip39-passphrase | grep -o "pubkey: .*" | awk '{print $2}' | tr -d '\n')
solana airdrop 100 $bob > /dev/null
bob_y=$(spl-token create-account $token_y --owner $bob | awk '{print $3}')
spl-token mint $token_y 100 $bob_y > /dev/null

function echo_balances() {
    alice_x=$(spl-token balance $token_x --owner $alice)
    alice_y=$(spl-token balance $token_y --owner $alice 2>/dev/null || echo 0)
    alice_sol=$(solana balance $alice)
    bob_x=$(spl-token balance $token_x --owner $bob 2>/dev/null || echo 0)
    bob_y=$(spl-token balance $token_y --owner $bob)
    bob_sol=$(solana balance $bob)
    echo "Current balances"
    echo "  Alice $alice ${alice_x}X and ${alice_y}Y $alice_sol"
    echo "  Bob $bob ${bob_x}X and ${bob_y}Y $bob_sol"
}

echo_balances

# Post trade
function post_trade() {
    echo "Posting trade"
    trade_output=$(cargo run -- post ./alice-keypair.json  $token_x 10 $token_y 11)
    escrow=$(echo $trade_output | awk '{print $4}' | tr -d '\n')
    tokens=$(echo $trade_output | awk '{print $8}' | tr -d '\n')
    echo "  Tokens account: $tokens"
    echo "  Escrow account: $escrow"
}

post_trade
echo_balances

# Cancel trade
echo "Cancelling trade"
cargo run -- cancel ./alice-keypair.json $escrow
echo_balances

# Post trade again
post_trade
echo_balances

# Take trade
echo "Taking trade"
cargo run -- take ./bob-keypair.json $escrow
echo_balances