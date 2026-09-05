import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { PublicKey, SystemProgram, LAMPORTS_PER_SOL, Keypair } from "@solana/web3.js";
import { assert } from "chai";
import { Vault } from "../target/types/vault";

describe("vault", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const program = anchor.workspace.Vault as Program<Vault>;
  const connection = provider.connection;

  const owner = (provider.wallet as anchor.Wallet).payer;

  let vaultPda: PublicKey;
  let vaultBump: number;
  let vaultAuthorityPda: PublicKey;
  let vaultAuthorityBump: number;

  before(() => {
    [vaultPda, vaultBump] = PublicKey.findProgramAddressSync(
      [Buffer.from("vault"), owner.publicKey.toBuffer()],
      program.programId
    );
    [vaultAuthorityPda, vaultAuthorityBump] = PublicKey.findProgramAddressSync(
      [Buffer.from("vault"), owner.publicKey.toBuffer(), Buffer.from("vault_authority")],
      program.programId
    );
  });

  const airdrop = async (pubkey: PublicKey, sol: number) => {
    const sig = await connection.requestAirdrop(pubkey, sol * LAMPORTS_PER_SOL);
    const latest = await connection.getLatestBlockhash();
    await connection.confirmTransaction({ signature: sig, ...latest });
  };

  it("initializes the vault", async () => {
    await program.methods
      .initialize()
      .accounts({
        owner: owner.publicKey,
        vault: vaultPda,
        vaultAuthority: vaultAuthorityPda,
        systemProgram: SystemProgram.programId,
      })
      .rpc();

    const vault = await program.account.vault.fetch(vaultPda);
    assert.strictEqual(vault.owner.toBase58(), owner.publicKey.toBase58());
    assert.strictEqual(vault.bump, vaultBump);
    assert.strictEqual(vault.vaultBump, vaultAuthorityBump);
    assert.strictEqual(vault.totalDeposited.toNumber(), 0);
    assert.strictEqual(vault.locked, false);
  });

  it("fails to initialize the same vault twice", async () => {
    try {
      await program.methods
        .initialize()
        .accounts({
          owner: owner.publicKey,
          vault: vaultPda,
          vaultAuthority: vaultAuthorityPda,
          systemProgram: SystemProgram.programId,
        })
        .rpc();
      assert.fail("expected double-initialize to fail");
    } catch (err) {
      assert.include(String(err), "already in use");
    }
  });

  it("deposits SOL into the vault", async () => {
    const depositAmount = 0.5 * LAMPORTS_PER_SOL;

    await program.methods
      .deposit(new anchor.BN(depositAmount))
      .accounts({
        owner: owner.publicKey,
        vault: vaultPda,
        vaultAuthority: vaultAuthorityPda,
        systemProgram: SystemProgram.programId,
      })
      .rpc();

    const vault = await program.account.vault.fetch(vaultPda);
    assert.strictEqual(vault.totalDeposited.toNumber(), depositAmount);

    const authorityBalance = await connection.getBalance(vaultAuthorityPda);
    assert.strictEqual(authorityBalance, depositAmount);
  });

  it("withdraws SOL as the owner", async () => {
    const withdrawAmount = 0.2 * LAMPORTS_PER_SOL;
    const balanceBefore = await connection.getBalance(owner.publicKey);

    await program.methods
      .withdraw(new anchor.BN(withdrawAmount))
      .accounts({
        owner: owner.publicKey,
        vault: vaultPda,
        vaultAuthority: vaultAuthorityPda,
        systemProgram: SystemProgram.programId,
      })
      .rpc();

    const vault = await program.account.vault.fetch(vaultPda);
    assert.strictEqual(vault.totalDeposited.toNumber(), 0.3 * LAMPORTS_PER_SOL);

    const balanceAfter = await connection.getBalance(owner.publicKey);
    assert.isAbove(balanceAfter, balanceBefore);
  });

  it("fails when a non-owner tries to withdraw", async () => {
    const intruder = Keypair.generate();
    await airdrop(intruder.publicKey, 1);

    try {
      await program.methods
        .withdraw(new anchor.BN(1000))
        .accounts({
          owner: intruder.publicKey,
          vault: vaultPda,
          vaultAuthority: vaultAuthorityPda,
          systemProgram: SystemProgram.programId,
        })
        .signers([intruder])
        .rpc();
      assert.fail("expected non-owner withdraw to fail");
    } catch (err) {
      assert.include(String(err), "Error");
    }
  });

  it("fails to withdraw more than the deposited balance", async () => {
    try {
      await program.methods
        .withdraw(new anchor.BN(1000 * LAMPORTS_PER_SOL))
        .accounts({
          owner: owner.publicKey,
          vault: vaultPda,
          vaultAuthority: vaultAuthorityPda,
          systemProgram: SystemProgram.programId,
        })
        .rpc();
      assert.fail("expected over-withdraw to fail");
    } catch (err) {
      assert.include(String(err), "InsufficientFunds");
    }
  });

  it("fails to close a vault that still holds funds", async () => {
    try {
      await program.methods
        .close()
        .accounts({
          owner: owner.publicKey,
          vault: vaultPda,
          vaultAuthority: vaultAuthorityPda,
        })
        .rpc();
      assert.fail("expected close-while-not-empty to fail");
    } catch (err) {
      assert.include(String(err), "VaultNotEmpty");
    }
  });

  it("fails when a non-owner tries to close", async () => {
    const intruder = Keypair.generate();
    await airdrop(intruder.publicKey, 1);

    try {
      await program.methods
        .close()
        .accounts({
          owner: intruder.publicKey,
          vault: vaultPda,
          vaultAuthority: vaultAuthorityPda,
        })
        .signers([intruder])
        .rpc();
      assert.fail("expected non-owner close to fail");
    } catch (err) {
      assert.include(String(err), "Error");
    }
  });

  it("withdraws the remaining balance and closes the vault, returning rent to the owner", async () => {
    const vaultBefore = await program.account.vault.fetch(vaultPda);
    const remaining = vaultBefore.totalDeposited;

    await program.methods
      .withdraw(remaining)
      .accounts({
        owner: owner.publicKey,
        vault: vaultPda,
        vaultAuthority: vaultAuthorityPda,
        systemProgram: SystemProgram.programId,
      })
      .rpc();

    const balanceBeforeClose = await connection.getBalance(owner.publicKey);

    await program.methods
      .close()
      .accounts({
        owner: owner.publicKey,
        vault: vaultPda,
        vaultAuthority: vaultAuthorityPda,
      })
      .rpc();

    const vaultAccount = await connection.getAccountInfo(vaultPda);
    assert.isNull(vaultAccount, "vault state account should be closed");

    const balanceAfterClose = await connection.getBalance(owner.publicKey);
    assert.isAbove(balanceAfterClose, balanceBeforeClose);
  });
});
