set -eu

# Minting new tokens
token_x=$(spl-token create-token | awk '{print $3}' | tr -d '\n')
token_y=$(spl-token create-token | awk '{print $3}' | tr -d '\n')

# Create users with token accounts
alice=$(solana-keygen new -o ./alice-keypair.json --force --no-bip39-passphrase | grep -o "pubkey: .*" | awk '{print $2}' | tr -d '\n')
alice_x=$(spl-token create-account $token_x --owner $alice | awk '{print $3}')
#alice_y=$(spl-token create-account $token_y --owner $alice | awk '{print $3}')
spl-token mint $token_x 100 $alice_x > /dev/null

bob=$(solana-keygen new -o ./bob-keypair.json --force --no-bip39-passphrase | grep -o "pubkey: .*" | awk '{print $2}' | tr -d '\n')
#bob_x=$(spl-token create-account $token_x --owner $bob | awk '{print $3}')
bob_y=$(spl-token create-account $token_y --owner $bob | awk '{print $3}')
spl-token mint $token_y 100 $bob_y > /dev/null

echo "Token X: $token_x, Token Y: $token_y"
echo "Alice $alice with $(spl-token balance --address $alice_x)X in $alice_x"
echo "Bob $bob with $(spl-token balance --address $bob_y)Y in $bob_y"