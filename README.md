# Q3_26Builder_week2-rohan911438

Turbine Cohort Week 2 homework: a custodial SOL **Vault** program and an SPL-token **Escrow**
program, both written in Anchor, with full TypeScript test coverage.

- Anchor `1.1.2` / Solana CLI `3.1.10` / Rust `1.89.0` toolchain (workspace-pinned via
  `rust-toolchain.toml`).
- Tests use Anchor's TS harness (Mocha + `ts-mocha` + `@coral-xyz/anchor` + `@solana/web3.js` +
  `@solana/spl-token`) against a local `solana-test-validator` spun up by `anchor test`.

## Build & test

```bash
npm install
anchor build
anchor test
```

`anchor test` builds both programs, boots a local validator, and runs `tests/vault.ts` and
`tests/escrow.ts` (16 tests total, happy path + at least one failure case per instruction).

**Screenshot of all tests passing:** add it at `docs/tests-passing.png` after running `anchor test`
locally (not included here since it has to be captured from your own terminal run).

---

## Vault program

A single-owner custodial SOL vault: one owner deposits and withdraws their own SOL through a
program-owned PDA, with an explicit close step that refuses to sweep undrained funds.

### Accounts / PDAs

```
                         seeds: ["vault", owner]
   owner (signer) ────────────────────────────► Vault (state PDA)
                                                   owner, bump, vault_bump,
                                                   total_deposited, locked

                         seeds: ["vault", owner, "vault_authority"]
   owner ─────────────────────────────────────► vault_authority (system-owned PDA)
                                                   holds the actual lamports;
                                                   never deserialized as typed data
```

The `Vault` state account and the `vault_authority` lamport-holding PDA are derived
independently from the same owner so the state account's rent-exempt balance is never mixed
with user funds — `total_deposited` (in `Vault`) is the accounting source of truth, and
`vault_authority`'s lamport balance should always equal it.

### Instructions

| Instruction | Accounts | Description |
|---|---|---|
| `initialize` | `owner` (signer, mut), `vault` (init), `vault_authority`, `system_program` | Creates the `Vault` state PDA for `owner`. |
| `deposit(amount)` | `owner` (signer, mut), `vault` (mut), `vault_authority` (mut), `system_program` | Transfers `amount` lamports from the owner into `vault_authority`; increments `total_deposited`. |
| `withdraw(amount)` | `owner` (signer, mut), `vault` (mut), `vault_authority` (mut), `system_program` | Transfers `amount` lamports back to the owner via a PDA-signed CPI; decrements `total_deposited`. |
| `close` | `owner` (signer, mut), `vault` (mut, closed), `vault_authority` | Closes the `Vault` state account and returns its rent to the owner. |

**Design decisions:**
- **Deposits and withdrawals are owner-only** (not a pooled/multi-depositor vault) — this is a
  personal custodial vault, not a shared pool, so there's no need for share accounting.
- **`close` refuses to run while `total_deposited > 0`** (`VaultError::VaultNotEmpty`) rather than
  silently sweeping remaining funds to the owner on close. This is deliberate: an explicit
  `withdraw` before `close` means the owner always sees exactly how much they withdrew in its own
  transaction, instead of a close instruction silently moving an unbounded amount of SOL.
- `locked: bool` is reserved on `Vault` for future extensions (e.g. timed locks) but unused by the
  core instructions.

### Errors

| Error | Condition |
|---|---|
| `Unauthorized` | Signer does not match `vault.owner`. |
| `InsufficientFunds` | `withdraw` amount exceeds `vault.total_deposited`. |
| `VaultNotEmpty` | `close` called while `vault.total_deposited > 0`. |
| `InvalidAmount` | `deposit`/`withdraw` called with `amount == 0` (or an overflowing deposit). |

---

## Escrow program

A classic SPL-token escrow: a maker locks token A in a program-owned vault and names a price in
token B; a taker can atomically pay that price and claim the vaulted token A, or the maker can
refund/update the offer before that happens.

