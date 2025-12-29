import * as anchor from "@coral-xyz/anchor";
import { PublicKey } from "@solana/web3.js";
import * as fs from "fs";
import * as path from "path";
// Updated Imports: Added 'getArciumProgram'
import { 
  getArciumProgramId,
  getMXEAccAddress,
  getClusterAccAddress,
  getMempoolAccAddress,
  getExecutingPoolAccAddress,
  getCompDefAccAddress,
  getComputationAccAddress,
  getCompDefAccOffset,
  getArciumProgram
} from "@arcium-hq/client";

// --- Constants ---
export const PROGRAM_ID = new PublicKey("GVZ7596AK1vqiNDSL4pGoNVCSMruXVAWgVKxGVWNE9Pt");

export const SEED_PROTOCOL_STATE = Buffer.from("protocol_state");
export const SEED_POOL = Buffer.from("pool");
export const SEED_POOL_VAULT = Buffer.from("pool_vault");
export const SEED_BET = Buffer.from("bet");

// --- Data Persistence ---
const DATA_FILE = path.join(process.cwd(), "tests", "test_data.json");
interface TestData {
  poolId?: string;
  mint?: string;
}

export const saveTestData = (data: TestData) => {
  let current = {};
  if (fs.existsSync(DATA_FILE)) {
    try {
      current = JSON.parse(fs.readFileSync(DATA_FILE, "utf8"));
    } catch { /* ignore */ }
  }
  const updated = { ...current, ...data };
  fs.writeFileSync(DATA_FILE, JSON.stringify(updated));
};

export const loadTestData = (): TestData => {
  if (!fs.existsSync(DATA_FILE)) {
    return {};
  }
  return JSON.parse(fs.readFileSync(DATA_FILE, "utf8"));
};

// --- Helper: Find MXE ---
export const findMxeAccount = async (connection: anchor.web3.Connection, clientProgramId: PublicKey): Promise<PublicKey> => {
    const arciumId = new PublicKey(getArciumProgramId());
    
    // 1. SDK Derivation
    const standardPda = getMXEAccAddress(clientProgramId);
    
    const info = await connection.getAccountInfo(standardPda);
    if (info) return standardPda;

    // 2. Deep Search Fallback
    console.log("    > Standard MXE derivation failed. Searching network...");
    const accounts = await connection.getProgramAccounts(arciumId, {
        filters: [
            {
                memcmp: {
                    offset: 21, 
                    bytes: clientProgramId.toBase58()
                }
            }
        ]
    });
    if (accounts.length === 0) {
        throw new Error("MXE Account not found on Devnet. Did 'arcium deploy' finish?");
    }
    return accounts[0].pubkey;
};

// --- Arcium Context Helper ---
export const getArciumContext = async (
    provider: anchor.AnchorProvider, // CHANGED: Now takes Provider, not just Connection
    clientProgramId: PublicKey, 
    compDefName: string
) => {
    // 1. Get MXE Public Key
    const mxeAccount = await findMxeAccount(provider.connection, clientProgramId);
    const arciumProgram = new PublicKey(getArciumProgramId());

    // 2. FETCH MXE DATA TO GET CORRECT CLUSTER ID
    // We use the SDK program to fetch the account state
    const sdkProgram = getArciumProgram(provider);
    
    // Explicitly casting to any to avoid strict TS issues if IDL types aren't perfect locally
    const mxeState = await sdkProgram.account.mxeAccount.fetch(mxeAccount) as any;
    
    if (mxeState.cluster === null || mxeState.cluster === undefined) {
      throw new Error(`MXE Account ${mxeAccount.toBase58()} is not assigned to any Cluster yet.`);
    }

    const clusterId = Number(mxeState.cluster);
    console.log(`    > MXE is on Cluster ID: ${clusterId}`);

    // 3. Derive Cluster-based Accounts using the REAL ID
    const clusterAccount = getClusterAccAddress(clusterId);
    const mempoolAccount = getMempoolAccAddress(clusterId);
    const executingPool = getExecutingPoolAccAddress(clusterId);
    
    // 4. Derive Comp Def Account
    // Convert name to offset (number)
    const rawOffsetBytes = getCompDefAccOffset(compDefName);
    const compDefOffset = Buffer.from(rawOffsetBytes).readUInt32LE(0);
    
    const compDefAccount = getCompDefAccAddress(clientProgramId, compDefOffset);

    // 5. Derive Computation Account
    // We use compDefOffset as the ID for deterministic testing
    const computationId = new anchor.BN(compDefOffset);
    const computationAccount = getComputationAccAddress(clusterId, computationId);

    return {
        arciumProgram,
        mxeAccount,
        clusterAccount,
        mempoolAccount,
        executingPool,
        compDefAccount,
        computationAccount
    };
};

// Helper for Setup (Comp Def Only)
export const getCompDefPdaAsync = async (connection: anchor.web3.Connection, clientProgramId: PublicKey, name: string) => {
    const rawOffsetBytes = getCompDefAccOffset(name);
    const compDefOffset = Buffer.from(rawOffsetBytes).readUInt32LE(0);
    return getCompDefAccAddress(clientProgramId, compDefOffset);
}

// --- Native PDAs ---
export const getProtocolStatePDA = (programId: PublicKey) => {
  return PublicKey.findProgramAddressSync([SEED_PROTOCOL_STATE], programId)[0];
};

export const getPoolPDA = (poolId: anchor.BN, programId: PublicKey) => {
  return PublicKey.findProgramAddressSync(
    [SEED_POOL, poolId.toArrayLike(Buffer, "le", 8)],
    programId
  )[0];
};

export const getPoolVaultPDA = (poolId: anchor.BN, programId: PublicKey) => {
  return PublicKey.findProgramAddressSync(
    [SEED_POOL_VAULT, poolId.toArrayLike(Buffer, "le", 8)],
    programId
  )[0];
};

export const getBetPDA = (pool: PublicKey, user: PublicKey, programId: PublicKey) => {
  return PublicKey.findProgramAddressSync(
    [SEED_BET, pool.toBuffer(), user.toBuffer()],
    programId
  )[0];
};

export const sleep = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));