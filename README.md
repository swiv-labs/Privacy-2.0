# Swiv with Arcium Integration

Privacy-preserving prediction market on Solana using Arcium's Multi-Party Computation (MPC) for confidential predictions.

## Overview

This integration adds privacy to your prediction market by encrypting users' predicted prices and stake amounts. The encrypted data is processed by Arcium's MPC network, ensuring predictions remain private until pool finalization.

## Key Privacy Features

- ✅ **Private Predictions**: User's predicted prices are encrypted and never revealed publicly
- ✅ **Confidential Computation**: Reward calculations happen on encrypted data via MPC
- ✅ **Fair Distribution**: Rewards are calculated without revealing individual predictions
- ✅ **Zero Knowledge**: Pool participants can't see others' predictions until finalized

## Architecture Changes

### Original Flow
1. User places bet → predicted_price stored in plaintext
2. Pool finalizes → everyone can see all predictions
3. Rewards calculated → transparent computation

### New Arcium-Enhanced Flow
1. User encrypts predicted_price client-side
2. Encrypted data sent to program → stored as ciphertext
3. Arcium MPC nodes process encrypted data
4. Pool finalizes → predictions remain private
5. Rewards calculated via MPC → results returned encrypted
6. Users claim rewards without revealing predictions

## Project Structure

```
swiv_privacy/
├── Arcium.toml                    # Arcium configuration
├── Cargo.toml                     # Dependencies with Arcium libs
├── programs/
│   └── swiv_privacy/
│       └── src/
│           └── lib.rs             # Main program with Arcium integration
├── encrypted-ixs/                 # Confidential instructions
│   └── bet_processing.rs          # MPC computation logic
└── client/
    └── encryption.ts              # TypeScript encryption helpers
```

## Installation & Setup

### 1. Install Arcium CLI

```bash
# Install Arcium CLI (wrapper over Anchor CLI)
npm install -g @arcium/cli

# Or using cargo
cargo install arcium-cli
```

### 2. Initialize Project

```bash
# If starting fresh
arcium init swiv_privacy
cd swiv_privacy

# If integrating into existing project, replace:
# - Replace #[program] with #[arcium_program]
# - Add Arcium dependencies to Cargo.toml
# - Add encrypted-ixs/ directory
```

### 3. Update Dependencies

Add to your `Cargo.toml`:

```toml
[dependencies]
anchor-lang = "0.32.1"
anchor-spl = "0.32.1"
arcium-macros = "0.4.0"
arcium-client = "0.4.0"
arcis-imports = "0.4.0"
```

### 4. Build the Program

```bash
# Build with Arcium
arcium build

# This compiles both:
# - Your main Solana program
# - Encrypted instructions for MPC
```

### 5. Deploy to Testnet

```bash
# Deploy program
arcium deploy --provider.cluster testnet

# Initialize protocol
arcium run initialize --provider.cluster testnet
```

## Usage

### Client-Side Integration

```typescript
import { ArciumClient } from '@arcium/client';
import { swiv_privacyEncryption } from './client/encryption';

// Initialize Arcium client
const arciumClient = new ArciumClient({
  network: 'testnet',
});

const encryption = new swiv_privacyEncryption(arciumClient);

// 1. Initialize computation definition (one-time setup)
const initCompDefTx = await encryption.initializeCompDef(
  program,
  payerKeypair
);
await provider.sendAndConfirm(new Transaction().add(initCompDefTx));

// 2. Place encrypted bet
const predictedPrice = 50000; // User's private prediction
const betIx = await encryption.createEncryptedBetInstruction(
  program,
  poolId,
  predictedPrice,
  userKeypair,
  poolAccount,
  poolVaultAccount,
  userTokenAccount,
  compDefAccount
);

await provider.sendAndConfirm(new Transaction().add(betIx));

// 3. Calculate rewards (after pool finalization)
const rewardIx = await encryption.calculateRewardInstruction(
  program,
  poolAccount,
  userKeypair
);

await provider.sendAndConfirm(new Transaction().add(rewardIx));
```

### Program Instructions

#### Initialize (Unchanged)
```rust
pub fn initialize(ctx: Context<Initialize>, protocol_fee_bps: u16)
```

#### Create Pool (Unchanged)
```rust
pub fn create_pool(
    ctx: Context<CreatePool>,
    pool_id: u64,
    asset_symbol: String,
    entry_fee: u64,
    target_timestamp: i64,
    max_participants: u32,
)
```

#### Place Encrypted Bet (New)
```rust
pub fn place_encrypted_bet(
    ctx: Context<PlaceEncryptedBet>,
    encrypted_bet_data: [u8; 32],  // Encrypted predicted_price
    public_key: [u8; 32],          // User's encryption key
    nonce: [u8; 16],               // Encryption nonce
)
```

#### Finalize Pool (Unchanged)
```rust
pub fn finalize_pool(ctx: Context<FinalizePool>, actual_price: u64)
```

#### Calculate Reward (New)
```rust
pub fn calculate_reward(ctx: Context<CalculateReward>)
```

