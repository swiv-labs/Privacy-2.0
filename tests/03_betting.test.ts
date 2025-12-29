import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { SwivPrivacy } from "../target/types/swiv_privacy";
import { ComputeBudgetProgram } from "@solana/web3.js";
import { assert } from "chai";
import {
  getPoolPDA,
  getPoolVaultPDA,
  getBetPDA,
  getArciumContext,
} from "./utils";
import { getOrCreateAssociatedTokenAccount, mintTo } from "@solana/spl-token";

describe("03_betting", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);
  const program = anchor.workspace.SwivPrivacy as Program<SwivPrivacy>;

  let usdcMint: anchor.web3.PublicKey;
  let user1 = anchor.web3.Keypair.generate();
  let user1ATA: anchor.web3.PublicKey;

  // Use a unique ID based on time to ensure fresh state for every test run
  const poolId = new anchor.BN(Date.now());

  // Hardcoded addresses for Arcium Devnet (Fee Pool & Clock)
  // If these change on devnet, update them here.
  const arciumFeePool = new anchor.web3.PublicKey(
    "BSC6rWJ9ucqZ6rcM3knfpgdRwCyJ7Q9KsddjeSL4EdHq"
  );
  const arciumClock = new anchor.web3.PublicKey(
    "EQr6UCd7eyRjpuRsNK6a8WxkgrpSGctKMFuz92FRRh63"
  );

  before(async () => {
    const payer = (provider.wallet as any).payer;
    const { createMint } = require("@solana/spl-token");

    // 1. Create a Mock USDC Mint
    usdcMint = await createMint(
      provider.connection,
      payer,
      payer.publicKey,
      null,
      6
    );

    // 2. Fund the User with SOL
    const transferTx = new anchor.web3.Transaction().add(
      anchor.web3.SystemProgram.transfer({
        fromPubkey: payer.publicKey,
        toPubkey: user1.publicKey,
        lamports: 1 * anchor.web3.LAMPORTS_PER_SOL,
      })
    );
    await provider.sendAndConfirm(transferTx);

    // 3. Create User's Token Account and Mint Mock USDC
    user1ATA = (
      await getOrCreateAssociatedTokenAccount(
        provider.connection,
        payer,
        usdcMint,
        user1.publicKey
      )
    ).address;

    await mintTo(
      provider.connection,
      payer,
      usdcMint,
      user1ATA,
      payer,
      10_000_000
    );

    // 4. Initialize Protocol & Create a Pool (if not already done)
    const poolPda = getPoolPDA(poolId, program.programId);
    const poolVault = getPoolVaultPDA(poolId, program.programId);
    const protocolState = anchor.web3.PublicKey.findProgramAddressSync(
      [Buffer.from("protocol_state")],
      program.programId
    )[0];

    try {
      await program.methods
        .initialize(500)
        .accounts({ protocolState, admin: payer.publicKey })
        .rpc();
    } catch (e) {
      // Ignore if already initialized
    }

    await program.methods
      .createPool(
        poolId,
        "BTC",
        new anchor.BN(1_000_000),
        new anchor.BN(Math.floor(Date.now() / 1000) + 10000), // Target time: Now + 10000s
        100 // Max participants
      )
      .accounts({
        protocolState,
        pool: poolPda,
        poolVault,
        tokenMint: usdcMint,
        admin: payer.publicKey,
        tokenProgram: anchor.utils.token.TOKEN_PROGRAM_ID,
        systemProgram: anchor.web3.SystemProgram.programId,
        rent: anchor.web3.SYSVAR_RENT_PUBKEY,
      })
      .rpc();
  });

  it("Places an Encrypted Bet", async () => {
    const poolPda = getPoolPDA(poolId, program.programId);
    const poolVault = getPoolVaultPDA(poolId, program.programId);
    const encryptedBetPda = getBetPDA(
      poolPda,
      user1.publicKey,
      program.programId
    );

    // --- ARCIUM CONTEXT FETCHING ---
    // IMPORTANT: We pass 'provider' here so utils.ts can fetch the Cluster ID from the MXE account
    const arcium = await getArciumContext(
      provider,
      program.programId,
      "process_bet"
    );
    console.log("    > Using MXE:", arcium.mxeAccount.toBase58());
    console.log("    > Comp Def:", arcium.compDefAccount.toBase58());

    // Verify Comp Def exists (it should have been created in step 02)
    const accInfo = await provider.connection.getAccountInfo(
      arcium.compDefAccount
    );
    if (!accInfo) {
      throw new Error(
        `Comp Def Account ${arcium.compDefAccount.toBase58()} does not exist. Please run 'anchor test --run 02_arcium_setup' first.`
      );
    }

    // --- PREPARE MOCK DATA ---
    // Using Buffer ensures correct byte format.
    // We convert to Array.from() because Anchor generated types usually expect number[] for [u8; 32]
    const encryptedPrice = Buffer.alloc(32).fill(1); // Mock Encrypted Data
    const pubKey = Buffer.alloc(32).fill(2); // Mock Public Key (must match circuit format)
    const nonce = new anchor.BN(12345);

    try {
      // Define the instruction to request more Compute Units
      const modifyComputeUnits = ComputeBudgetProgram.setComputeUnitLimit({
        units: 600_000, // Increase to 600k (Safe upper bound)
      });

      await program.methods
        .placeEncryptedBet(
          poolId,
          Array.from(encryptedPrice),
          Array.from(pubKey),
          nonce
        )
        .accounts({
          // ... all your accounts ...
          user: user1.publicKey,
          pool: poolPda,
          poolVault: poolVault,
          tokenMint: usdcMint,
          userTokenAccount: user1ATA,
          encryptedBet: encryptedBetPda,
          mxeAccount: arcium.mxeAccount,
          compDefAccount: arcium.compDefAccount,
          clusterAccount: arcium.clusterAccount,
          mempoolAccount: arcium.mempoolAccount,
          executingPool: arcium.executingPool,
          computationAccount: arcium.computationAccount,
          poolAccount: arciumFeePool,
          clockAccount: arciumClock,
          tokenProgram: anchor.utils.token.TOKEN_PROGRAM_ID,
          systemProgram: anchor.web3.SystemProgram.programId,
          arciumProgram: arcium.arciumProgram,
        })
        .preInstructions([modifyComputeUnits]) // <--- ADD THIS LINE
        .signers([user1])
        .rpc();

      console.log("    > Bet Placed Successfully");
    } catch (e: any) {
      console.error("    > Bet Failed:", e);
      if (e.logs) console.log("    > Logs:", e.logs);
      throw e;
    }
  });
});
