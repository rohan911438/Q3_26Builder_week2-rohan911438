import * as anchor from "@coral-xyz/anchor";
import { Program, BN } from "@coral-xyz/anchor";
import {
  Keypair,
  PublicKey,
  LAMPORTS_PER_SOL,
  SystemProgram,
} from "@solana/web3.js";
import {
  TOKEN_PROGRAM_ID,
  ASSOCIATED_TOKEN_PROGRAM_ID,
  createMint,
  getOrCreateAssociatedTokenAccount,
  mintTo,
  getAssociatedTokenAddressSync,
  getAccount,
} from "@solana/spl-token";
import { assert } from "chai";
import { Escrow } from "../target/types/escrow";

describe("escrow", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const program = anchor.workspace.Escrow as Program<Escrow>;
  const connection = provider.connection;
  const payer = (provider.wallet as anchor.Wallet).payer;

  const DECIMALS = 6;
  const DEPOSIT_AMOUNT = 1_000_000; // 1.0 token A
  const RECEIVE_AMOUNT = 2_000_000; // 2.0 token B

  let maker: Keypair;
  let taker: Keypair;
  let mintA: PublicKey;
  let mintB: PublicKey;
  let makerAtaA: PublicKey;
  let takerAtaB: PublicKey;

  const airdrop = async (pubkey: PublicKey, sol: number) => {
    const sig = await connection.requestAirdrop(pubkey, sol * LAMPORTS_PER_SOL);
    const latest = await connection.getLatestBlockhash();
    await connection.confirmTransaction({ signature: sig, ...latest });
  };

  const escrowPda = (makerKey: PublicKey, seed: BN) =>
    PublicKey.findProgramAddressSync(
      [Buffer.from("escrow"), makerKey.toBuffer(), seed.toArrayLike(Buffer, "le", 8)],
      program.programId
    )[0];

  before(async () => {
    maker = Keypair.generate();
    taker = Keypair.generate();
    await airdrop(maker.publicKey, 2);
    await airdrop(taker.publicKey, 2);

    mintA = await createMint(connection, payer, payer.publicKey, null, DECIMALS);
    mintB = await createMint(connection, payer, payer.publicKey, null, DECIMALS);

    makerAtaA = (
      await getOrCreateAssociatedTokenAccount(connection, payer, mintA, maker.publicKey)
    ).address;
    takerAtaB = (
      await getOrCreateAssociatedTokenAccount(connection, payer, mintB, taker.publicKey)
    ).address;

    await mintTo(connection, payer, mintA, makerAtaA, payer, DEPOSIT_AMOUNT * 10);
    await mintTo(connection, payer, mintB, takerAtaB, payer, RECEIVE_AMOUNT * 10);
  });

  it("make: locks token A and records escrow terms", async () => {
    const seed = new BN(1);
    const escrow = escrowPda(maker.publicKey, seed);
    const vault = getAssociatedTokenAddressSync(mintA, escrow, true);

    await program.methods
      .make(seed, new BN(DEPOSIT_AMOUNT), new BN(RECEIVE_AMOUNT))
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

    const escrowAccount = await program.account.escrow.fetch(escrow);
    assert.strictEqual(escrowAccount.maker.toBase58(), maker.publicKey.toBase58());
    assert.strictEqual(escrowAccount.mintA.toBase58(), mintA.toBase58());
    assert.strictEqual(escrowAccount.mintB.toBase58(), mintB.toBase58());
    assert.strictEqual(escrowAccount.receiveAmount.toNumber(), RECEIVE_AMOUNT);

    const vaultAccount = await getAccount(connection, vault);
    assert.strictEqual(Number(vaultAccount.amount), DEPOSIT_AMOUNT);
  });

  it("update: maker can change terms before a take", async () => {
    const seed = new BN(1);
    const escrow = escrowPda(maker.publicKey, seed);
    const newReceiveAmount = new BN(RECEIVE_AMOUNT * 2);

    await program.methods
      .update(seed, mintB, newReceiveAmount)
      .accounts({
        maker: maker.publicKey,
        escrow,
      })
      .signers([maker])
      .rpc();

    const escrowAccount = await program.account.escrow.fetch(escrow);
    assert.strictEqual(escrowAccount.receiveAmount.toNumber(), newReceiveAmount.toNumber());

    // Revert back so the take test below uses the originally agreed price.
    await program.methods
      .update(seed, mintB, new BN(RECEIVE_AMOUNT))
      .accounts({ maker: maker.publicKey, escrow })
      .signers([maker])
      .rpc();
  });

  it("update: fails when called by a non-maker", async () => {
    const seed = new BN(1);
    const escrow = escrowPda(maker.publicKey, seed);

    try {
      await program.methods
        .update(seed, mintB, new BN(RECEIVE_AMOUNT))
        .accounts({ maker: taker.publicKey, escrow })
        .signers([taker])
        .rpc();
      assert.fail("expected non-maker update to fail");
    } catch (err) {
      assert.include(String(err), "Error");
    }
  });

  it("take: swaps tokens atomically and closes the vault + escrow accounts", async () => {
    const seed = new BN(1);
    const escrow = escrowPda(maker.publicKey, seed);
    const vault = getAssociatedTokenAddressSync(mintA, escrow, true);
    const takerAtaA = getAssociatedTokenAddressSync(mintA, taker.publicKey);
    const makerAtaB = getAssociatedTokenAddressSync(mintB, maker.publicKey);

    const makerLamportsBefore = await connection.getBalance(maker.publicKey);

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

    const takerAtaAAccount = await getAccount(connection, takerAtaA);
    assert.strictEqual(Number(takerAtaAAccount.amount), DEPOSIT_AMOUNT);

    const makerAtaBAccount = await getAccount(connection, makerAtaB);
    assert.strictEqual(Number(makerAtaBAccount.amount), RECEIVE_AMOUNT);

    const vaultInfo = await connection.getAccountInfo(vault);
    assert.isNull(vaultInfo, "vault token account should be closed");

    const escrowInfo = await connection.getAccountInfo(escrow);
    assert.isNull(escrowInfo, "escrow account should be closed");

    const makerLamportsAfter = await connection.getBalance(maker.publicKey);
    assert.isAbove(makerLamportsAfter, makerLamportsBefore, "maker should receive rent back");
  });

  it("update: fails after the escrow has been taken", async () => {
    const seed = new BN(1);
    const escrow = escrowPda(maker.publicKey, seed);

    try {
      await program.methods
        .update(seed, mintB, new BN(RECEIVE_AMOUNT))
        .accounts({ maker: maker.publicKey, escrow })
        .signers([maker])
        .rpc();
      assert.fail("expected update-after-take to fail");
    } catch (err) {
      assert.include(String(err), "Error");
    }
  });

  it("refund: returns token A to maker and closes accounts", async () => {
    const seed = new BN(2);
    const escrow = escrowPda(maker.publicKey, seed);
    const vault = getAssociatedTokenAddressSync(mintA, escrow, true);

    await program.methods
      .make(seed, new BN(DEPOSIT_AMOUNT), new BN(RECEIVE_AMOUNT))
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

    const makerAtaABefore = await getAccount(connection, makerAtaA);

    await program.methods
      .refund(seed)
      .accounts({
        maker: maker.publicKey,
        mintA,
        makerAtaA,
        escrow,
        vault,
        tokenProgram: TOKEN_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
      })
      .signers([maker])
      .rpc();

    const makerAtaAAfter = await getAccount(connection, makerAtaA);
    assert.strictEqual(
      Number(makerAtaAAfter.amount) - Number(makerAtaABefore.amount),
      DEPOSIT_AMOUNT
    );

    const vaultInfo = await connection.getAccountInfo(vault);
    assert.isNull(vaultInfo, "vault token account should be closed");

    const escrowInfo = await connection.getAccountInfo(escrow);
    assert.isNull(escrowInfo, "escrow account should be closed");
  });

  it("refund: fails after the escrow has already been taken", async () => {
    const seed = new BN(3);
    const escrow = escrowPda(maker.publicKey, seed);
    const vault = getAssociatedTokenAddressSync(mintA, escrow, true);
    const takerAtaA = getAssociatedTokenAddressSync(mintA, taker.publicKey);
    const makerAtaB = getAssociatedTokenAddressSync(mintB, maker.publicKey);

    await program.methods
      .make(seed, new BN(DEPOSIT_AMOUNT), new BN(RECEIVE_AMOUNT))
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

    try {
      await program.methods
        .refund(seed)
        .accounts({
          maker: maker.publicKey,
          mintA,
          makerAtaA,
          escrow,
          vault,
          tokenProgram: TOKEN_PROGRAM_ID,
          systemProgram: SystemProgram.programId,
        })
        .signers([maker])
        .rpc();
      assert.fail("expected refund-after-take to fail");
    } catch (err) {
      assert.include(String(err), "Error");
    }
  });
});
