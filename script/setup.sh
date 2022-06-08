set -eu

# Minting new tokens
token_x=$(spl-token create-token | awk '{print $3}' | tr -d '\n')
token_y=$(spl-token create-token | awk '{print $3}' | tr -d '\n')
echo "Token X: $token_x"
echo "Token Y: $token_y"

# Create users with token accounts
alice=$(solana-keygen new -o ./alice-keypair.json --force --no-bip39-passphrase | grep -o "pubkey: .*" | awk '{print $2}' | tr -d '\n')
solana airdrop 1 $alice > /dev/null
alice_x=$(spl-token create-account $token_x --owner $alice | awk '{print $3}')
alice_y=$(spl-token create-account $token_y --owner $alice | awk '{print $3}')
spl-token mint $token_x 100 $alice_x > /dev/null

bob=$(solana-keygen new -o ./bob-keypair.json --force --no-bip39-passphrase | grep -o "pubkey: .*" | awk '{print $2}' | tr -d '\n')
bob_x=$(spl-token create-account $token_x --owner $bob | awk '{print $3}')
bob_y=$(spl-token create-account $token_y --owner $bob | awk '{print $3}')
spl-token mint $token_y 100 $bob_y > /dev/null

echo "Alice $alice"
echo "  $(spl-token balance --address $alice_x)X in $alice_x"
echo "  $(spl-token balance --address $alice_y)X in $alice_y"
echo "Bob $bob"
echo "  $(spl-token balance --address $bob_x)X in $bob_x"
echo "  $(spl-token balance --address $bob_y)Y in $bob_y"

# Post trade
program_id=$(solana-keygen pubkey /home/drgabble/mlabs/solana/target/deploy/program-keypair.json | tr -d '\n')
echo "Program id $program_id"
cargo run -- post $program_id $token_x ./alice-keypair.json  $alice_x 10 $alice_y 11

echo "Alice $alice"
echo "  $(spl-token balance --address $alice_x)X in $alice_x"
echo "  $(spl-token balance --address $alice_y)X in $alice_y"
echo "Bob $bob"
echo "  $(spl-token balance --address $bob_x)X in $bob_x"
echo "  $(spl-token balance --address $bob_y)Y in $bob_y"