## Key Components

### 1. Encrypted Instructions (`encrypted-ixs/bet_processing.rs`)

These run in Arcium's MPC environment:

- **`process_encrypted_bet`**: Validates encrypted predictions
- **`calculate_encrypted_reward`**: Computes rewards on encrypted data
- **`aggregate_predictions`**: Calculate statistics without revealing individual values
- **`compare_predictions`**: Rank predictions privately

### 2. Modified Account Structure

```rust
#[account]
pub struct EncryptedBet {
    pub user: Pubkey,
    pub pool: Pubkey,
    pub encrypted_data: [u8; 32],      // Ciphertext
    pub user_public_key: [u8; 32],     // For decryption
    pub nonce: [u8; 16],               // Encryption nonce
    pub stake_amount: u64,             // Public (for pool tracking)
    pub claimed: bool,
    pub bump: u8,
}
```

### 3. Computation Flow

```
Client                  Solana Program              Arcium MPC Network
  |                           |                            |
  |-- Encrypt Data -------->  |                            |
  |                           |                            |
  |-- Place Bet ----------->  |                            |
  |                           |-- Submit Computation ---->  |
  |                           |                            |
  |                           |                     [Process Encrypted]
  |                           |                            |
  |                           | <---- Return Result ----   |
  |                           |                            |
  |                      [Callback]                        |
  | <-- Confirmation ----    |                            |
```

## Security Considerations

### What's Private
- ✅ Individual predicted prices
- ✅ Prediction accuracy before finalization
- ✅ Individual reward amounts (optional)

### What's Public
- ⚠️ Pool total amount
- ⚠️ Number of participants
- ⚠️ Entry fees
- ⚠️ Final actual price
- ⚠️ User participation (addresses visible on-chain)

### Encryption Details
- Uses NaCl (TweetNaCl) for client-side encryption
- Arcium MPC nodes process without learning plaintext
- Threshold cryptography ensures no single node can decrypt
- Computations verified on-chain via Solana

## Testing

```bash
# Run tests
arcium test

# Test encrypted instructions
arcium test --encrypted-only
```

Example test:

```typescript
it('Places encrypted bet', async () => {
  const predictedPrice = 50000;
  
  const { encryptedData, publicKey, nonce } = 
    await encryption.encryptBetData(predictedPrice, user);
  
  await program.methods
    .placeEncryptedBet(encryptedData, publicKey, nonce)
    .accounts({...})
    .rpc();
  
  const bet = await program.account.encryptedBet.fetch(betAccount);
  assert(bet.encrypted_data !== null);
  // Predicted price should NOT be readable
});
```

## Migration from Original Code

### Step 1: Replace Bet Account
```rust
// OLD
#[account]
pub struct Bet {
    pub predicted_price: u64,  // Plaintext!
    // ...
}

// NEW
#[account]
pub struct EncryptedBet {
    pub encrypted_data: [u8; 32],  // Encrypted!
    // ...
}
```

### Step 2: Update Instructions
```rust
// OLD
pub fn place_bet(ctx: Context<PlaceBet>, predicted_price: u64)

// NEW
pub fn place_encrypted_bet(
    ctx: Context<PlaceEncryptedBet>,
    encrypted_bet_data: [u8; 32],
    // ...
)
```

### Step 3: Add MPC Computation
```rust
// Add confidential computation call
arcium_client::invoke_computation(
    &ctx.accounts.comp_def,
    &ctx.accounts.computation,
    // ...
)?;
```

## Limitations & Trade-offs

1. **Performance**: MPC adds latency (typically 2-5 seconds per computation)
2. **Cost**: MPC computations incur additional fees
3. **Complexity**: More complex client-side encryption logic
4. **Testnet Only**: Currently on Arcium Public Testnet

## Benefits

1. **Privacy**: True prediction privacy until finalization
2. **Fair Competition**: No front-running based on visible predictions
3. **Regulatory Compliance**: Better privacy for sensitive financial data
4. **User Trust**: Users can verify MPC computation integrity

## Resources

- [Arcium Documentation](https://docs.arcium.com)
- [Arcium Discord](https://discord.com/invite/arcium)
- [TypeScript SDK](https://ts.arcium.com/api)
- [Example Programs](https://github.com/arcium-network/examples)

## Troubleshooting

### Build Errors
```bash
# Clear cache
arcium clean
arcium build
```

### Encryption Issues
- Ensure nonce is 16 bytes
- Verify public key format (32 bytes)
- Check ciphertext padding (32 bytes)

### Computation Timeouts
- MPC computations can take 2-5 seconds
- Implement proper retry logic
- Monitor computation status on-chain

## Next Steps

1. Test on Arcium Testnet
2. Implement additional privacy features
3. Optimize MPC computation costs
4. Add privacy-preserving analytics
5. Deploy to mainnet when Arcium launches

## Support

- Discord: [Arcium Community](https://discord.com/invite/arcium)
- Docs: https://docs.arcium.com
- GitHub: https://github.com/arcium-network