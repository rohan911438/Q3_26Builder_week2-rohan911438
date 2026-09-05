import * as anchor from "@coral-xyz/anchor";
import { Program, BN } from "@coral-xyz/anchor";
import {
  Keypair,
  LAMPORTS_PER_SOL,
  PublicKey,
  SystemProgram,
  Transaction,
  TransactionInstruction,
} from "@solana/web3.js";
import {
  TOKEN_PROGRAM_ID,
  ASSOCIATED_TOKEN_PROGRAM_ID,
  MINT_SIZE,
  createInitializeMint2Instruction,
  createAssociatedTokenAccountInstruction,
  createMintToInstruction,
  getAssociatedTokenAddressSync,
  getAccount,
} from "@solana/spl-token";
import { startAnchor, Clock, ProgramTestContext } from "solana-bankrun";
import { BankrunProvider } from "anchor-bankrun";
import { assert } from "chai";
import { Escrow } from "../target/types/escrow";
import escrowIdl from "../target/idl/escrow.json";

// These tests exercise the timed-escrow expiry/reclaim behavior via `solana-bankrun`
// instead of the local-validator harness used in tests/escrow.ts. A real validator's
// Clock sysvar only advances in step with real wall-clock time (and, on this slow
// WSL/`/mnt/c` setup, can stall for tens of seconds under disk-I/O pressure — see
// README), which makes deadline tests either slow or flaky. Bankrun's `setClock`
// warps the on-chain clock instantly and deterministically, exactly as recommended
// by the original assignment for solana-test-validator's clock-manipulation gap.
describe("escrow timed extension (bankrun)", () => {
  const DECIMALS = 6;
  const DEPOSIT_AMOUNT = 1_000_000;
  const RECEIVE_AMOUNT = 2_000_000;
  const DEADLINE_DURATION = 60; // arbitrary; bankrun warps past it instantly

  let context: ProgramTestContext;
  let provider: BankrunProvider;
  let program: Program<Escrow>;
  let payer: Keypair;

  let maker: Keypair;
  let taker: Keypair;
  let mintA: PublicKey;
  let mintB: PublicKey;
  let makerAtaA: PublicKey;
  let takerAtaB: PublicKey;

  const sendIxs = async (ixs: TransactionInstruction[], signers: Keypair[]) => {
    const tx = new Transaction();
    const [blockhash] = await context.banksClient.getLatestBlockhash();
    tx.recentBlockhash = blockhash;
    tx.feePayer = payer.publicKey;
    tx.add(...ixs);
    tx.sign(...signers);
    const result = await context.banksClient.tryProcessTransaction(tx);
    if (result.result !== null) {
      throw new Error(`transaction failed: ${result.result}\n${result.meta?.logMessages.join("\n")}`);
    }
  };

  const createMint = async (decimals: number, authority: PublicKey): Promise<PublicKey> => {
    const mintKp = Keypair.generate();
    const lamports = await provider.connection.getMinimumBalanceForRentExemption(MINT_SIZE);
    await sendIxs(
      [
        SystemProgram.createAccount({
          fromPubkey: payer.publicKey,
          newAccountPubkey: mintKp.publicKey,
          space: MINT_SIZE,
          lamports,
          programId: TOKEN_PROGRAM_ID,
        }),
        createInitializeMint2Instruction(mintKp.publicKey, decimals, authority, null, TOKEN_PROGRAM_ID),
      ],
      [payer, mintKp]
    );
    return mintKp.publicKey;
  };

  const createAtaAndMint = async (mint: PublicKey, owner: PublicKey, amount: number) => {
    const ata = getAssociatedTokenAddressSync(mint, owner);
    await sendIxs(
      [
        createAssociatedTokenAccountInstruction(
          payer.publicKey,
          ata,
          owner,
          mint,
          TOKEN_PROGRAM_ID,
          ASSOCIATED_TOKEN_PROGRAM_ID
        ),
        createMintToInstruction(mint, ata, payer.publicKey, amount, [], TOKEN_PROGRAM_ID),
      ],
      [payer]
    );
    return ata;
  };

  const escrowPda = (makerKey: PublicKey, seed: BN) =>
    PublicKey.findProgramAddressSync(
      [Buffer.from("escrow"), makerKey.toBuffer(), seed.toArrayLike(Buffer, "le", 8)],
      program.programId
    )[0];

  // Instantly moves the on-chain clock `seconds` into the future.
  const warpSeconds = async (seconds: number) => {
    const clock = await context.banksClient.getClock();
    context.setClock(
      new Clock(
        clock.slot,
        clock.epochStartTimestamp,
        clock.epoch,
        clock.leaderScheduleEpoch,
        clock.unixTimestamp + BigInt(seconds)
      )
    );
  };

  before(async () => {
    context = await startAnchor(".", [], []);
    provider = new BankrunProvider(context);
    payer = context.payer;
    program = new Program(escrowIdl as anchor.Idl, provider) as unknown as Program<Escrow>;

    maker = Keypair.generate();
    taker = Keypair.generate();

    // `make` funds account rent from `maker` directly (not from the tx fee payer), so
    // maker/taker need real lamports here, unlike a plain fee-payer-subsidized call.
    for (const kp of [maker, taker]) {
      context.setAccount(kp.publicKey, {
        lamports: 2 * LAMPORTS_PER_SOL,
        data: Buffer.alloc(0),
        owner: SystemProgram.programId,
        executable: false,
      });
    }

    mintA = await createMint(DECIMALS, payer.publicKey);
    mintB = await createMint(DECIMALS, payer.publicKey);

    makerAtaA = await createAtaAndMint(mintA, maker.publicKey, DEPOSIT_AMOUNT * 10);
    takerAtaB = await createAtaAndMint(mintB, taker.publicKey, RECEIVE_AMOUNT * 10);
  });

  it("take: fails once the escrow's deadline has passed", async () => {
    const seed = new BN(1);
    const escrow = escrowPda(maker.publicKey, seed);
    const vault = getAssociatedTokenAddressSync(mintA, escrow, true);
    const takerAtaA = getAssociatedTokenAddressSync(mintA, taker.publicKey);
    const makerAtaB = getAssociatedTokenAddressSync(mintB, maker.publicKey);

    await program.methods
      .make(seed, new BN(DEPOSIT_AMOUNT), new BN(RECEIVE_AMOUNT), new BN(DEADLINE_DURATION))
      .accounts({
        maker: maker.publicKey,
        mintA,
        mintB,
        makerAtaA,
        escrow,
        vault,
        tokenProgram: TOKEN_PROGRAM_ID,
        associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
      })
      .signers([maker])
      .rpc();

    await warpSeconds(DEADLINE_DURATION + 5);

    try {
      await program.methods
        .take(seed)
        .accounts({
          taker: taker.publicKey,
          maker: maker.publicKey,
          mintA,
          mintB,
          takerAtaB,
          takerAtaA,
          makerAtaB,
          escrow,
          vault,
          tokenProgram: TOKEN_PROGRAM_ID,
          associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
          systemProgram: SystemProgram.programId,
        })
        .signers([taker])
        .rpc();
      assert.fail("expected take-after-deadline to fail");
    } catch (err) {
      assert.include(String(err), "EscrowExpired");
    }
  });

  it("reclaim: fails before the deadline has passed", async () => {
    const seed = new BN(2);
    const escrow = escrowPda(maker.publicKey, seed);
    const vault = getAssociatedTokenAddressSync(mintA, escrow, true);

    await program.methods
      .make(seed, new BN(DEPOSIT_AMOUNT), new BN(RECEIVE_AMOUNT), new BN(DEADLINE_DURATION))
      .accounts({
        maker: maker.publicKey,
        mintA,
        mintB,
        makerAtaA,
        escrow,
        vault,
        tokenProgram: TOKEN_PROGRAM_ID,
        associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
      })
      .signers([maker])
      .rpc();

    try {
      await program.methods
        .reclaim(seed)
        .accounts({
          caller: taker.publicKey,
          maker: maker.publicKey,
          mintA,
          makerAtaA,
          escrow,
          vault,
          tokenProgram: TOKEN_PROGRAM_ID,
          systemProgram: SystemProgram.programId,
        })
        .signers([taker])
        .rpc();
      assert.fail("expected early reclaim to fail");
    } catch (err) {
      assert.include(String(err), "EscrowNotExpired");
    }
  });

  it("reclaim: anyone can return an expired escrow's funds to the maker", async () => {
    const seed = new BN(3);
    const escrow = escrowPda(maker.publicKey, seed);
    const vault = getAssociatedTokenAddressSync(mintA, escrow, true);

    await program.methods
      .make(seed, new BN(DEPOSIT_AMOUNT), new BN(RECEIVE_AMOUNT), new BN(DEADLINE_DURATION))
      .accounts({
        maker: maker.publicKey,
        mintA,
        mintB,
        makerAtaA,
        escrow,
        vault,
        tokenProgram: TOKEN_PROGRAM_ID,
        associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
      })
      .signers([maker])
      .rpc();

    await warpSeconds(DEADLINE_DURATION + 5);

    const before = await getAccount(provider.connection as any, makerAtaA);

    // `taker` triggers the reclaim — a third party, not the maker — proving it's
    // permissionless. Funds still land in the maker's own token account.
    await program.methods
      .reclaim(seed)
      .accounts({
        caller: taker.publicKey,
        maker: maker.publicKey,
        mintA,
        makerAtaA,
        escrow,
        vault,
        tokenProgram: TOKEN_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
      })
      .signers([taker])
      .rpc();

    const after = await getAccount(provider.connection as any, makerAtaA);
    assert.strictEqual(Number(after.amount) - Number(before.amount), DEPOSIT_AMOUNT);

    assert.isNull(await context.banksClient.getAccount(vault), "vault token account should be closed");
    assert.isNull(await context.banksClient.getAccount(escrow), "escrow account should be closed");
  });
});