### Accounts / PDAs

```
                    seeds: ["escrow", maker, seed]
  maker (signer) ─────────────────────────────► Escrow (state PDA)
                                                    seed, maker, mint_a, mint_b,
                                                    receive_amount, bump
                                                        │
                                                        │ associated_token::authority
                                                        ▼
                                                  vault (ATA, mint = mint_a)
                                                    holds the maker's deposited token A
```

`seed` lets one maker run multiple concurrent escrows. Client calls to `take`/`refund`/`update`
pass `seed` back in as an instruction argument so the PDA can be re-derived and validated without
the program needing to trust a client-supplied escrow address.

### Instructions

| Instruction | Accounts | Description |
|---|---|---|
| `make(seed, deposit_amount, receive_amount)` | `maker` (signer, mut), `mint_a`, `mint_b`, `maker_ata_a` (mut), `escrow` (init), `vault` (init, ATA), `token_program`, `associated_token_program`, `system_program` | Creates the escrow PDA and vault ATA, and moves `deposit_amount` of token A from the maker into the vault via `transfer_checked`. |
| `take(seed)` | `taker` (signer, mut), `maker` (mut), `mint_a`, `mint_b`, `taker_ata_b` (mut), `taker_ata_a` (init-if-needed), `maker_ata_b` (init-if-needed), `escrow` (mut, closed), `vault` (mut), `token_program`, `associated_token_program`, `system_program` | Taker pays `receive_amount` of token B to the maker, then the escrow PDA releases the vaulted token A to the taker; closes the vault ATA and the escrow account, rent returned to the maker. |
| `refund(seed)` | `maker` (signer, mut), `mint_a`, `maker_ata_a` (mut), `escrow` (mut, closed), `vault` (mut), `token_program`, `system_program` | Maker cancels before a take: returns the vaulted token A to the maker and closes the vault ATA + escrow account. |
| `update(seed, mint_b, receive_amount)` | `maker` (signer), `escrow` (mut) | Maker changes the asked-for mint/amount on an escrow that has not yet been taken. |

**Design decisions:**
- **`take`/`refund` close the escrow account**, so `update` (and a second `take`/`refund`) after
  either of those naturally fails — there's no separate "already taken" flag to maintain; the
  account simply no longer exists, which surfaces as an Anchor account-resolution error in tests.
- **Amounts are read from the vault's live token balance** (`vault.amount`) at `take`/`refund`
  time rather than duplicated into `Escrow` state, so there is exactly one source of truth for how
  much token A is owed.
- `take`'s `taker_ata_a` and `maker_ata_b` use `init_if_needed` so a first-time taker/maker pair
  doesn't need to pre-create associated token accounts.
- Several `Account<'info, T>` fields in `TakeEscrow` are wrapped in `Box<...>` — with 12 accounts
  in that context, the generated `try_accounts` stack frame exceeded Solana BPF's 4KB per-frame
  limit; boxing moves the deserialized account structs to the heap.
- `EscrowError` intentionally omits an `EscrowExpired` variant in this core version — there's no
  deadline concept yet. That would be introduced together with the (not-yet-implemented) timed
  escrow extension.

### Errors

| Error | Condition |
|---|---|
| `InvalidAmount` | `make` called with `deposit_amount == 0` or `receive_amount == 0`, or `update` with `receive_amount == 0`. |
| `Unauthorized` | Signer does not match `escrow.maker` (checked on `take`, `refund`, `update`). |

---

## What's implemented vs. not

- [x] Vault: `initialize`, `deposit`, `withdraw`, `close` — implemented and tested.
- [x] Escrow: `make`, `take`, `refund`, `update` — implemented and tested.
- [x] Full test suite passing (`anchor test`, 16/16).
- [ ] Timed escrow extension (deadline + `EscrowExpired`) — not implemented in this pass.
- [ ] Non-custodial share-token vault extension — not implemented in this pass.
