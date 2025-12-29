import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { SwivPrivacy } from "../target/types/swiv_privacy";
import { findMxeAccount, getCompDefPdaAsync } from "./utils";
import * as fs from "fs";
import * as path from "path";
import { uploadCircuit, buildFinalizeCompDefTx, getCompDefAccOffset } from "@arcium-hq/client";

describe("02_arcium_setup", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);
  const program = anchor.workspace.SwivPrivacy as Program<SwivPrivacy>;

  // Paths
  const PROCESS_BET_PATH = path.join(process.cwd(), "build", "process_bet.arcis"); 
  const REWARD_PATH = path.join(process.cwd(), "build", "calculate_reward_v2.arcis"); 

  it("Initializes Process Bet Comp Def (Upload & Finalize)", async () => {
    const mxeAccount = await findMxeAccount(provider.connection, program.programId);
    const compDef = await getCompDefPdaAsync(provider.connection, program.programId, "process_bet");

    // 1. Initialize Header
    try {
      await program.methods
        .initProcessBetCompDef()
        .accounts({
          payer: provider.wallet.publicKey,
          mxeAccount: mxeAccount,
          compDefAccount: compDef,
        })
        .rpc();
      console.log("    > Header Initialized.");
    } catch (e: any) {
      if (e.message.includes("already in use") || e.message.includes("0x0")) {
         console.log("    > Header already exists.");
      } else {
         throw e;
      }
    }

    // 2. Upload Circuit (Handle Persistence)
    if (!fs.existsSync(PROCESS_BET_PATH)) throw new Error(`Missing: ${PROCESS_BET_PATH}`);
    const rawCircuit = fs.readFileSync(PROCESS_BET_PATH);
    
    console.log("    > Uploading 'process_bet' circuit...");
    try {
        await uploadCircuit(
          provider, 
          "process_bet", 
          program.programId, 
          rawCircuit, 
          true, 
          100 // Chunk size 25
        );
        console.log("    > Circuit Uploaded.");
    } catch (e: any) {
        // CATCH "ALREADY IN USE"
        const logs = e.logs ? e.logs.join("") : e.message;
        if (logs.includes("already in use") || logs.includes("0x0")) {
            console.log("    > Circuit accounts already exist. Proceeding to finalize...");
        } else {
            throw e;
        }
    }

    // 3. Finalize
    const offsetBytes = getCompDefAccOffset("process_bet");
    const offsetNum = Buffer.from(offsetBytes).readUInt32LE(0);
    const finalizeTx = await buildFinalizeCompDefTx(provider, offsetNum, program.programId);
    
    try {
        const latestBlockhash = await provider.connection.getLatestBlockhash();
        finalizeTx.recentBlockhash = latestBlockhash.blockhash;
        finalizeTx.feePayer = provider.wallet.publicKey;
        
        const signed = await provider.wallet.signTransaction(finalizeTx);
        const sig = await provider.connection.sendRawTransaction(signed.serialize());
        await provider.connection.confirmTransaction({ signature: sig, ...latestBlockhash });
        console.log("    > Process Bet Finalized.");
    } catch (e: any) {
        if (e.message.includes("already completed") || e.message.includes("6303")) {
            console.log("    > Already finalized.");
        } else {
            throw e;
        }
    }
  });

  it("Initializes Calculate Reward Comp Def (Upload & Finalize)", async () => {
    const mxeAccount = await findMxeAccount(provider.connection, program.programId);
    const compDef = await getCompDefPdaAsync(provider.connection, program.programId, "calculate_reward_v2");

    // 1. Init
    try {
      await program.methods
        .initCalculateRewardCompDef()
        .accounts({
          payer: provider.wallet.publicKey,
          mxeAccount: mxeAccount,
          compDefAccount: compDef,
        })
        .rpc();
    } catch (e: any) {
      if (!e.message.includes("already in use")) throw e;
    }

    // 2. Upload (Handle Persistence)
    if (!fs.existsSync(REWARD_PATH)) {
       console.warn("Skipping reward upload (file not found)");
       return;
    }
    const rawCircuit = fs.readFileSync(REWARD_PATH);
    
    console.log("    > Uploading 'calculate_reward_v2' circuit...");
    try {
        await uploadCircuit(provider, "calculate_reward_v2", program.programId, rawCircuit, true, 25);
    } catch (e: any) {
        const logs = e.logs ? e.logs.join("") : e.message;
        if (logs.includes("already in use") || logs.includes("0x0")) {
            console.log("    > Circuit accounts already exist. Proceeding to finalize...");
        } else {
            throw e;
        }
    }

    // 3. Finalize
    const offsetBytes = getCompDefAccOffset("calculate_reward_v2");
    const offsetNum = Buffer.from(offsetBytes).readUInt32LE(0);
    const finalizeTx = await buildFinalizeCompDefTx(provider, offsetNum, program.programId);
    
    try {
        const latestBlockhash = await provider.connection.getLatestBlockhash();
        finalizeTx.recentBlockhash = latestBlockhash.blockhash;
        finalizeTx.feePayer = provider.wallet.publicKey;
        const signed = await provider.wallet.signTransaction(finalizeTx);
        const sig = await provider.connection.sendRawTransaction(signed.serialize());
        await provider.connection.confirmTransaction({ signature: sig, ...latestBlockhash });
        console.log("    > Reward Finalized.");
    } catch (e: any) {
        if (!e.message.includes("already completed")) throw e;
    }
  });
});