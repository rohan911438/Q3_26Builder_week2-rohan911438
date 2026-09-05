import * as anchor from "@coral-xyz/anchor";
import { Program, BN } from "@coral-xyz/anchor";
import { Keypair, PublicKey, LAMPORTS_PER_SOL } from "@solana/web3.js";
import {
  createMint,
  getOrCreateAssociatedTokenAccount,
  getAssociatedTokenAddressSync,
  mintTo,
  getAccount,
} from "@solana/spl-token";
import { assert } from "chai";
import { ShareVault } from "../target/types/share_vault";

// Non-custodial vault extension: deposits mint a proportional share token; redemptions
// always succeed, paying out in underlying when idle liquidity allows and falling back
// to an in-kind transfer of the mock market receipt token when it doesn't. See the
// README section on this extension for the full design rationale.
describe("share_vault (non-custodial vault extension)", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const program = anchor.workspace.ShareVault as Program<ShareVault>;
  const connection = provider.connection;
  const payer = (provider.wallet as anchor.Wallet).payer;

  const DECIMALS = 6;

  let underlyingMint: PublicKey;
  let alice: Keypair;
  let bob: Keypair;
  let aliceUnderlying: PublicKey;
  let bobUnderlying: PublicKey;

  let vault: PublicKey;
  let vaultAuthority: PublicKey;
  let shareMint: PublicKey;
  let marketMint: PublicKey;
  let idleUnderlying: PublicKey;
  let marketCustody: PublicKey;
  let marketPosition: PublicKey;

  const airdrop = async (pubkey: PublicKey, sol: number) => {
    const sig = await connection.requestAirdrop(pubkey, sol * LAMPORTS_PER_SOL);
    const latest = await connection.getLatestBlockhash();
    await connection.confirmTransaction({ signature: sig, ...latest });
  };

  before(async () => {
    alice = Keypair.generate();
    bob = Keypair.generate();
    await airdrop(alice.publicKey, 2);
    await airdrop(bob.publicKey, 2);

    underlyingMint = await createMint(connection, payer, payer.publicKey, null, DECIMALS);

    aliceUnderlying = (
      await getOrCreateAssociatedTokenAccount(connection, payer, underlyingMint, alice.publicKey)
    ).address;
    bobUnderlying = (
      await getOrCreateAssociatedTokenAccount(connection, payer, underlyingMint, bob.publicKey)
    ).address;

    await mintTo(connection, payer, underlyingMint, aliceUnderlying, payer, 10_000_000);
    await mintTo(connection, payer, underlyingMint, bobUnderlying, payer, 10_000_000);

    [vault] = PublicKey.findProgramAddressSync(
      [Buffer.from("share_vault"), underlyingMint.toBuffer()],
      program.programId
    );
    [vaultAuthority] = PublicKey.findProgramAddressSync(
      [Buffer.from("share_vault"), underlyingMint.toBuffer(), Buffer.from("authority")],
      program.programId
    );
    [shareMint] = PublicKey.findProgramAddressSync(
      [Buffer.from("share_vault"), underlyingMint.toBuffer(), Buffer.from("share_mint")],
      program.programId
    );
    [marketMint] = PublicKey.findProgramAddressSync(
      [Buffer.from("share_vault"), underlyingMint.toBuffer(), Buffer.from("market_mint")],
      program.programId
    );
    [idleUnderlying] = PublicKey.findProgramAddressSync(
      [Buffer.from("share_vault"), underlyingMint.toBuffer(), Buffer.from("idle")],
      program.programId
    );
    [marketCustody] = PublicKey.findProgramAddressSync(
      [Buffer.from("share_vault"), underlyingMint.toBuffer(), Buffer.from("market_custody")],
      program.programId
    );
    marketPosition = getAssociatedTokenAddressSync(marketMint, vaultAuthority, true);
  });

  it("initializes the vault, its share mint, and its mock market accounts", async () => {
    await program.methods
      .initializeVault()
      .accounts({
        payer: payer.publicKey,
        underlyingMint,
        vault,
        vaultAuthority,
        shareMint,
        marketMint,
        idleUnderlying,
        marketCustody,
        marketPosition,
      })
      .rpc();

    const vaultAccount = await program.account.shareVault.fetch(vault);
    assert.strictEqual(vaultAccount.underlyingMint.toBase58(), underlyingMint.toBase58());
    assert.strictEqual(vaultAccount.shareMint.toBase58(), shareMint.toBase58());
    assert.strictEqual(vaultAccount.marketMint.toBase58(), marketMint.toBase58());
  });

  it("deposit: first depositor gets shares 1:1 with their deposit", async () => {
    const aliceShares = getAssociatedTokenAddressSync(shareMint, alice.publicKey);

    await program.methods
      .deposit(new BN(1_000_000))
      .accounts({
        depositor: alice.publicKey,
        underlyingMint,
        vault,
        vaultAuthority,
        shareMint,
        depositorUnderlying: aliceUnderlying,
        depositorShares: aliceShares,
        idleUnderlying,
        marketPosition,
      })
      .signers([alice])
      .rpc();

    const shares = await getAccount(connection, aliceShares);
    assert.strictEqual(Number(shares.amount), 1_000_000);

    const idle = await getAccount(connection, idleUnderlying);
    assert.strictEqual(Number(idle.amount), 1_000_000);
  });

  it("deposit: a second depositor gets proportional shares at the current price", async () => {
    const bobShares = getAssociatedTokenAddressSync(shareMint, bob.publicKey);

    await program.methods
      .deposit(new BN(500_000))
      .accounts({
        depositor: bob.publicKey,
        underlyingMint,
        vault,
        vaultAuthority,
        shareMint,
        depositorUnderlying: bobUnderlying,
        depositorShares: bobShares,
        idleUnderlying,
        marketPosition,
      })
      .signers([bob])
      .rpc();

    const shares = await getAccount(connection, bobShares);
    // Price is still 1:1 at this point (nothing has been deployed to the mock market yet).
    assert.strictEqual(Number(shares.amount), 500_000);
  });

  it("fails to deposit a zero amount", async () => {
    const aliceShares = getAssociatedTokenAddressSync(shareMint, alice.publicKey);
    try {
      await program.methods
        .deposit(new BN(0))
        .accounts({
          depositor: alice.publicKey,
          underlyingMint,
          vault,
          vaultAuthority,
          shareMint,
          depositorUnderlying: aliceUnderlying,
          depositorShares: aliceShares,
          idleUnderlying,
          marketPosition,
        })
        .signers([alice])
        .rpc();
      assert.fail("expected zero-amount deposit to fail");
    } catch (err) {
      assert.include(String(err), "InvalidAmount");
    }
  });

  it("deploy_to_market: moves idle liquidity into the mock market position", async () => {
    // Pool holds 1,500,000 idle. Deploy all but 100,000, so a later large redemption
    // can't be paid entirely from idle.
    await program.methods
      .deployToMarket(new BN(1_400_000))
      .accounts({
        caller: bob.publicKey,
        underlyingMint,
        vault,
        vaultAuthority,
        marketMint,
        idleUnderlying,
        marketCustody,
        marketPosition,
      })
      .signers([bob])
      .rpc();

    const idle = await getAccount(connection, idleUnderlying);
    assert.strictEqual(Number(idle.amount), 100_000);

    const position = await getAccount(connection, marketPosition);
    assert.strictEqual(Number(position.amount), 1_400_000);
  });

  it("fails to deploy more than the idle balance", async () => {
    try {
      await program.methods
        .deployToMarket(new BN(1_000_000))
        .accounts({
          caller: bob.publicKey,
          underlyingMint,
          vault,
          vaultAuthority,
          marketMint,
          idleUnderlying,
          marketCustody,
          marketPosition,
        })
        .signers([bob])
        .rpc();
      assert.fail("expected over-deploy to fail");
    } catch (err) {
      assert.include(String(err), "InsufficientIdleLiquidity");
    }
  });

  it("redeem: pays out entirely in underlying when idle liquidity covers it", async () => {
    const aliceShares = getAssociatedTokenAddressSync(shareMint, alice.publicKey);
    const aliceMarketReceipt = getAssociatedTokenAddressSync(marketMint, alice.publicKey);

    const underlyingBefore = await getAccount(connection, aliceUnderlying);

    await program.methods
      .redeem(new BN(50_000))
      .accounts({
        redeemer: alice.publicKey,
        underlyingMint,
        vault,
        vaultAuthority,
        shareMint,
        marketMint,
        marketPosition,
        idleUnderlying,
        redeemerShares: aliceShares,
        redeemerUnderlying: aliceUnderlying,
        redeemerMarketReceipt: aliceMarketReceipt,
      })
      .signers([alice])
      .rpc();

    const underlyingAfter = await getAccount(connection, aliceUnderlying);
    assert.strictEqual(Number(underlyingAfter.amount) - Number(underlyingBefore.amount), 50_000);

    const receipt = await getAccount(connection, aliceMarketReceipt);
    assert.strictEqual(Number(receipt.amount), 0, "no in-kind payout should have been needed");
  });

  it("redeem: always lets the holder exit, paying the shortfall in-kind when idle liquidity is insufficient", async () => {
    const bobShares = getAssociatedTokenAddressSync(shareMint, bob.publicKey);
    const bobMarketReceipt = getAssociatedTokenAddressSync(marketMint, bob.publicKey);

    // Idle is now 50,000 (100,000 - the 50,000 Alice just redeemed). Bob redeems shares
    // worth more than that, so this can only succeed if the shortfall is covered in-kind.
    const idleBefore = await getAccount(connection, idleUnderlying);
    assert.strictEqual(Number(idleBefore.amount), 50_000);

    const underlyingBefore = await getAccount(connection, bobUnderlying);

    await program.methods
      .redeem(new BN(200_000))
      .accounts({
        redeemer: bob.publicKey,
        underlyingMint,
        vault,
        vaultAuthority,
        shareMint,
        marketMint,
        marketPosition,
        idleUnderlying,
        redeemerShares: bobShares,
        redeemerUnderlying: bobUnderlying,
        redeemerMarketReceipt: bobMarketReceipt,
      })
      .signers([bob])
      .rpc();

    const underlyingAfter = await getAccount(connection, bobUnderlying);
    assert.strictEqual(
      Number(underlyingAfter.amount) - Number(underlyingBefore.amount),
      50_000,
      "should receive all remaining idle liquidity"
    );

    const receipt = await getAccount(connection, bobMarketReceipt);
    assert.strictEqual(
      Number(receipt.amount),
      150_000,
      "shortfall should be paid in-kind as market_mint receipts"
    );

    const idleAfter = await getAccount(connection, idleUnderlying);
    assert.strictEqual(Number(idleAfter.amount), 0);
  });

  it("withdraw_from_market: moves mock market capital back to idle", async () => {
    await program.methods
      .withdrawFromMarket(new BN(300_000))
      .accounts({
        caller: alice.publicKey,
        underlyingMint,
        vault,
        vaultAuthority,
        marketMint,
        marketPosition,
        marketCustody,
        idleUnderlying,
      })
      .signers([alice])
      .rpc();

    const idle = await getAccount(connection, idleUnderlying);
    assert.strictEqual(Number(idle.amount), 300_000);
  });

  it("fails to withdraw more from the market than the vault has deployed", async () => {
    try {
      await program.methods
        .withdrawFromMarket(new BN(100_000_000))
        .accounts({
          caller: alice.publicKey,
          underlyingMint,
          vault,
          vaultAuthority,
          marketMint,
          marketPosition,
          marketCustody,
          idleUnderlying,
        })
        .signers([alice])
        .rpc();
      assert.fail("expected over-withdraw from market to fail");
    } catch (err) {
      assert.include(String(err), "InsufficientMarketPosition");
    }
  });
});
